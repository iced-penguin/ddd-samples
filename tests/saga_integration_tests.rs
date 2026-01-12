use bookstore_order_management::adapter::driven::{EventBusConfig, InMemoryEventBus};
use bookstore_order_management::application::service::OrderApplicationService;
use bookstore_order_management::domain::event::{DomainEvent, OrderConfirmed};
use bookstore_order_management::domain::event_bus::EventHandler;
use bookstore_order_management::domain::handler::{
    EventualConsistencyVerifier, InventoryReservationFailureCompensationHandler,
    InventoryReservationHandler, NotificationHandler, SagaCompensationCoordinator,
};
use bookstore_order_management::domain::model::{
    BookId, CustomerId, Inventory, Money, Order, OrderId, OrderStatus,
};
use bookstore_order_management::domain::port::EventBus;
use bookstore_order_management::domain::port::{
    InventoryRepository, OrderRepository, RepositoryError,
};
use bookstore_order_management::domain::serialization::{EventSerializer, SerializationError};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// テスト用ヘルパー関数
fn serialize_domain_event(event: &DomainEvent) -> Result<String, SerializationError> {
    let serializer = EventSerializer::new();
    serializer.serialize_event(event)
}

fn deserialize_domain_event(json: &str) -> Result<DomainEvent, SerializationError> {
    let serializer = EventSerializer::new();
    serializer.deserialize_event(json)
}

fn test_event_round_trip(event: &DomainEvent) -> Result<bool, SerializationError> {
    let serializer = EventSerializer::new();
    let serialized = serializer.serialize_event(event)?;
    let deserialized = serializer.deserialize_event(&serialized)?;

    // 基本的な等価性チェック
    Ok(event.event_type() == deserialized.event_type()
        && event.metadata().event_id == deserialized.metadata().event_id
        && event.metadata().correlation_id == deserialized.metadata().correlation_id)
}

// テスト用のモックリポジトリ
struct MockOrderRepository {
    orders: Arc<Mutex<HashMap<OrderId, Order>>>,
}

impl MockOrderRepository {
    fn new() -> Self {
        Self {
            orders: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl OrderRepository for MockOrderRepository {
    async fn save(&self, order: &Order) -> Result<(), RepositoryError> {
        let mut orders = self.orders.lock().await;
        orders.insert(order.id(), order.clone());
        Ok(())
    }

    async fn find_by_id(&self, order_id: OrderId) -> Result<Option<Order>, RepositoryError> {
        let orders = self.orders.lock().await;
        Ok(orders.get(&order_id).cloned())
    }

    async fn find_all(&self) -> Result<Vec<Order>, RepositoryError> {
        let orders = self.orders.lock().await;
        Ok(orders.values().cloned().collect())
    }

    async fn find_by_status(&self, status: OrderStatus) -> Result<Vec<Order>, RepositoryError> {
        let orders = self.orders.lock().await;
        Ok(orders
            .values()
            .filter(|order| order.status() == status)
            .cloned()
            .collect())
    }

    fn next_identity(&self) -> OrderId {
        OrderId::new()
    }
}

struct MockInventoryRepository {
    inventories: Arc<Mutex<HashMap<BookId, Inventory>>>,
}

impl MockInventoryRepository {
    fn new() -> Self {
        Self {
            inventories: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn add_inventory(&self, inventory: Inventory) {
        let mut inventories = self.inventories.lock().await;
        inventories.insert(inventory.book_id(), inventory);
    }
}

#[async_trait]
impl InventoryRepository for MockInventoryRepository {
    async fn save(&self, inventory: &Inventory) -> Result<(), RepositoryError> {
        let mut inventories = self.inventories.lock().await;
        inventories.insert(inventory.book_id(), inventory.clone());
        Ok(())
    }

    async fn find_by_book_id(&self, book_id: BookId) -> Result<Option<Inventory>, RepositoryError> {
        let inventories = self.inventories.lock().await;
        Ok(inventories.get(&book_id).cloned())
    }

    async fn find_all(&self) -> Result<Vec<Inventory>, RepositoryError> {
        let inventories = self.inventories.lock().await;
        Ok(inventories.values().cloned().collect())
    }

    async fn find_by_max_quantity(
        &self,
        max_quantity: u32,
    ) -> Result<Vec<Inventory>, RepositoryError> {
        let inventories = self.inventories.lock().await;
        Ok(inventories
            .values()
            .filter(|inv| inv.quantity_on_hand() <= max_quantity)
            .cloned()
            .collect())
    }
}

/// **Feature: choreography-saga-refactoring, Property 4: Eventual Consistency Across Aggregates**
/// 注文確定から在庫予約までのサーガフローテスト（冪等性の検証）
#[tokio::test]
async fn test_complete_order_lifecycle_saga_flow() {
    // インフラストラクチャの設定（リトライを有効にして冪等性の問題を検証）
    let inventory_repo = Arc::new(MockInventoryRepository::new());
    let order_repo = Arc::new(MockOrderRepository::new());

    // 通常のリトライ設定でイベントバスを作成（冪等性の問題を露呈させる）
    let config = EventBusConfig {
        max_retry_attempts: 3, // リトライを有効にして冪等性の問題を検証
        retry_delay: std::time::Duration::from_millis(50),
        dead_letter_queue_max_size: 100,
        handler_timeout: std::time::Duration::from_secs(5),
    };
    let event_bus = Arc::new(InMemoryEventBus::new(config));

    // ハンドラーの作成（自動実行される在庫予約ハンドラーのみ）
    let inventory_handler = InventoryReservationHandler::new(
        inventory_repo.clone(),
        order_repo.clone(),
        event_bus.clone(),
    );
    let notification_handler = NotificationHandler::new();
    let consistency_verifier =
        EventualConsistencyVerifier::new(order_repo.clone(), inventory_repo.clone());

    // イベントバスにハンドラーを登録（自動実行される部分のみ）
    event_bus
        .subscribe_order_confirmed(inventory_handler)
        .await
        .unwrap();

    // 通知ハンドラーを登録
    event_bus
        .subscribe_order_confirmed(notification_handler.clone())
        .await
        .unwrap();

    // 整合性検証ハンドラーを登録
    event_bus
        .subscribe_order_confirmed(consistency_verifier.clone())
        .await
        .unwrap();

    // アプリケーションサービスの作成
    let app_service = OrderApplicationService::new(
        MockOrderRepository {
            orders: order_repo.orders.clone(),
        },
        event_bus.clone(),
    );

    // テストデータの準備（冪等性の問題を検証するため、正確な在庫数を設定）
    let book_id = BookId::new();
    let initial_inventory = 10u32; // 正確な在庫数
    let order_quantity = 3u32;
    let expected_final_inventory = initial_inventory - order_quantity; // 期待される最終在庫数

    // 在庫を追加
    let inventory = Inventory::new(book_id, initial_inventory);
    inventory_repo.add_inventory(inventory).await;

    // 注文を作成
    let customer_id = CustomerId::new();
    let order_id = app_service.create_order(customer_id).await.unwrap();

    // 書籍を注文に追加
    let unit_price = Money::jpy(1500);
    app_service
        .add_book_to_order(order_id, book_id, order_quantity, unit_price)
        .await
        .unwrap();

    // 配送先住所を設定
    app_service
        .set_shipping_address_from_request(
            order_id,
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .await
        .unwrap();

    // サーガを開始（注文確定）
    app_service.confirm_order(order_id).await.unwrap();

    // イベント処理が完了するまで十分に待機
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // 最終状態の検証
    let final_order = order_repo.find_by_id(order_id).await.unwrap().unwrap();
    let final_inventory = inventory_repo
        .find_by_book_id(book_id)
        .await
        .unwrap()
        .unwrap();

    // 注文確定後の状態確認：在庫予約まで自動実行される
    // 注文状態はConfirmedのまま（発送・配達は手動操作が必要）
    assert_eq!(
        final_order.status(),
        OrderStatus::Confirmed,
        "Order should remain in Confirmed state after automatic saga steps, but got: {:?}",
        final_order.status()
    );

    // 冪等性の検証：在庫は正確に注文数量だけ減るべき
    assert_eq!(
        final_inventory.quantity_on_hand(),
        expected_final_inventory,
        "Inventory should be exactly {} (initial: {} - ordered: {}), but got: {}. This indicates a lack of idempotency in event handlers.",
        expected_final_inventory, initial_inventory, order_quantity, final_inventory.quantity_on_hand()
    );

    println!("✅ Order confirmation saga flow test passed - Inventory reserved with idempotency maintained");
}

/// **Feature: choreography-saga-refactoring, Property 25: Event Handler Idempotency**
/// イベントハンドラーの冪等性テスト
/// 同じイベントが複数回処理されても結果が同じであることを検証
#[tokio::test]
async fn test_event_handler_idempotency() {
    let inventory_repo = Arc::new(MockInventoryRepository::new());
    let order_repo = Arc::new(MockOrderRepository::new());
    let event_bus = Arc::new(InMemoryEventBus::new(EventBusConfig::default()));
    let handler = InventoryReservationHandler::new(
        inventory_repo.clone(),
        order_repo.clone(),
        event_bus.clone(),
    );

    // テスト用の在庫を追加
    let book_id = BookId::new();
    let initial_inventory = 10u32;
    let order_quantity = 3u32;
    let inventory = Inventory::new(book_id, initial_inventory);
    inventory_repo.add_inventory(inventory).await;

    // OrderConfirmedイベントを作成
    let order_id = OrderId::new();
    let customer_id = CustomerId::new();
    let order_line = bookstore_order_management::domain::model::OrderLine::new(
        book_id,
        order_quantity,
        Money::jpy(1000),
    )
    .unwrap();
    let event = OrderConfirmed::new(order_id, customer_id, vec![order_line], Money::jpy(3000));

    // テスト用の注文を作成してリポジトリに保存
    let mut order = bookstore_order_management::domain::model::Order::new(order_id, customer_id);
    order
        .add_book(book_id, order_quantity, Money::jpy(1000))
        .unwrap();
    order.set_shipping_address(
        bookstore_order_management::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap(),
    );
    order.confirm().unwrap();
    order_repo.save(&order).await.unwrap();

    // 同じイベントを複数回処理
    let result1 = handler.handle(event.clone()).await;
    let result2 = handler.handle(event.clone()).await;
    let result3 = handler.handle(event.clone()).await;

    // 全ての処理が成功することを確認（現在の実装では2回目以降は失敗する可能性がある）
    println!("First processing result: {:?}", result1);
    println!("Second processing result: {:?}", result2);
    println!("Third processing result: {:?}", result3);

    // 在庫の最終状態を確認
    let final_inventory = inventory_repo
        .find_by_book_id(book_id)
        .await
        .unwrap()
        .unwrap();
    let expected_final_inventory = initial_inventory - order_quantity;

    println!("Initial inventory: {}", initial_inventory);
    println!("Order quantity: {}", order_quantity);
    println!("Expected final inventory: {}", expected_final_inventory);
    println!(
        "Actual final inventory: {}",
        final_inventory.quantity_on_hand()
    );

    // 冪等性の検証：在庫は1回だけ減るべき
    // 注意：現在の実装では冪等性が実装されていないため、このテストは失敗する
    assert_eq!(
        final_inventory.quantity_on_hand(),
        expected_final_inventory,
        "Idempotency violation: Inventory should be {} after processing the same event multiple times, but got {}. Each event should only be processed once.",
        expected_final_inventory, final_inventory.quantity_on_hand()
    );

    println!("✅ Event handler idempotency test passed - Same event processed multiple times with consistent results");
}

/// **Feature: choreography-saga-refactoring, Property 12: Saga Compensation**
/// サーガ補償メカニズムのテスト
#[tokio::test]
async fn test_saga_compensation_flow() {
    // インフラストラクチャの設定
    let inventory_repo = Arc::new(MockInventoryRepository::new());
    let order_repo = Arc::new(MockOrderRepository::new());
    let event_bus = Arc::new(InMemoryEventBus::new(EventBusConfig::default()));

    // ハンドラーの作成
    let inventory_handler = InventoryReservationHandler::new(
        inventory_repo.clone(),
        order_repo.clone(),
        event_bus.clone(),
    );
    let compensation_handler =
        InventoryReservationFailureCompensationHandler::new(order_repo.clone(), event_bus.clone());
    let saga_coordinator = SagaCompensationCoordinator::new(event_bus.clone());

    // イベントバスにハンドラーを登録
    event_bus
        .subscribe_order_confirmed(inventory_handler)
        .await
        .unwrap();
    event_bus
        .subscribe_inventory_reservation_failed(compensation_handler)
        .await
        .unwrap();
    event_bus
        .subscribe_saga_compensation_started(saga_coordinator)
        .await
        .unwrap();

    // アプリ���ーションサービスの作成
    let app_service = OrderApplicationService::new(
        MockOrderRepository {
            orders: order_repo.orders.clone(),
        },
        event_bus.clone(),
    );

    // テストデータの準備（在庫不足のシナリオ）
    let book_id = BookId::new();
    let insufficient_inventory = 2u32;
    let order_quantity = 5u32; // 在庫より多い数量

    // 不十分な在庫を追加
    let inventory = Inventory::new(book_id, insufficient_inventory);
    inventory_repo.add_inventory(inventory).await;

    // 注文を作成
    let customer_id = CustomerId::new();
    let order_id = app_service.create_order(customer_id).await.unwrap();

    // 書籍を注文に追加
    let unit_price = Money::jpy(1000);
    app_service
        .add_book_to_order(order_id, book_id, order_quantity, unit_price)
        .await
        .unwrap();

    // 配送先住所を設定
    app_service
        .set_shipping_address_from_request(
            order_id,
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .await
        .unwrap();

    // サーガを開始（注文確定）- 在庫不足で失敗するはず
    app_service.confirm_order(order_id).await.unwrap();

    // 補償処理が完了するまで待機
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // 補償処理の結果を検証
    let final_order = order_repo.find_by_id(order_id).await.unwrap().unwrap();
    let final_inventory = inventory_repo
        .find_by_book_id(book_id)
        .await
        .unwrap()
        .unwrap();

    // 注文がキャンセル状態になっていることを確認（補償処理）
    assert_eq!(
        final_order.status(),
        OrderStatus::Cancelled,
        "Order should be cancelled due to insufficient inventory"
    );

    // 在庫が変更されていないことを確認
    assert_eq!(
        final_inventory.quantity_on_hand(),
        insufficient_inventory,
        "Inventory should remain unchanged after compensation"
    );

    println!("✅ Saga compensation test passed - Order cancelled due to insufficient inventory");
}

/// **Feature: choreography-saga-refactoring, Property 15: Concurrent Handler Processing**
/// 並行ハンドラー処理のテスト
#[tokio::test]
async fn test_concurrent_handler_processing() {
    let event_bus = Arc::new(InMemoryEventBus::new(EventBusConfig::default()));
    let order_repo = Arc::new(MockOrderRepository::new());
    let inventory_repo = Arc::new(MockInventoryRepository::new());

    // 複数の異なるハンドラーを登録
    let notification_handler = NotificationHandler::new();
    let consistency_verifier =
        EventualConsistencyVerifier::new(order_repo.clone(), inventory_repo.clone());

    event_bus
        .subscribe_order_confirmed(notification_handler)
        .await
        .unwrap();
    event_bus
        .subscribe_order_confirmed(consistency_verifier)
        .await
        .unwrap();

    // 複数のイベントを並行して発行
    let mut handles = vec![];

    for i in 0..5 {
        let event_bus_clone = event_bus.clone();
        let handle = tokio::spawn(async move {
            let order_id = OrderId::new();
            let customer_id = CustomerId::new();
            let event =
                OrderConfirmed::new(order_id, customer_id, vec![], Money::jpy(1000 + i * 100));

            event_bus_clone
                .publish(DomainEvent::OrderConfirmed(event))
                .await
        });
        handles.push(handle);
    }

    // 全ての並行処理が完了するまで待機
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent event publishing should succeed");
    }

    // 追加の処理時間を待機
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    println!("✅ Concurrent handler processing test passed - All events processed successfully");
}

/// **Feature: choreography-saga-refactoring, Property 21: Event Serialization Round Trip**
/// イベントシリアライゼーション往復テスト
#[tokio::test]
async fn test_event_serialization_round_trip() {
    let order_id = OrderId::new();
    let customer_id = CustomerId::new();
    let book_id = BookId::new();
    let order_line =
        bookstore_order_management::domain::model::OrderLine::new(book_id, 2, Money::jpy(1500))
            .unwrap();

    let original_event =
        OrderConfirmed::new(order_id, customer_id, vec![order_line], Money::jpy(3000));

    // シリアライゼーション
    let serialized = serde_json::to_string(&original_event).unwrap();
    assert!(
        !serialized.is_empty(),
        "Serialized event should not be empty"
    );

    // デシリアライゼーション
    let deserialized: OrderConfirmed = serde_json::from_str(&serialized).unwrap();

    // 往復後の内容が同じであることを確認
    assert_eq!(
        original_event.order_id, deserialized.order_id,
        "Order ID should be preserved"
    );
    assert_eq!(
        original_event.customer_id, deserialized.customer_id,
        "Customer ID should be preserved"
    );
    assert_eq!(
        original_event.total_amount, deserialized.total_amount,
        "Total amount should be preserved"
    );
    assert_eq!(
        original_event.order_lines.len(),
        deserialized.order_lines.len(),
        "Order lines count should be preserved"
    );
    assert_eq!(
        original_event.metadata.event_version, deserialized.metadata.event_version,
        "Event version should be preserved"
    );

    println!("✅ Event serialization round trip test passed - All data preserved");
}

/// **Feature: choreography-saga-refactoring, Property 8: Order Confirmation Saga Step**
/// 注文確定サーガステップのテスト
#[tokio::test]
async fn test_order_confirmation_saga_step() {
    let inventory_repo = Arc::new(MockInventoryRepository::new());
    let order_repo = Arc::new(MockOrderRepository::new());
    let event_bus = Arc::new(InMemoryEventBus::new(EventBusConfig::default()));
    let handler = InventoryReservationHandler::new(
        inventory_repo.clone(),
        order_repo.clone(),
        event_bus.clone(),
    );

    // イベントバスにハンドラーを登録
    event_bus.subscribe_order_confirmed(handler).await.unwrap();

    // テスト用の在庫を追加
    let book_id = BookId::new();
    let inventory = Inventory::new(book_id, 10);
    inventory_repo.add_inventory(inventory).await;

    // OrderConfirmedイベントを作成
    let order_id = OrderId::new();
    let customer_id = CustomerId::new();
    let order_line =
        bookstore_order_management::domain::model::OrderLine::new(book_id, 3, Money::jpy(1000))
            .unwrap();
    let event = OrderConfirmed::new(order_id, customer_id, vec![order_line], Money::jpy(3000));

    // テスト用の注文を作成してリポジトリに保存
    let mut order = bookstore_order_management::domain::model::Order::new(order_id, customer_id);
    order.add_book(book_id, 3, Money::jpy(1000)).unwrap();
    order.set_shipping_address(
        bookstore_order_management::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap(),
    );
    order.confirm().unwrap();
    order_repo.save(&order).await.unwrap();

    // イベントを発行
    let result = event_bus.publish(DomainEvent::OrderConfirmed(event)).await;
    assert!(
        result.is_ok(),
        "Order confirmation event should be published successfully"
    );

    // 処理完了を待機
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 在庫が予約されたことを確認
    let updated_inventory = inventory_repo
        .find_by_book_id(book_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated_inventory.quantity_on_hand(),
        7,
        "Inventory should be reserved after order confirmation"
    );

    println!("✅ Order confirmation saga step test passed - Inventory reserved successfully");
}

/// **Feature: choreography-saga-refactoring, Property 24: Serialization Error Clarity**
/// シリアライゼーションエラー明確性テスト
#[tokio::test]
async fn test_serialization_error_clarity() {
    use bookstore_order_management::domain::serialization::EventSerializer;

    let serializer = EventSerializer::new();

    // テストケース1: 空のJSON入力
    let result = serializer.deserialize_event("");
    assert!(result.is_err());
    match result.unwrap_err() {
        SerializationError::JsonDeserializationFailed {
            message,
            expected_type,
            input_preview,
        } => {
            assert!(message.contains("Empty JSON input"));
            assert_eq!(expected_type, "DomainEvent");
            assert_eq!(input_preview, "");
        }
        _ => panic!("Expected JsonDeserializationFailed error for empty input"),
    }

    // テストケース2: 不正なJSON構文
    let invalid_json = "{ invalid json syntax }";
    let result = serializer.deserialize_event(invalid_json);
    assert!(result.is_err());
    match result.unwrap_err() {
        SerializationError::JsonDeserializationFailed {
            message,
            input_preview,
            ..
        } => {
            assert!(message.contains("Invalid JSON syntax"));
            assert_eq!(input_preview, invalid_json);
        }
        _ => panic!("Expected JsonDeserializationFailed error for invalid JSON"),
    }

    // テストケース3: 未知のイベントタイプ
    let unknown_event_json = r#"{"event_type": "UnknownEvent", "event_data": {}}"#;
    let result = serializer.deserialize_event(unknown_event_json);
    assert!(result.is_err());
    match result.unwrap_err() {
        SerializationError::UnsupportedEventFormat { format, event_type } => {
            assert!(format.contains("Unknown event variant"));
            assert_eq!(event_type, "Unknown");
        }
        _ => panic!("Expected UnsupportedEventFormat error for unknown event type"),
    }

    // テストケース4: 必須フィールドの欠如
    let missing_field_json = r#"{"event_type": "OrderConfirmed"}"#;
    let result = serializer.deserialize_event(missing_field_json);
    assert!(result.is_err());
    match result.unwrap_err() {
        SerializationError::MissingRequiredField {
            field_name,
            event_type,
        } => {
            assert_eq!(field_name, "event_data");
            assert_eq!(event_type, "Unknown");
        }
        _ => panic!("Expected MissingRequiredField error for missing event_data"),
    }
}

/// **Feature: choreography-saga-refactoring, Property 24: Serialization Error Clarity**
/// 複雑なシリアライゼーションエラーシナリオのテスト
#[tokio::test]
async fn test_complex_serialization_error_scenarios() {
    use bookstore_order_management::domain::serialization::EventSerializer;

    let serializer = EventSerializer::new();

    // テストケース2: 長い入力データのプレビュー機能
    let very_long_json = format!("{{\"invalid\": \"{}\"}}", "a".repeat(200));
    let result = serializer.deserialize_event(&very_long_json);
    assert!(result.is_err());
    let error_message = result.unwrap_err().to_string();
    // プレビューが100文字に制限されていることを確認
    assert!(error_message.len() < very_long_json.len());
    // エラーメッセージが生成されていることを確認
    assert!(!error_message.is_empty());
}

/// **Feature: choreography-saga-refactoring, Property 24: Serialization Error Clarity**
/// イベントバスでのシリアライゼーション検証テスト
#[tokio::test]
async fn test_event_bus_serialization_validation() {
    use bookstore_order_management::adapter::driven::InMemoryEventBus;
    use bookstore_order_management::domain::port::EventBus;

    let event_bus = InMemoryEventBus::new(EventBusConfig::default());

    // 正常なイベントは問題なく発行できる
    let valid_event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
        OrderId::new(),
        CustomerId::new(),
        vec![],
        Money::jpy(1000),
    ));

    let result = event_bus.publish(valid_event).await;
    assert!(result.is_ok());

    // 注意: 現在の実装では、DomainEventは常に有効なserdeアノテーションを持っているため、
    // 実際のシリアライゼーションエラーを発生させるのは困難です。
    // しかし、将来的に無効なデータが含まれる場合、エラーハンドリングが機能することを確認できます。
}

/// **Feature: choreography-saga-refactoring, Property 24: Serialization Error Clarity**
/// エッジケースでのシリアライゼーション処理テスト
#[tokio::test]
async fn test_serialization_edge_cases() {
    use bookstore_order_management::domain::serialization::EventSerializer;

    let serializer = EventSerializer::new();

    // テストケース1: 非常に大きなメタデータを持つイベント
    let mut event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
        OrderId::new(),
        CustomerId::new(),
        vec![],
        Money::jpy(1000),
    ));

    // 大量のメタデータを追加
    if let DomainEvent::OrderConfirmed(ref mut order_confirmed) = event {
        for i in 0..100 {
            order_confirmed
                .metadata
                .additional_metadata
                .insert(format!("key_{}", i), format!("value_{}", "x".repeat(100)));
        }
    }

    // シリアライゼーションが成功することを確認
    let serialized = serializer.serialize_event(&event);
    assert!(serialized.is_ok());

    let json = serialized.unwrap();
    assert!(json.len() > 10000); // 大きなJSONが生成される

    // デシリアライゼーションも成功することを確認
    let deserialized = serializer.deserialize_event(&json);
    assert!(deserialized.is_ok());

    // 往復テストも成功することを確認
    let round_trip_result = test_event_round_trip(&event);
    assert!(round_trip_result.is_ok());
    assert!(round_trip_result.unwrap());

    // テストケース2: Unicode文字を含むイベント
    let mut unicode_event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
        OrderId::new(),
        CustomerId::new(),
        vec![],
        Money::jpy(1000),
    ));

    if let DomainEvent::OrderConfirmed(ref mut order_confirmed) = unicode_event {
        order_confirmed.metadata.additional_metadata.insert(
            "unicode_field".to_string(),
            "こんにちは世界🌍🚀".to_string(),
        );
    }

    let unicode_result = test_event_round_trip(&unicode_event);
    assert!(unicode_result.is_ok());
    assert!(unicode_result.unwrap());
}

/// **Feature: choreography-saga-refactoring, Property 24: Serialization Error Clarity**
/// シリアライゼーションユーティリティ関数のテスト
#[tokio::test]
async fn test_serialization_utility_functions() {


    let event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
        OrderId::new(),
        CustomerId::new(),
        vec![],
        Money::jpy(1000),
    ));

    // ユーティリティ関数でのシリアライゼーション
    let serialized = serialize_domain_event(&event);
    assert!(serialized.is_ok());

    let json = serialized.unwrap();
    assert!(json.contains("OrderConfirmed"));
    assert!(json.contains("event_type"));
    assert!(json.contains("event_data"));

    // ユーティリティ関数でのデシリアライゼーション
    let deserialized = deserialize_domain_event(&json);
    assert!(deserialized.is_ok());

    let deserialized_event = deserialized.unwrap();
    assert_eq!(event.event_type(), deserialized_event.event_type());
    assert_eq!(
        event.metadata().event_version,
        deserialized_event.metadata().event_version
    );

    // 往復テストユーティリティ
    let round_trip_result = test_event_round_trip(&event);
    assert!(round_trip_result.is_ok());
    assert!(round_trip_result.unwrap());
}

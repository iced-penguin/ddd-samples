mod adapter;
mod application;
mod domain;

use adapter::driven::{InMemoryEventBus, MySqlInventoryRepository, MySqlOrderRepository};
use adapter::driver::rest_api::{create_router, AppStateInner};
use adapter::{DatabaseConfig, DatabaseMigration};
use application::service::{InventoryApplicationService, OrderApplicationService};

use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 書店注文管理システム REST API ===");
    println!("ドメイン駆動設計サンプルプロジェクト");
    println!();

    // .envファイルから環境変数を読み込む
    dotenvy::dotenv().ok();

    // データベース設定を読み込む
    let config = DatabaseConfig::from_env()?;
    println!(
        "データベース設定を読み込みました: {}:{}",
        config.host, config.port
    );

    // 接続プールを作成
    let pool = MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.connection_string())
        .await?;
    println!("データベース接続プールを作成しました");

    // マイグレーションを実行
    let migration = DatabaseMigration::new(pool.clone());
    migration.run().await?;
    println!("データベースマイグレーションを実行しました");

    // MySQLリポジトリを作成
    let order_repository = Arc::new(MySqlOrderRepository::new(pool.clone()));
    let inventory_repository = Arc::new(MySqlInventoryRepository::new(pool.clone()));

    // イベントバスを作成
    let event_bus = Arc::new(InMemoryEventBus::new());

    // イベントハンドラーを作成して登録
    let inventory_handler = crate::domain::handler::InventoryReservationHandler::new(
        inventory_repository.clone(),
        order_repository.clone(),
        event_bus.clone(),
    );
    let notification_handler = crate::domain::handler::NotificationHandler::new();
    let consistency_verifier = crate::domain::handler::EventualConsistencyVerifier::new(
        order_repository.clone(),
        inventory_repository.clone(),
    );

    // 補償ハンドラーを作成
    let inventory_compensation_handler =
        crate::domain::handler::InventoryReservationFailureCompensationHandler::new(
            order_repository.clone(),
            event_bus.clone(),
        );
    let shipping_compensation_handler =
        crate::domain::handler::ShippingFailureCompensationHandler::new(
            inventory_repository.clone(),
            order_repository.clone(),
            event_bus.clone(),
        );
    let delivery_compensation_handler =
        crate::domain::handler::DeliveryFailureCompensationHandler::new(
            order_repository.clone(),
            event_bus.clone(),
        );
    let saga_coordinator =
        crate::domain::handler::SagaCompensationCoordinator::new(event_bus.clone());
    let compensation_completion_handler =
        crate::domain::handler::CompensationCompletionHandler::new();

    // イベントハンドラーをイベントバスに登録
    // 注文確定時は在庫予約のみ自動実行（発送・配達は手動操作）
    event_bus
        .subscribe_order_confirmed(inventory_handler)
        .await?;

    // 通知ハンドラーを各イベントに登録（並行処理で通知送信）
    event_bus
        .subscribe_order_confirmed(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_shipped(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_delivered(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_cancelled(notification_handler)
        .await?;

    // 整合性検証ハンドラーを登録（並行処理で検証実行）
    event_bus
        .subscribe_order_confirmed(consistency_verifier.clone())
        .await?;
    event_bus
        .subscribe_order_delivered(consistency_verifier)
        .await?;

    // 補償ハンドラーを登録
    event_bus
        .subscribe_inventory_reservation_failed(inventory_compensation_handler)
        .await?;
    event_bus
        .subscribe_shipping_failed(shipping_compensation_handler)
        .await?;
    event_bus
        .subscribe_delivery_failed(delivery_compensation_handler)
        .await?;
    event_bus
        .subscribe_saga_compensation_started(saga_coordinator)
        .await?;
    event_bus
        .subscribe_saga_compensation_completed(compensation_completion_handler)
        .await?;

    println!("イベントハンドラーを登録しました");
    println!("注文フロー:");
    println!("  1. 注文確定 → 在庫予約（自動）+ 通知送信");
    println!("  2. 注文発送 → 手動操作（POST /orders/:id/ship）");
    println!("  3. 配達完了 → 手動操作（POST /orders/:id/deliver）");

    // アプリケーションサービスを作成（Arcを外して渡す）
    let order_service =
        OrderApplicationService::new(MySqlOrderRepository::new(pool.clone()), event_bus.clone());

    // 在庫サービスを作成
    let inventory_service = InventoryApplicationService::new(inventory_repository.clone());

    // アプリケーション状態を作成
    let app_state = AppStateInner {
        order_service: Arc::new(order_service),
        inventory_service: Arc::new(inventory_service),
    };

    // REST APIルーターを作成
    let app = create_router()
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // サーバーを起動
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("REST APIサーバーが起動しました: http://localhost:3000");
    println!("ヘルスチェック: GET http://localhost:3000/health");
    println!("API仕様:");
    println!("  POST /orders - 注文作成");
    println!("  GET  /orders - 注文一覧取得");
    println!("  GET  /orders/:id - 注文詳細取得");
    println!("  POST /orders/:id/books - 本を注文に追加");
    println!("  PUT  /orders/:id/shipping-address - 配送先住所設定");
    println!("  POST /orders/:id/confirm - 注文確定");
    println!("  POST /orders/:id/cancel - 注文キャンセル");
    println!("  POST /orders/:id/ship - 注文発送");
    println!("  POST /orders/:id/deliver - 注文配達完了");
    println!("  POST /inventory - 在庫作成（テスト用）");
    println!("  GET  /inventory - 在庫一覧取得");
    println!("  GET  /inventory/:book_id - 在庫詳細取得");
    println!();

    axum::serve(listener, app).await?;

    Ok(())
}

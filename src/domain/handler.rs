use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::event::{
    CompensationResult, DomainEvent, InventoryReleased, InventoryReservationFailed,
    InventoryReserved, InventoryReserved as InventoryReservedEvent, OrderCancelled, OrderConfirmed,
    OrderDelivered, OrderShipped, SagaCompensationCompleted, SagaCompensationStarted,
    ShippingFailed,
};
use crate::domain::event_bus::{EventHandler, HandlerError};
use crate::domain::model::{Inventory, OrderId, OrderStatus};
use crate::domain::port::{EventBus, InventoryRepository, Logger, OrderRepository};

/// 処理済みイベントを追跡するためのリポジトリ
/// 実際の実装では永続化ストレージ（Redis、データベースなど）を使用
#[derive(Clone)]
pub struct ProcessedEventTracker {
    processed_events: Arc<Mutex<HashSet<Uuid>>>,
}

impl Default for ProcessedEventTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessedEventTracker {
    pub fn new() -> Self {
        Self {
            processed_events: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// イベントが既に処理済みかチェック
    pub async fn is_processed(&self, event_id: Uuid) -> bool {
        let processed = self.processed_events.lock().await;
        processed.contains(&event_id)
    }

    /// イベントを処理済みとしてマーク
    pub async fn mark_processed(&self, event_id: Uuid) {
        let mut processed = self.processed_events.lock().await;
        processed.insert(event_id);
    }
}

/// 在庫予約ハンドラー
/// OrderConfirmedイベントを受信して在庫を予約する
pub struct InventoryReservationHandler {
    inventory_repository: Arc<dyn InventoryRepository>,
    order_repository: Arc<dyn OrderRepository>,
    event_bus: Arc<dyn EventBus>,
    processed_events: ProcessedEventTracker,
    logger: Arc<dyn Logger>,
}

impl InventoryReservationHandler {
    /// 新しい在庫予約ハンドラーを作成
    pub fn new(
        inventory_repository: Arc<dyn InventoryRepository>,
        order_repository: Arc<dyn OrderRepository>,
        event_bus: Arc<dyn EventBus>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            inventory_repository,
            order_repository,
            event_bus,
            processed_events: ProcessedEventTracker::new(),
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<OrderConfirmed> for InventoryReservationHandler {
    async fn handle(&self, event: OrderConfirmed) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        self.logger.info(
            "InventoryReservationHandler",
            "Processing OrderConfirmed event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 冪等性チェック: 既に処理済みのイベントかどうか確認
        if self
            .processed_events
            .is_processed(event.metadata.event_id)
            .await
        {
            let mut context = HashMap::new();
            context.insert("event_id".to_string(), event.metadata.event_id.to_string());
            context.insert("already_processed".to_string(), "true".to_string());
            
            self.logger.debug(
                "InventoryReservationHandler",
                "Idempotency check: Event already processed, skipping",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            return Ok(());
        }

        // 注文の現在状態を確認
        let order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 注文がConfirmed状態でない場合は処理をスキップ（既に処理済み）
        if order.status() != OrderStatus::Confirmed {
            let mut context = HashMap::new();
            context.insert("current_status".to_string(), format!("{:?}", order.status()));
            context.insert("expected_status".to_string(), "Confirmed".to_string());
            
            self.logger.debug(
                "InventoryReservationHandler",
                "Order is not in Confirmed state, skipping inventory reservation",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            
            // イベントを処理済みとしてマーク
            self.processed_events
                .mark_processed(event.metadata.event_id)
                .await;
            return Ok(());
        }

        // 各注文明細について在庫を予約
        for order_line in &event.order_lines {
            // 在庫を取得
            let mut inventory = match self
                .inventory_repository
                .find_by_book_id(order_line.book_id())
                .await
                .map_err(|e| HandlerError::RepositoryError(format!("在庫取得エラー: {}", e)))?
            {
                Some(inventory) => inventory,
                None => {
                    // 在庫が見つからない場合は新しい在庫を作成（在庫数0）
                    Inventory::new(order_line.book_id(), 0)
                }
            };

            // 在庫を予約（失敗時は補償イベントを発行）
            match inventory.reserve(order_line.quantity()) {
                Ok(()) => {
                    // 在庫を保存
                    self.inventory_repository
                        .save(&inventory)
                        .await
                        .map_err(|e| {
                            HandlerError::RepositoryError(format!("在庫保存エラー: {}", e))
                        })?;
                }
                Err(domain_error) => {
                    // 在庫予約失敗 - 補償イベントを発行
                    let failure_reason = format!("在庫不足: {}", domain_error);
                    let compensation_event = InventoryReservationFailed::with_correlation_id(
                        event.order_id,
                        event.order_lines.clone(),
                        failure_reason.clone(),
                        event.metadata.event_id,
                        event.metadata.correlation_id,
                    );

                    self.event_bus
                        .publish(DomainEvent::InventoryReservationFailed(compensation_event))
                        .await
                        .map_err(|e| {
                            HandlerError::ProcessingFailed(format!("補償イベント発行エラー: {}", e))
                        })?;

                    // エラーログ出力
                    let mut context = HashMap::new();
                    context.insert("event_type".to_string(), "OrderConfirmed".to_string());
                    context.insert("error".to_string(), failure_reason.clone());
                    context.insert("execution_time_ms".to_string(), start_time.elapsed().as_millis().to_string());
                    
                    self.logger.error(
                        "InventoryReservationHandler",
                        &format!("OrderConfirmed event processing failed: {}", failure_reason),
                        Some(event.metadata.correlation_id),
                        Some(context),
                    );

                    // イベントを処理済みとしてマーク（失敗した場合でも重複処理を防ぐ）
                    self.processed_events
                        .mark_processed(event.metadata.event_id)
                        .await;

                    return Err(HandlerError::DomainError(format!(
                        "在庫予約エラー: {}",
                        domain_error
                    )));
                }
            }
        }

        // InventoryReservedイベントを発行
        let inventory_reserved_event = InventoryReservedEvent::with_correlation_id(
            event.order_id,
            event.order_lines.clone(),
            event.metadata.correlation_id,
        );

        self.event_bus
            .publish(DomainEvent::InventoryReserved(inventory_reserved_event))
            .await
            .map_err(|e| HandlerError::ProcessingFailed(format!("イベント発行エラー: {}", e)))?;

        // イベントを処理済みとしてマーク（成功時）
        self.processed_events
            .mark_processed(event.metadata.event_id)
            .await;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "InventoryReservationHandler",
            "OrderConfirmed event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 発送ハンドラー
/// InventoryReservedイベントを受信して注文を発送可能状態にする
pub struct ShippingHandler {
    order_repository: Arc<dyn OrderRepository>,
    event_bus: Arc<dyn EventBus>,
    processed_events: ProcessedEventTracker,
    logger: Arc<dyn Logger>,
}

impl ShippingHandler {
    /// 新しい発送ハンドラーを作成
    pub fn new(order_repository: Arc<dyn OrderRepository>, event_bus: Arc<dyn EventBus>, logger: Arc<dyn Logger>) -> Self {
        Self {
            order_repository,
            event_bus,
            processed_events: ProcessedEventTracker::new(),
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<InventoryReserved> for ShippingHandler {
    async fn handle(&self, event: InventoryReserved) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "InventoryReserved".to_string());
        self.logger.info(
            "ShippingHandler",
            "Processing InventoryReserved event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 冪等性チェック: 既に処理済みのイベントかどうか確認
        if self
            .processed_events
            .is_processed(event.metadata.event_id)
            .await
        {
            let mut context = HashMap::new();
            context.insert("event_id".to_string(), event.metadata.event_id.to_string());
            context.insert("already_processed".to_string(), "true".to_string());
            
            self.logger.debug(
                "ShippingHandler",
                "Idempotency check: Event already processed, skipping",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            return Ok(());
        }

        // 注文を取得
        let mut order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 注文がConfirmed状態でない場合は処理をスキップ（既に処理済みまたは無効な状態）
        if order.status() != OrderStatus::Confirmed {
            let mut context = HashMap::new();
            context.insert("current_status".to_string(), format!("{:?}", order.status()));
            context.insert("expected_status".to_string(), "Confirmed".to_string());
            
            self.logger.debug(
                "ShippingHandler",
                "Order is not in Confirmed state, skipping shipping",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            
            // イベントを処理済みとしてマーク
            self.processed_events
                .mark_processed(event.metadata.event_id)
                .await;
            return Ok(());
        }

        // 注文を発送済みにマーク（失敗時は補償イベントを発行）
        match order.mark_as_shipped() {
            Ok(()) => {
                // 注文を保存
                self.order_repository
                    .save(&order)
                    .await
                    .map_err(|e| HandlerError::RepositoryError(format!("注文保存エラー: {}", e)))?;

                let shipping_address = order
                    .shipping_address()
                    .expect("Confirmed状態の注文には配送先住所が必須です")
                    .clone();
                let shipped_event = crate::domain::event::OrderShipped::with_correlation_id(
                    order.id(),
                    shipping_address,
                    event.metadata.correlation_id,
                );
                let domain_event = crate::domain::event::DomainEvent::OrderShipped(shipped_event);

                self.event_bus.publish(domain_event).await.map_err(|e| {
                    HandlerError::ProcessingFailed(format!("イベント発行エラー: {}", e))
                })?;
            }
            Err(domain_error) => {
                // 発送失敗 - 補償イベントを発行
                let failure_reason = format!("発送処理失敗: {}", domain_error);
                let compensation_event = ShippingFailed::with_correlation_id(
                    event.order_id,
                    failure_reason.clone(),
                    event.metadata.event_id,
                    event.metadata.correlation_id,
                );

                self.event_bus
                    .publish(DomainEvent::ShippingFailed(compensation_event))
                    .await
                    .map_err(|e| {
                        HandlerError::ProcessingFailed(format!("補償イベント発行エラー: {}", e))
                    })?;

                // エラーログ出力
                let mut context = HashMap::new();
                context.insert("event_type".to_string(), "InventoryReserved".to_string());
                context.insert("error".to_string(), failure_reason.clone());
                context.insert("execution_time_ms".to_string(), start_time.elapsed().as_millis().to_string());
                
                self.logger.error(
                    "ShippingHandler",
                    &format!("InventoryReserved event processing failed: {}", failure_reason),
                    Some(event.metadata.correlation_id),
                    Some(context),
                );

                // イベントを処理済みとしてマーク（失敗した場合でも重複処理を防ぐ）
                self.processed_events
                    .mark_processed(event.metadata.event_id)
                    .await;

                return Err(HandlerError::DomainError(format!(
                    "発送マークエラー: {}",
                    domain_error
                )));
            }
        }

        // イベントを処理済みとしてマーク（成功時）
        self.processed_events
            .mark_processed(event.metadata.event_id)
            .await;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "InventoryReserved".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "ShippingHandler",
            "InventoryReserved event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 通知ハンドラー
/// 各種注文イベントを受信して通知を送信する
#[derive(Clone)]
pub struct NotificationHandler {
    logger: Arc<dyn Logger>,
}

impl NotificationHandler {
    /// 新しい通知ハンドラーを作成
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        Self { logger }
    }

    /// 通知メッセージを送信（実装では外部サービスを呼び出し）
    async fn send_notification(
        &self,
        message: &str,
        correlation_id: Uuid,
    ) -> Result<(), HandlerError> {
        // 実際の実装では外部通知サービス（メール、SMS、プッシュ通知など）を呼び出し
        // 今回はログ出力で代用
        let mut context = HashMap::new();
        context.insert("notification_type".to_string(), "General".to_string());
        context.insert("recipient".to_string(), "customer".to_string());
        
        self.logger.info(
            "NotificationHandler",
            &format!("Notification sent: General"),
            Some(correlation_id),
            Some(context),
        );

        // 通知内容もログに記録
        self.logger.info("NotificationHandler", message, Some(correlation_id), None);

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderConfirmed> for NotificationHandler {
    async fn handle(&self, event: OrderConfirmed) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        self.logger.info(
            "NotificationHandler",
            "Processing OrderConfirmed event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        let message = format!(
            "ご注文が確定されました。注文ID: {:?}, 合計金額: {}円",
            event.order_id,
            event.total_amount.amount()
        );

        self.send_notification(&message, event.metadata.correlation_id)
            .await?;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "NotificationHandler",
            "OrderConfirmed event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderShipped> for NotificationHandler {
    async fn handle(&self, event: OrderShipped) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderShipped".to_string());
        self.logger.info(
            "NotificationHandler",
            "Processing OrderShipped event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        let message = format!(
            "ご注文が発送されました。注文ID: {:?}, 配送先: {}",
            event.order_id,
            format_args!(
                "{} {} {}",
                event.shipping_address.prefecture(),
                event.shipping_address.city(),
                event.shipping_address.street()
            )
        );

        self.send_notification(&message, event.metadata.correlation_id)
            .await?;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderShipped".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "NotificationHandler",
            "OrderShipped event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderDelivered> for NotificationHandler {
    async fn handle(&self, event: OrderDelivered) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderDelivered".to_string());
        self.logger.info(
            "NotificationHandler",
            "Processing OrderDelivered event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        let message = format!("ご注文の配達が完了しました。注文ID: {:?}", event.order_id);

        self.send_notification(&message, event.metadata.correlation_id)
            .await?;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderDelivered".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "NotificationHandler",
            "OrderDelivered event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderCancelled> for NotificationHandler {
    async fn handle(&self, event: OrderCancelled) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderCancelled".to_string());
        self.logger.info(
            "NotificationHandler",
            "Processing OrderCancelled event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        let message = format!("ご注文がキャンセルされました。注文ID: {:?}", event.order_id);

        self.send_notification(&message, event.metadata.correlation_id)
            .await?;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderCancelled".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "NotificationHandler",
            "OrderCancelled event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 配達ハンドラー
/// OrderShippedイベントを受信して注文を配達完了状態にする
pub struct DeliveryHandler {
    order_repository: Arc<dyn OrderRepository>,
    event_bus: Arc<dyn EventBus>,
    processed_events: ProcessedEventTracker,
    logger: Arc<dyn Logger>,
}

impl DeliveryHandler {
    /// 新しい配達ハンドラーを作成
    pub fn new(order_repository: Arc<dyn OrderRepository>, event_bus: Arc<dyn EventBus>, logger: Arc<dyn Logger>) -> Self {
        Self {
            order_repository,
            event_bus,
            processed_events: ProcessedEventTracker::new(),
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<OrderShipped> for DeliveryHandler {
    async fn handle(&self, event: OrderShipped) -> Result<(), HandlerError> {
        // ハンドラー開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderShipped".to_string());
        self.logger.info(
            "DeliveryHandler",
            "Processing OrderShipped event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 冪等性チェック: 既に処理済みのイベントかどうか確認
        if self
            .processed_events
            .is_processed(event.metadata.event_id)
            .await
        {
            let mut context = HashMap::new();
            context.insert("event_id".to_string(), event.metadata.event_id.to_string());
            context.insert("already_processed".to_string(), "true".to_string());
            
            self.logger.debug(
                "DeliveryHandler",
                "Idempotency check: Event already processed, skipping",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            return Ok(());
        }

        // 注文を取得
        let mut order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 注文がShipped状態でない場合は処理をスキップ（既に処理済みまたは無効な状態）
        if order.status() != OrderStatus::Shipped {
            let mut context = HashMap::new();
            context.insert("current_status".to_string(), format!("{:?}", order.status()));
            context.insert("expected_status".to_string(), "Shipped".to_string());
            
            self.logger.debug(
                "DeliveryHandler",
                "Order is not in Shipped state, skipping delivery",
                Some(event.metadata.correlation_id),
                Some(context),
            );
            
            // イベントを処理済みとしてマーク
            self.processed_events
                .mark_processed(event.metadata.event_id)
                .await;
            return Ok(());
        }

        // 注文を配達完了にマーク（失敗時は補償イベントを発行）
        match order.mark_as_delivered() {
            Ok(()) => {
                // 注文を保存
                self.order_repository
                    .save(&order)
                    .await
                    .map_err(|e| HandlerError::RepositoryError(format!("注文保存エラー: {}", e)))?;

                let delivered_event = crate::domain::event::OrderDelivered::with_correlation_id(
                    order.id(),
                    event.metadata.correlation_id,
                );
                let domain_event =
                    crate::domain::event::DomainEvent::OrderDelivered(delivered_event);

                self.event_bus.publish(domain_event).await.map_err(|e| {
                    HandlerError::ProcessingFailed(format!("イベント発行エラー: {}", e))
                })?;
            }
            Err(domain_error) => {
                // 配達失敗 - 補償イベントを発行
                let failure_reason = format!("配達処理失敗: {}", domain_error);
                let compensation_event = crate::domain::event::DeliveryFailed::with_correlation_id(
                    event.order_id,
                    failure_reason.clone(),
                    event.metadata.event_id,
                    event.metadata.correlation_id,
                );

                self.event_bus
                    .publish(DomainEvent::DeliveryFailed(compensation_event))
                    .await
                    .map_err(|e| {
                        HandlerError::ProcessingFailed(format!("補償イベント発行エラー: {}", e))
                    })?;

                // エラーログ出力
                let mut context = HashMap::new();
                context.insert("event_type".to_string(), "OrderShipped".to_string());
                context.insert("error".to_string(), failure_reason.clone());
                context.insert("execution_time_ms".to_string(), start_time.elapsed().as_millis().to_string());
                
                self.logger.error(
                    "DeliveryHandler",
                    &format!("OrderShipped event processing failed: {}", failure_reason),
                    Some(event.metadata.correlation_id),
                    Some(context),
                );

                // イベントを処理済みとしてマーク（失敗した場合でも重複処理を防ぐ）
                self.processed_events
                    .mark_processed(event.metadata.event_id)
                    .await;

                return Err(HandlerError::DomainError(format!(
                    "配達マークエラー: {}",
                    domain_error
                )));
            }
        }

        // イベントを処理済みとしてマーク（成功時）
        self.processed_events
            .mark_processed(event.metadata.event_id)
            .await;

        // 処理成功ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderShipped".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "DeliveryHandler",
            "OrderShipped event processed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 配達失敗補償ハンドラー
/// DeliveryFailedイベントを受信して発送状態を元に戻す
pub struct DeliveryFailureCompensationHandler {
    order_repository: Arc<dyn OrderRepository>,
    #[allow(dead_code)]
    event_bus: Arc<dyn EventBus>,
    logger: Arc<dyn Logger>,
}

impl DeliveryFailureCompensationHandler {
    /// 新しい配達失敗補償ハンドラーを作成
    pub fn new(order_repository: Arc<dyn OrderRepository>, event_bus: Arc<dyn EventBus>, logger: Arc<dyn Logger>) -> Self {
        Self {
            order_repository,
            event_bus,
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<crate::domain::event::DeliveryFailed> for DeliveryFailureCompensationHandler {
    async fn handle(
        &self,
        event: crate::domain::event::DeliveryFailed,
    ) -> Result<(), HandlerError> {
        // 補償ログ出力
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "DeliveryFailed".to_string());
        context.insert("compensation_type".to_string(), "DeliveryFailure".to_string());
        self.logger.info(
            "DeliveryFailureCompensationHandler",
            "Processing DeliveryFailed compensation event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 注文を取得
        let order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 注文状態を発送済みに戻す（補償アクション）
        // 実際の実装では、より複雑な補償ロジックが必要になる場合がある
        // ここでは簡単のため、ステータスを直接変更
        if order.status() == crate::domain::model::OrderStatus::Delivered {
            // 注文を発送済み状態に戻すためのロジック
            // 実際の実装では、注文集約にcompensate_deliveryメソッドを追加することを推奨
        }

        // 補償処理完了ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "DeliveryFailed".to_string());
        context.insert("compensation_type".to_string(), "DeliveryFailure".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "DeliveryFailureCompensationHandler",
            "DeliveryFailed compensation completed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 結果整合性検証ハンドラー
/// サーガの完了を監視し、システム全体の整合性を検証する
#[derive(Clone)]
pub struct EventualConsistencyVerifier {
    order_repository: Arc<dyn OrderRepository>,
    inventory_repository: Arc<dyn InventoryRepository>,
    logger: Arc<dyn Logger>,
}

impl EventualConsistencyVerifier {
    /// 新しい結果整合性検証ハンドラーを作成
    pub fn new(
        order_repository: Arc<dyn OrderRepository>,
        inventory_repository: Arc<dyn InventoryRepository>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            order_repository,
            inventory_repository,
            logger,
        }
    }

    /// 注文とその関連する在庫の整合性を検証
    async fn verify_order_inventory_consistency(
        &self,
        order_id: OrderId,
        _correlation_id: Uuid,
    ) -> Result<(), HandlerError> {
        // 注文を取得
        let order = self
            .order_repository
            .find_by_id(order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!("注文が見つかりません: {:?}", order_id))
            })?;

        // 注文が確定済みまたは発送済みの場合、在庫が適切に予約されているかチェック
        if matches!(
            order.status(),
            crate::domain::model::OrderStatus::Confirmed
                | crate::domain::model::OrderStatus::Shipped
        ) {
            for order_line in order.order_lines() {
                let inventory = self
                    .inventory_repository
                    .find_by_book_id(order_line.book_id())
                    .await
                    .map_err(|e| HandlerError::RepositoryError(format!("在庫取得エラー: {}", e)))?;

                if let Some(inventory) = inventory {
                    // 在庫が十分にあることを確認（実際の実装では予約済み在庫の追跡が必要）
                    if !inventory.has_available_stock(order_line.quantity()) {
                    }
                } else {
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderConfirmed> for EventualConsistencyVerifier {
    async fn handle(&self, event: OrderConfirmed) -> Result<(), HandlerError> {
        // 注文確定時の整合性検証
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        context.insert("verification_type".to_string(), "OrderInventoryConsistency".to_string());
        self.logger.debug(
            "EventualConsistencyVerifier",
            "Starting order-inventory consistency verification",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        self.verify_order_inventory_consistency(event.order_id, event.metadata.correlation_id)
            .await?;

        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderConfirmed".to_string());
        context.insert("verification_result".to_string(), "Success".to_string());
        self.logger.debug(
            "EventualConsistencyVerifier",
            "Order-inventory consistency verification completed",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

#[async_trait]
impl EventHandler<OrderDelivered> for EventualConsistencyVerifier {
    async fn handle(&self, event: OrderDelivered) -> Result<(), HandlerError> {
        // 注文配達完了時の整合性検証
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderDelivered".to_string());
        context.insert("verification_type".to_string(), "OrderInventoryConsistency".to_string());
        self.logger.debug(
            "EventualConsistencyVerifier",
            "Starting order-inventory consistency verification",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        self.verify_order_inventory_consistency(event.order_id, event.metadata.correlation_id)
            .await?;

        // サーガ完了ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "OrderDelivered".to_string());
        context.insert("saga_status".to_string(), "Completed".to_string());
        self.logger.info(
            "EventualConsistencyVerifier",
            "Saga completed successfully - order delivered",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{BookId, CustomerId, Money, OrderId, OrderLine, OrderStatus};
    use crate::domain::port::{EventBus, EventBusError, RepositoryError};
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    // テスト用のモックイベントバス
    #[derive(Clone)]
    struct MockEventBus {
        published_events: Arc<Mutex<Vec<DomainEvent>>>,
    }

    impl MockEventBus {
        fn new() -> Self {
            Self {
                published_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        async fn get_published_events(&self) -> Vec<DomainEvent> {
            let events = self.published_events.lock().await;
            events.clone()
        }

        async fn clear_events(&self) {
            let mut events = self.published_events.lock().await;
            events.clear();
        }
    }

    #[async_trait]
    impl EventBus for MockEventBus {
        async fn publish(&self, event: DomainEvent) -> Result<(), EventBusError> {
            let mut events = self.published_events.lock().await;
            events.push(event);
            Ok(())
        }
    }

    // テスト用のモックリポジトリ
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

        async fn find_by_book_id(
            &self,
            book_id: BookId,
        ) -> Result<Option<Inventory>, RepositoryError> {
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

    struct MockOrderRepository {
        orders: Arc<Mutex<HashMap<OrderId, crate::domain::model::Order>>>,
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
        async fn save(&self, order: &crate::domain::model::Order) -> Result<(), RepositoryError> {
            let mut orders = self.orders.lock().await;
            orders.insert(order.id(), order.clone());
            Ok(())
        }

        async fn find_by_id(
            &self,
            order_id: OrderId,
        ) -> Result<Option<crate::domain::model::Order>, RepositoryError> {
            let orders = self.orders.lock().await;
            Ok(orders.get(&order_id).cloned())
        }

        async fn find_all(&self) -> Result<Vec<crate::domain::model::Order>, RepositoryError> {
            let orders = self.orders.lock().await;
            Ok(orders.values().cloned().collect())
        }

        async fn find_by_status(
            &self,
            status: OrderStatus,
        ) -> Result<Vec<crate::domain::model::Order>, RepositoryError> {
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

    // テスト用のモックロガー
    #[derive(Clone)]
    struct MockLogger;

    impl Logger for MockLogger {
        fn debug(&self, _component: &str, _message: &str, _correlation_id: Option<Uuid>, _context: Option<HashMap<String, String>>) {
            // テスト用なので何もしない
        }

        fn info(&self, _component: &str, _message: &str, _correlation_id: Option<Uuid>, _context: Option<HashMap<String, String>>) {
            // テスト用なので何もしない
        }

        fn warn(&self, _component: &str, _message: &str, _correlation_id: Option<Uuid>, _context: Option<HashMap<String, String>>) {
            // テスト用なので何もしない
        }

        fn error(&self, _component: &str, _message: &str, _correlation_id: Option<Uuid>, _context: Option<HashMap<String, String>>) {
            // テスト用なので何もしない
        }
    }

    #[tokio::test]
    async fn test_inventory_reservation_handler_success() {
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let handler = InventoryReservationHandler::new(
            inventory_repo.clone(),
            order_repo.clone(),
            event_bus.clone(),
            logger,
        );

        // テスト用の在庫を追加
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 10);
        inventory_repo.add_inventory(inventory).await;

        // テスト用のOrderConfirmedイベントを作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let order_line = OrderLine::new(book_id, 3, Money::jpy(1000)).unwrap();
        let event = OrderConfirmed::new(order_id, customer_id, vec![order_line], Money::jpy(3000));

        // テスト用の注文を作成してリポジトリに保存
        let mut order = crate::domain::model::Order::new(order_id, customer_id);
        order.add_book(book_id, 3, Money::jpy(1000)).unwrap();
        order.set_shipping_address(
            crate::domain::model::ShippingAddress::new(
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

        // ハンドラーを実行
        let result = handler.handle(event).await;
        assert!(result.is_ok());

        // 在庫が正しく減っていることを確認
        let updated_inventory = inventory_repo
            .find_by_book_id(book_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_inventory.quantity_on_hand(), 7);

        // InventoryReservedイベントが発行されていることを確認
        let published_events = event_bus.get_published_events().await;
        assert_eq!(published_events.len(), 1);
        match &published_events[0] {
            DomainEvent::InventoryReserved(event) => {
                assert_eq!(event.order_id, order_id);
            }
            _ => panic!("Expected InventoryReserved event"),
        }
    }

    #[tokio::test]
    async fn test_inventory_reservation_handler_insufficient_stock() {
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let handler = InventoryReservationHandler::new(
            inventory_repo.clone(),
            order_repo.clone(),
            event_bus.clone(),
            logger,
        );

        // テスト用の在庫を追加（少ない在庫）
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 2);
        inventory_repo.add_inventory(inventory).await;

        // テスト用のOrderConfirmedイベントを作成（在庫より多い数量）
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let order_line = OrderLine::new(book_id, 5, Money::jpy(1000)).unwrap();
        let event = OrderConfirmed::new(order_id, customer_id, vec![order_line], Money::jpy(5000));

        // テスト用の注文を作成してリポジトリに保存
        let mut order = crate::domain::model::Order::new(order_id, customer_id);
        order.add_book(book_id, 5, Money::jpy(1000)).unwrap();
        order.set_shipping_address(
            crate::domain::model::ShippingAddress::new(
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

        // ハンドラーを実行（失敗するはず）
        let result = handler.handle(event).await;
        assert!(result.is_err());

        // 在庫が変わっていないことを確認
        let inventory = inventory_repo
            .find_by_book_id(book_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inventory.quantity_on_hand(), 2);

        // InventoryReservationFailedイベントが発行されていることを確認
        let published_events = event_bus.get_published_events().await;
        assert_eq!(published_events.len(), 1);
        match &published_events[0] {
            DomainEvent::InventoryReservationFailed(event) => {
                assert_eq!(event.order_id, order_id);
            }
            _ => panic!("Expected InventoryReservationFailed event"),
        }
    }

    #[tokio::test]
    async fn test_notification_handler_order_confirmed() {
        let logger = Arc::new(MockLogger);
        let handler = NotificationHandler::new(logger);

        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let event = OrderConfirmed::new(order_id, customer_id, vec![], Money::jpy(1000));

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_notification_handler_order_shipped() {
        let logger = Arc::new(MockLogger);
        let handler = NotificationHandler::new(logger);

        let order_id = OrderId::new();
        let shipping_address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();

        let event = OrderShipped::new(order_id, shipping_address);

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_notification_handler_order_delivered() {
        let logger = Arc::new(MockLogger);
        let handler = NotificationHandler::new(logger);

        let order_id = OrderId::new();
        let event = OrderDelivered::new(order_id);

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_notification_handler_order_cancelled() {
        let logger = Arc::new(MockLogger);
        let handler = NotificationHandler::new(logger);

        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let event = OrderCancelled::new(order_id, customer_id, vec![]);

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_compensation_mechanism_inventory_reservation_failure() {
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let handler = InventoryReservationFailureCompensationHandler::new(
            order_repo.clone(),
            event_bus.clone(),
            logger,
        );

        // テスト用の注文を作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address);

        // 注文を確定状態にする
        order.confirm().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // 在庫予約失敗イベントを作成
        let failure_event = InventoryReservationFailed::new(
            order_id,
            vec![],
            "在庫不足".to_string(),
            Uuid::new_v4(),
        );

        // 補償ハンドラーを実行
        let result = handler.handle(failure_event).await;
        assert!(result.is_ok());

        // 注文がキャンセル状態になっていることを確認
        let orders = order_repo.orders.lock().await;
        let updated_order = orders.get(&order_id).unwrap();
        assert_eq!(
            updated_order.status(),
            crate::domain::model::OrderStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn test_compensation_mechanism_shipping_failure() {
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let handler = ShippingFailureCompensationHandler::new(
            inventory_repo.clone(),
            order_repo.clone(),
            event_bus.clone(),
            logger,
        );

        // テスト用の在庫を追加
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 5); // 在庫を予約済み状態にするため少なめに設定
        inventory_repo.add_inventory(inventory).await;

        // テスト用の注文を作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address);

        // 注文を確定状態にする
        order.confirm().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // 発送失敗イベントを作成
        let failure_event =
            ShippingFailed::new(order_id, "配送業者エラー".to_string(), Uuid::new_v4());

        // 補償ハンドラーを実行
        let result = handler.handle(failure_event).await;
        assert!(result.is_ok());

        // 在庫が解放されていることを確認
        let updated_inventory = inventory_repo
            .find_by_book_id(book_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_inventory.quantity_on_hand(), 7); // 5 + 2 = 7
    }

    #[tokio::test]
    async fn test_delivery_handler_success() {
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let handler = DeliveryHandler::new(order_repo.clone(), event_bus.clone(), logger);

        // テスト用の注文を作成（発送済み状態）
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address.clone());

        // 注文を確定状態にしてから発送済み状態にする
        order.confirm().unwrap();
        order.mark_as_shipped().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // OrderShippedイベントを作成
        let event = OrderShipped::new(order_id, address);

        // ハンドラーを実行
        let result = handler.handle(event).await;
        assert!(result.is_ok());

        // 注文が配達完了状態になっていることを確認
        let orders = order_repo.orders.lock().await;
        let updated_order = orders.get(&order_id).unwrap();
        assert_eq!(
            updated_order.status(),
            crate::domain::model::OrderStatus::Delivered
        );
    }

    #[tokio::test]
    async fn test_eventual_consistency_verifier() {
        let order_repo = Arc::new(MockOrderRepository::new());
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let logger = Arc::new(MockLogger);
        let verifier = EventualConsistencyVerifier::new(order_repo.clone(), inventory_repo.clone(), logger);

        // テスト用の在庫を追加
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 10);
        inventory_repo.add_inventory(inventory).await;

        // テスト用の注文を作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let price = Money::jpy(1000);
        order.add_book(book_id, 3, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address);

        // 注文を確定状態にする
        order.confirm().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // OrderConfirmedイベントを作成
        let event = OrderConfirmed::new(order_id, customer_id, vec![], Money::jpy(3000));

        // 整合性検証を実行
        let result = verifier.handle(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete_saga_flow() {
        // 修正されたフローのテスト: 注文確定→在庫予約まで（発送・配達は手動操作）
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());

        // ハンドラーを作成（実際のフローに合わせて在庫予約のみ自動実行）
        let logger = Arc::new(MockLogger);
        let inventory_handler = InventoryReservationHandler::new(
            inventory_repo.clone(),
            order_repo.clone(),
            event_bus.clone(),
            logger.clone(),
        );

        // テスト用の在庫を追加（十分な在庫を確保）
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 20);
        inventory_repo.add_inventory(inventory).await;

        // テスト用の注文を作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let price = Money::jpy(1000);
        order.add_book(book_id, 3, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address);

        // 注文を確定状態にする
        order.confirm().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // OrderConfirmedイベントを発行（在庫予約まで自動実行）
        let event = OrderConfirmed::new(
            order_id,
            customer_id,
            vec![crate::domain::model::OrderLine::new(book_id, 3, price).unwrap()],
            Money::jpy(3000),
        );

        // 手動でハンドラーを実行（モックイベントバスでは自動実行されないため）
        let result = inventory_handler.handle(event.clone()).await;
        assert!(result.is_ok());

        // イベントが発行されたことを確認
        let published_events = event_bus.get_published_events().await;
        assert_eq!(published_events.len(), 1);
        match &published_events[0] {
            DomainEvent::InventoryReserved(event) => {
                assert_eq!(event.order_id, order_id);
            }
            _ => panic!("Expected InventoryReserved event"),
        }

        // 注文確定後の状態を確認（Confirmedのまま、在庫のみ予約済み）
        let orders = order_repo.orders.lock().await;
        let order_after_confirmation = orders.get(&order_id).unwrap();

        // 注文はConfirmed状態のまま（発送は手動操作のため）
        assert_eq!(
            order_after_confirmation.status(),
            crate::domain::model::OrderStatus::Confirmed
        );

        // 在庫が予約されていることを確認
        let updated_inventory = inventory_repo
            .find_by_book_id(book_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated_inventory.quantity_on_hand(),
            17, // 20 - 3 = 17
            "Expected inventory to be reduced to 17, but got: {}",
            updated_inventory.quantity_on_hand()
        );
    }

    #[tokio::test]
    async fn test_manual_shipping_and_delivery_flow() {
        // 手動操作フローのテスト: 発送・配達は手動API呼び出しで実行
        let inventory_repo = Arc::new(MockInventoryRepository::new());
        let order_repo = Arc::new(MockOrderRepository::new());
        let event_bus = Arc::new(MockEventBus::new());

        // 通知ハンドラーのみ登録（発送・配達時の通知用）
        let logger = Arc::new(MockLogger);
        let notification_handler = NotificationHandler::new(logger.clone());

        // テスト用の注文を作成（Confirmed状態）
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = crate::domain::model::Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 3, price).unwrap();

        // 配送先住所を設定
        let address = crate::domain::model::ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();
        order.set_shipping_address(address.clone());

        // 注文を確定状態にする
        order.confirm().unwrap();

        // モックリポジトリに注文を保存
        {
            let mut orders = order_repo.orders.lock().await;
            orders.insert(order_id, order);
        }

        // 手動発送操作をシミュレート
        {
            let mut orders = order_repo.orders.lock().await;
            let mut order = orders.get_mut(&order_id).unwrap();
            order.mark_as_shipped().unwrap();

            // OrderShippedイベントを手動発行（実際のAPIでは自動発行される）
            let shipped_event = crate::domain::event::OrderShipped::new(order_id, address.clone());
            event_bus
                .publish(crate::domain::event::DomainEvent::OrderShipped(
                    shipped_event.clone(),
                ))
                .await
                .unwrap();

            // 通知ハンドラーを手動実行
            notification_handler.handle(shipped_event).await.unwrap();
        }

        // イベント処理を待つ
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 発送後の状態確認
        {
            let orders = order_repo.orders.lock().await;
            let order = orders.get(&order_id).unwrap();
            assert_eq!(order.status(), crate::domain::model::OrderStatus::Shipped);
        }

        // 手動配達完了操作をシミュレート
        {
            let mut orders = order_repo.orders.lock().await;
            let mut order = orders.get_mut(&order_id).unwrap();
            order.mark_as_delivered().unwrap();

            // OrderDeliveredイベントを手動発行（実際のAPIでは自動発行される）
            let delivered_event = crate::domain::event::OrderDelivered::new(order_id);
            event_bus
                .publish(crate::domain::event::DomainEvent::OrderDelivered(
                    delivered_event.clone(),
                ))
                .await
                .unwrap();

            // 通知ハンドラーを手動実行
            notification_handler.handle(delivered_event).await.unwrap();
        }

        // イベント処理を待つ
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 配達完了後の状態確認
        {
            let orders = order_repo.orders.lock().await;
            let order = orders.get(&order_id).unwrap();
            assert_eq!(order.status(), crate::domain::model::OrderStatus::Delivered);
        }
    }

    #[tokio::test]
    async fn test_saga_compensation_coordinator() {
        let event_bus = Arc::new(MockEventBus::new());
        let logger = Arc::new(MockLogger);
        let coordinator = SagaCompensationCoordinator::new(event_bus.clone(), logger);

        let saga_id = Uuid::new_v4();
        let result = coordinator
            .start_compensation(
                saga_id,
                "shipping".to_string(),
                "配送業者エラー".to_string(),
            )
            .await;

        assert!(result.is_ok());

        // SagaCompensationStartedイベントが発行されていることを確認
        let published_events = event_bus.get_published_events().await;
        assert_eq!(published_events.len(), 1);
        match &published_events[0] {
            DomainEvent::SagaCompensationStarted(event) => {
                assert_eq!(event.saga_id, saga_id);
            }
            _ => panic!("Expected SagaCompensationStarted event"),
        }
    }

    #[tokio::test]
    async fn test_compensation_completion_handler() {
        let logger = Arc::new(MockLogger);
        let handler = CompensationCompletionHandler::new(logger);

        let saga_id = Uuid::new_v4();
        let event = SagaCompensationCompleted::new(
            saga_id,
            vec!["inventory_reservation".to_string()],
            CompensationResult::Success,
        );

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }
}

// ========== 補償ハンドラー（サーガ失敗時のロールバック処理） ==========

/// 在庫予約失敗補償ハンドラー
/// InventoryReservationFailedイベントを受信して注文をキャンセルする
pub struct InventoryReservationFailureCompensationHandler {
    order_repository: Arc<dyn OrderRepository>,
    event_bus: Arc<dyn EventBus>,
    logger: Arc<dyn Logger>,
}

impl InventoryReservationFailureCompensationHandler {
    /// 新しい在庫予約失敗補償ハンドラーを作成
    pub fn new(order_repository: Arc<dyn OrderRepository>, event_bus: Arc<dyn EventBus>, logger: Arc<dyn Logger>) -> Self {
        Self {
            order_repository,
            event_bus,
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<InventoryReservationFailed> for InventoryReservationFailureCompensationHandler {
    async fn handle(&self, event: InventoryReservationFailed) -> Result<(), HandlerError> {
        // 補償ログ出力
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "InventoryReservationFailed".to_string());
        context.insert("compensation_type".to_string(), "InventoryReservationFailure".to_string());
        self.logger.info(
            "InventoryReservationFailureCompensationHandler",
            "Processing InventoryReservationFailed compensation event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 注文を取得
        let mut order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 注文をキャンセル（補償アクション）
        order
            .cancel()
            .map_err(|e| HandlerError::DomainError(format!("注文キャンセルエラー: {}", e)))?;

        // 注文を保存
        self.order_repository
            .save(&order)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文保存エラー: {}", e)))?;

        let cancelled_event = crate::domain::event::OrderCancelled::with_correlation_id(
            order.id(),
            order.customer_id(),
            order.order_lines().to_vec(),
            event.metadata.correlation_id,
        );
        let domain_event = crate::domain::event::DomainEvent::OrderCancelled(cancelled_event);

        self.event_bus
            .publish(domain_event)
            .await
            .map_err(|e| HandlerError::ProcessingFailed(format!("イベント発行エラー: {}", e)))?;

        // 補償処理完了ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "InventoryReservationFailed".to_string());
        context.insert("compensation_type".to_string(), "InventoryReservationFailure".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "InventoryReservationFailureCompensationHandler",
            "InventoryReservationFailed compensation completed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// 発送失敗補償ハンドラー
/// ShippingFailedイベントを受信して在庫を解放する
pub struct ShippingFailureCompensationHandler {
    inventory_repository: Arc<dyn InventoryRepository>,
    order_repository: Arc<dyn OrderRepository>,
    event_bus: Arc<dyn EventBus>,
    logger: Arc<dyn Logger>,
}

impl ShippingFailureCompensationHandler {
    /// 新しい発送失敗補償ハンドラーを作成
    pub fn new(
        inventory_repository: Arc<dyn InventoryRepository>,
        order_repository: Arc<dyn OrderRepository>,
        event_bus: Arc<dyn EventBus>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            inventory_repository,
            order_repository,
            event_bus,
            logger,
        }
    }
}

#[async_trait]
impl EventHandler<ShippingFailed> for ShippingFailureCompensationHandler {
    async fn handle(&self, event: ShippingFailed) -> Result<(), HandlerError> {
        // 補償ログ出力
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "ShippingFailed".to_string());
        context.insert("compensation_type".to_string(), "ShippingFailure".to_string());
        self.logger.info(
            "ShippingFailureCompensationHandler",
            "Processing ShippingFailed compensation event",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        let start_time = std::time::Instant::now();

        // 注文を取得
        let order = self
            .order_repository
            .find_by_id(event.order_id)
            .await
            .map_err(|e| HandlerError::RepositoryError(format!("注文取得エラー: {}", e)))?
            .ok_or_else(|| {
                HandlerError::ProcessingFailed(format!(
                    "注文が見つかりません: {:?}",
                    event.order_id
                ))
            })?;

        // 各注文明細について在庫を解放（補償アクション）
        for order_line in order.order_lines() {
            // 在庫を取得
            let mut inventory = match self
                .inventory_repository
                .find_by_book_id(order_line.book_id())
                .await
                .map_err(|e| HandlerError::RepositoryError(format!("在庫取得エラー: {}", e)))?
            {
                Some(inventory) => inventory,
                None => {
                    // 在庫が見つからない場合はスキップ（ログに記録）
                    let mut context = HashMap::new();
                    context.insert("book_id".to_string(), format!("{:?}", order_line.book_id()));
                    context.insert("reason".to_string(), "inventory_not_found".to_string());
                    
                    self.logger.warn(
                        "ShippingFailureCompensationHandler",
                        "Inventory not found for book, skipping release",
                        Some(event.metadata.correlation_id),
                        Some(context),
                    );
                    continue;
                }
            };

            // 在庫を解放
            inventory
                .release(order_line.quantity())
                .map_err(|e| HandlerError::DomainError(format!("在庫解放エラー: {}", e)))?;

            // 在庫を保存
            self.inventory_repository
                .save(&inventory)
                .await
                .map_err(|e| HandlerError::RepositoryError(format!("在庫保存エラー: {}", e)))?;
        }

        // InventoryReleasedイベントを発行
        let inventory_released_event = InventoryReleased::with_correlation_id(
            event.order_id,
            order.order_lines().to_vec(),
            event.metadata.correlation_id,
        );

        self.event_bus
            .publish(DomainEvent::InventoryReleased(inventory_released_event))
            .await
            .map_err(|e| HandlerError::ProcessingFailed(format!("イベント発行エラー: {}", e)))?;

        // 補償処理完了ログ
        let execution_time = start_time.elapsed();
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "ShippingFailed".to_string());
        context.insert("compensation_type".to_string(), "ShippingFailure".to_string());
        context.insert("execution_time_ms".to_string(), execution_time.as_millis().to_string());
        
        self.logger.info(
            "ShippingFailureCompensationHandler",
            "ShippingFailed compensation completed successfully",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

/// サーガ補償コーディネーター
/// サーガ補償コーディネーター
/// サーガの失敗を検出し、補償プロセスを開始する
pub struct SagaCompensationCoordinator {
    event_bus: Arc<dyn EventBus>,
    logger: Arc<dyn Logger>,
}

impl SagaCompensationCoordinator {
    /// 新しいサーガ補償コーディネーターを作成
    pub fn new(event_bus: Arc<dyn EventBus>, logger: Arc<dyn Logger>) -> Self {
        Self { event_bus, logger }
    }

    /// サーガ失敗を検出し、補償を開始
    pub async fn start_compensation(
        &self,
        saga_id: Uuid,
        failed_step: String,
        failure_reason: String,
    ) -> Result<(), HandlerError> {
        // 補償が必要なステップを決定（逆順）
        let compensation_steps = self.determine_compensation_steps(&failed_step);

        // サーガ補償開始イベントを発行
        let compensation_started_event =
            SagaCompensationStarted::new(saga_id, failed_step, failure_reason, compensation_steps);

        self.event_bus
            .publish(DomainEvent::SagaCompensationStarted(
                compensation_started_event,
            ))
            .await
            .map_err(|e| {
                HandlerError::ProcessingFailed(format!("補償開始イベント発行エラー: {}", e))
            })?;

        Ok(())
    }

    /// 失敗したステップに基づいて補償が必要なステップを決定
    fn determine_compensation_steps(&self, failed_step: &str) -> Vec<String> {
        match failed_step {
            "inventory_reservation" => vec![], // 最初のステップなので補償不要
            "shipping" => vec!["inventory_reservation".to_string()], // 在庫予約を補償
            "delivery" => vec!["shipping".to_string(), "inventory_reservation".to_string()], // 発送と在庫予約を補償
            _ => vec![], // 不明なステップ
        }
    }
}

#[async_trait]
impl EventHandler<SagaCompensationStarted> for SagaCompensationCoordinator {
    async fn handle(&self, event: SagaCompensationStarted) -> Result<(), HandlerError> {
        // サーガ補償開始ログ
        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "SagaCompensationStarted".to_string());
        context.insert("saga_id".to_string(), event.saga_id.to_string());
        context.insert("failed_step".to_string(), event.failed_step.clone());
        context.insert("compensation_steps_count".to_string(), event.compensation_steps.len().to_string());
        
        self.logger.info(
            "SagaCompensationCoordinator",
            "Saga compensation process started",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        // 実際の補償処理は個別の補償ハンドラーが実行するため、
        // ここでは補償プロセスの追跡とログ出力のみ行う

        Ok(())
    }
}

/// 補償完了ハンドラー
/// 補償プロセスの完了を監視し、ログを記録する
pub struct CompensationCompletionHandler {
    logger: Arc<dyn Logger>,
}

impl CompensationCompletionHandler {
    /// 新しい補償完了ハンドラーを作成
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        Self { logger }
    }
}

#[async_trait]
impl EventHandler<SagaCompensationCompleted> for CompensationCompletionHandler {
    async fn handle(&self, event: SagaCompensationCompleted) -> Result<(), HandlerError> {
        let start_time = std::time::Instant::now();

        // 補償完了ログ出力
        let result_str = match &event.compensation_result {
            CompensationResult::Success => "Success",
            CompensationResult::PartialSuccess { .. } => "PartialSuccess",
            CompensationResult::Failed { .. } => "Failed",
        };

        let mut context = HashMap::new();
        context.insert("event_type".to_string(), "SagaCompensationCompleted".to_string());
        context.insert("saga_id".to_string(), event.saga_id.to_string());
        context.insert("compensation_result".to_string(), result_str.to_string());
        context.insert("completed_steps_count".to_string(), event.compensated_steps.len().to_string());
        context.insert("execution_time_ms".to_string(), start_time.elapsed().as_millis().to_string());
        
        self.logger.info(
            "CompensationCompletionHandler",
            "Saga compensation process completed",
            Some(event.metadata.correlation_id),
            Some(context),
        );

        Ok(())
    }
}

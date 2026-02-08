use crate::domain::event::DomainEvent;
use crate::domain::event_bus::{
    DeliveryFailedHandlerWrapper, DynEventHandler, EventHandler, HandlerError,
    InventoryReservationFailedHandlerWrapper,
    OrderCancelledHandlerWrapper, OrderConfirmedHandlerWrapper,
    OrderDeliveredHandlerWrapper, OrderShippedHandlerWrapper,
    SagaCompensationCompletedHandlerWrapper, SagaCompensationStartedHandlerWrapper,
    ShippingFailedHandlerWrapper,
};
use crate::domain::port::{EventBus, EventBusError};
use crate::domain::serialization::EventSerializer;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};

/// 失敗したイベント処理の情報
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FailedEventProcessing {
    pub event: DomainEvent,
    pub handler_name: String,
    pub error: String,
    pub attempt_count: u32,
    pub first_failed_at: SystemTime,
    pub last_failed_at: SystemTime,
    pub is_retryable: bool,
}

/// デッドレターキューエントリ
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeadLetterEntry {
    pub failed_processing: FailedEventProcessing,
    pub added_at: SystemTime,
}

/// イベントバス設定
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// 最大リトライ回数
    pub max_retry_attempts: u32,
    /// リトライ間隔
    pub retry_delay: Duration,
    /// デッドレターキューの最大サイズ
    pub dead_letter_queue_max_size: usize,
    /// ハンドラータイムアウト
    pub handler_timeout: Duration,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 3,
            retry_delay: Duration::from_millis(1000),
            dead_letter_queue_max_size: 1000,
            handler_timeout: Duration::from_secs(30),
        }
    }
}

/// インメモリイベントバス実装
/// 開発・テスト用の高度な機能を持つ実装
pub struct InMemoryEventBus {
    handlers: Arc<RwLock<Vec<Box<dyn DynEventHandler>>>>,
    dead_letter_queue: Arc<Mutex<VecDeque<DeadLetterEntry>>>,
    config: EventBusConfig,
    serializer: EventSerializer,
}

impl InMemoryEventBus {
    /// 設定を指定してインメモリイベントバスを作成
    /// 
    /// # 例
    /// ```
    /// use bookstore_order_management::adapter::driven::{InMemoryEventBus, EventBusConfig};
    /// 
    /// // デフォルト設定で作成
    /// let event_bus = InMemoryEventBus::new(EventBusConfig::default());
    /// 
    /// // カスタム設定で作成
    /// let config = EventBusConfig {
    ///     max_retry_attempts: 5,
    ///     retry_delay: std::time::Duration::from_millis(100),
    ///     ..EventBusConfig::default()
    /// };
    /// let event_bus = InMemoryEventBus::new(config);
    /// ```
    pub fn new(config: EventBusConfig) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
            dead_letter_queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            serializer: EventSerializer::new(),
        }
    }

    /// ハンドラーの実行（エラー処理とリトライ機能付き）
    async fn execute_handler_with_retry(
        &self,
        handler: &dyn DynEventHandler,
        event: &DomainEvent,
    ) -> Result<(), HandlerError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.config.max_retry_attempts {
            attempts += 1;

            // スキーマバージョンの互換性チェック
            let event_version = event.metadata().event_version;
            if !handler.supports_schema_version(event_version) {
                return Err(HandlerError::PermanentError(format!(
                    "Handler {} does not support schema version {}",
                    handler.handler_name(),
                    event_version
                )));
            }

            // タイムアウト付きでハンドラーを実行
            let result =
                tokio::time::timeout(self.config.handler_timeout, handler.handle_event(event))
                    .await;

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(handler_error)) => {
                    last_error = Some(handler_error.clone());

                    // 永続的エラーの場合はリトライしない
                    if matches!(handler_error, HandlerError::PermanentError(_)) {
                        break;
                    }

                    // 最後の試行でない場合は待機
                    if attempts < self.config.max_retry_attempts {
                        tokio::time::sleep(self.config.retry_delay).await;
                    }
                }
                Err(_timeout_error) => {
                    last_error = Some(HandlerError::TransientError("Handler timeout".to_string()));

                    // 最後の試行でない場合は待機
                    if attempts < self.config.max_retry_attempts {
                        tokio::time::sleep(self.config.retry_delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(HandlerError::ProcessingFailed("Unknown error".to_string())))
    }

    /// 失敗したイベントをデッドレターキューに追加
    async fn add_to_dead_letter_queue(
        &self,
        event: DomainEvent,
        handler_name: String,
        error: &HandlerError,
    ) -> Result<(), EventBusError> {
        let mut dlq = self.dead_letter_queue.lock().await;

        // キューサイズの制限チェック
        if dlq.len() >= self.config.dead_letter_queue_max_size {
            dlq.pop_front(); // 古いエントリを削除
        }

        let is_retryable = matches!(error, HandlerError::TransientError(_));
        let now = SystemTime::now();

        let failed_processing = FailedEventProcessing {
            event: event.clone(),
            handler_name: handler_name.clone(),
            error: error.to_string(),
            attempt_count: self.config.max_retry_attempts,
            first_failed_at: now,
            last_failed_at: now,
            is_retryable,
        };

        let entry = DeadLetterEntry {
            failed_processing,
            added_at: now,
        };

        dlq.push_back(entry);
        Ok(())
    }

    /// イベントのシリアライゼーション検証
    fn validate_event_serialization(&self, event: &DomainEvent) -> Result<(), EventBusError> {
        // シリアライゼーションテストを実行
        match self.serializer.serialize_event(event) {
            Ok(json) => {
                // デシリアライゼーションテストも実行（往復テスト）
                match self.serializer.deserialize_event(&json) {
                    Ok(_) => Ok(()),
                    Err(serialization_error) => {
                        // Note: Logger trait is not available in this context as it would create circular dependency
                        // Serialization errors should be handled at the application layer
                        Err(EventBusError::PublishingFailed(format!(
                            "Serialization error: {}",
                            serialization_error
                        )))
                    }
                }
            }
            Err(serialization_error) => {
                // Note: Logger trait is not available in this context as it would create circular dependency
                // Serialization errors should be handled at the application layer
                Err(EventBusError::PublishingFailed(format!(
                    "Serialization error: {}",
                    serialization_error
                )))
            }
        }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new(EventBusConfig::default())
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), EventBusError> {
        // シリアライゼーション検証
        self.validate_event_serialization(&event)?;

        // イベント発行ログ
        // Note: Logger trait is not available in this context as it would create circular dependency
        // Individual handlers log their own processing

        // ハンドラー情報を収集
        let handlers = {
            let handlers_guard = self.handlers.read().await;
            let mut applicable_handlers = Vec::new();

            for handler in handlers_guard.iter() {
                if handler.can_handle(&event) {
                    applicable_handlers.push((
                        handler.handler_name().to_string(),
                        handler.supports_schema_version(event.metadata().event_version),
                    ));
                }
            }
            applicable_handlers
        };

        // 各ハンドラーを順次処理
        for (handler_name, supports_version) in handlers {
            if !supports_version {
                let error = HandlerError::PermanentError(format!(
                    "Handler {} does not support schema version {}",
                    handler_name,
                    event.metadata().event_version
                ));

                // エラーログ
                // Note: Logger trait is not available in this context as it would create circular dependency
                // Individual handlers should log their own failures

                if let Err(dlq_error) = self
                    .add_to_dead_letter_queue(event.clone(), handler_name.clone(), &error)
                    .await
                {
                    // Note: Logger trait is not available in this context as it would create circular dependency
                    // DLQ errors are handled silently to prevent infinite loops
                    let _ = dlq_error; // Acknowledge the error without logging
                }
                continue;
            }

            // ハンドラーを名前で実行
            match self.execute_handler_by_name(&handler_name, &event).await {
                Ok(()) => {
                    // 成功ログは個別のハンドラー内で出力される
                }
                Err(handler_error) => {
                    // Note: Logger trait is not available in this context as it would create circular dependency
                    // Individual handlers should log their own failures

                    if let Err(dlq_error) = self
                        .add_to_dead_letter_queue(
                            event.clone(),
                            handler_name.clone(),
                            &handler_error,
                        )
                        .await
                    {
                        // Note: Logger trait is not available in this context as it would create circular dependency
                        // DLQ errors are handled silently to prevent infinite loops
                        let _ = dlq_error; // Acknowledge the error without logging
                    }
                }
            }
        }

        Ok(())
    }
}

impl InMemoryEventBus {
    /// 名前でハンドラーを実行
    async fn execute_handler_by_name(
        &self,
        handler_name: &str,
        event: &DomainEvent,
    ) -> Result<(), HandlerError> {
        let handlers = self.handlers.read().await;

        for handler in handlers.iter() {
            if handler.handler_name() == handler_name && handler.can_handle(event) {
                return self
                    .execute_handler_with_retry(handler.as_ref(), event)
                    .await;
            }
        }

        Err(HandlerError::ProcessingFailed(format!(
            "Handler {} not found",
            handler_name
        )))
    }

    /// OrderConfirmedハンドラーを登録
    pub async fn subscribe_order_confirmed<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::OrderConfirmed> + Send + Sync + 'static,
    {
        let wrapped_handler = OrderConfirmedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }



    /// OrderCancelledハンドラーを登録
    pub async fn subscribe_order_cancelled<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::OrderCancelled> + Send + Sync + 'static,
    {
        let wrapped_handler = OrderCancelledHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// OrderShippedハンドラーを登録
    pub async fn subscribe_order_shipped<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::OrderShipped> + Send + Sync + 'static,
    {
        let wrapped_handler = OrderShippedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// OrderDeliveredハンドラーを登録
    pub async fn subscribe_order_delivered<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::OrderDelivered> + Send + Sync + 'static,
    {
        let wrapped_handler = OrderDeliveredHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    // ========== 補償イベント用の登録メソッド ==========

    /// InventoryReservationFailedハンドラーを登録
    pub async fn subscribe_inventory_reservation_failed<H>(
        &self,
        handler: H,
    ) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::InventoryReservationFailed> + Send + Sync + 'static,
    {
        let wrapped_handler = InventoryReservationFailedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// ShippingFailedハンドラーを登録
    pub async fn subscribe_shipping_failed<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::ShippingFailed> + Send + Sync + 'static,
    {
        let wrapped_handler = ShippingFailedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// DeliveryFailedハンドラーを登録
    pub async fn subscribe_delivery_failed<H>(&self, handler: H) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::DeliveryFailed> + Send + Sync + 'static,
    {
        let wrapped_handler = DeliveryFailedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// SagaCompensationStartedハンドラーを登録
    pub async fn subscribe_saga_compensation_started<H>(
        &self,
        handler: H,
    ) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::SagaCompensationStarted> + Send + Sync + 'static,
    {
        let wrapped_handler = SagaCompensationStartedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }

    /// SagaCompensationCompletedハンドラーを登録
    pub async fn subscribe_saga_compensation_completed<H>(
        &self,
        handler: H,
    ) -> Result<(), EventBusError>
    where
        H: EventHandler<crate::domain::event::SagaCompensationCompleted> + Send + Sync + 'static,
    {
        let wrapped_handler = SagaCompensationCompletedHandlerWrapper::new(handler);
        let mut handlers = self.handlers.write().await;
        handlers.push(Box::new(wrapped_handler));
        Ok(())
    }
}

// Clone実装（Arc使用のため簡単に実装可能）
impl Clone for InMemoryEventBus {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
            dead_letter_queue: self.dead_letter_queue.clone(),
            config: self.config.clone(),
            serializer: EventSerializer::new(), // 新しいシリアライザーインスタンスを作成
        }
    }
}

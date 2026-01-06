use crate::domain::event::DomainEvent;
use async_trait::async_trait;

/// イベントハンドラーエラー
#[derive(Debug, Clone, thiserror::Error)]
pub enum HandlerError {
    #[error("Handler processing failed: {0}")]
    ProcessingFailed(String),
    #[error("Repository error: {0}")]
    RepositoryError(String),
    #[error("Domain error: {0}")]
    DomainError(String),
    #[error("Transient error (retryable): {0}")]
    TransientError(String),
    #[error("Permanent error (not retryable): {0}")]
    PermanentError(String),
}

/// イベントハンドラートレイト
/// 特定のイベントタイプを処理するハンドラーを定義
#[async_trait]
pub trait EventHandler<E>: Send + Sync {
    async fn handle(&self, event: E) -> Result<(), HandlerError>;
}

/// 型消去されたイベントハンドラー
/// 異なるイベントタイプのハンドラーを統一的に扱うため
#[async_trait]
pub trait DynEventHandler: Send + Sync {
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError>;
    fn can_handle(&self, event: &DomainEvent) -> bool;
    fn handler_name(&self) -> &str;
    fn supports_schema_version(&self, version: u32) -> bool;
}

/// OrderConfirmed用のハンドラーラッパー
pub struct OrderConfirmedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderConfirmed>,
{
    handler: H,
    name: String,
}

impl<H> OrderConfirmedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderConfirmed>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "OrderConfirmedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for OrderConfirmedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderConfirmed>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::OrderConfirmed(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::OrderConfirmed(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        // OrderConfirmed supports versions 1 and above
        version >= 1
    }
}

/// OrderCancelled用のハンドラーラッパー
pub struct OrderCancelledHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderCancelled>,
{
    handler: H,
    name: String,
}

impl<H> OrderCancelledHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderCancelled>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "OrderCancelledHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for OrderCancelledHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderCancelled>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::OrderCancelled(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::OrderCancelled(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// OrderShipped用のハンドラーラッパー
pub struct OrderShippedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderShipped>,
{
    handler: H,
    name: String,
}

impl<H> OrderShippedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderShipped>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "OrderShippedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for OrderShippedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderShipped>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::OrderShipped(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::OrderShipped(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// OrderDelivered用のハンドラーラッパー
pub struct OrderDeliveredHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderDelivered>,
{
    handler: H,
    name: String,
}

impl<H> OrderDeliveredHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderDelivered>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "OrderDeliveredHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for OrderDeliveredHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::OrderDelivered>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::OrderDelivered(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::OrderDelivered(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// InventoryReserved用のハンドラーラッパー
pub struct InventoryReservedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReserved>,
{
    handler: H,
    name: String,
}

impl<H> InventoryReservedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReserved>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "InventoryReservedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for InventoryReservedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReserved>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::InventoryReserved(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::InventoryReserved(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// InventoryReleased用のハンドラーラッパー
pub struct InventoryReleasedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReleased>,
{
    handler: H,
    name: String,
}

impl<H> InventoryReleasedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReleased>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "InventoryReleasedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for InventoryReleasedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReleased>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::InventoryReleased(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::InventoryReleased(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

// ========== 補償イベント用のハンドラーラッパー ==========

/// InventoryReservationFailed用のハンドラーラッパー
pub struct InventoryReservationFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReservationFailed>,
{
    handler: H,
    name: String,
}

impl<H> InventoryReservationFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReservationFailed>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "InventoryReservationFailedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for InventoryReservationFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::InventoryReservationFailed>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::InventoryReservationFailed(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::InventoryReservationFailed(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// ShippingFailed用のハンドラーラッパー
pub struct ShippingFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::ShippingFailed>,
{
    handler: H,
    name: String,
}

impl<H> ShippingFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::ShippingFailed>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "ShippingFailedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for ShippingFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::ShippingFailed>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::ShippingFailed(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::ShippingFailed(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// DeliveryFailed用のハンドラーラッパー
pub struct DeliveryFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::DeliveryFailed>,
{
    handler: H,
    name: String,
}

impl<H> DeliveryFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::DeliveryFailed>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "DeliveryFailedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for DeliveryFailedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::DeliveryFailed>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::DeliveryFailed(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::DeliveryFailed(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// SagaCompensationStarted用のハンドラーラッパー
pub struct SagaCompensationStartedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationStarted>,
{
    handler: H,
    name: String,
}

impl<H> SagaCompensationStartedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationStarted>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "SagaCompensationStartedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for SagaCompensationStartedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationStarted>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::SagaCompensationStarted(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::SagaCompensationStarted(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

/// SagaCompensationCompleted用のハンドラーラッパー
pub struct SagaCompensationCompletedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationCompleted>,
{
    handler: H,
    name: String,
}

impl<H> SagaCompensationCompletedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationCompleted>,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            name: "SagaCompensationCompletedHandler".to_string(),
        }
    }

    pub fn with_name(handler: H, name: String) -> Self {
        Self { handler, name }
    }
}

#[async_trait]
impl<H> DynEventHandler for SagaCompensationCompletedHandlerWrapper<H>
where
    H: EventHandler<crate::domain::event::SagaCompensationCompleted>,
{
    async fn handle_event(&self, event: &DomainEvent) -> Result<(), HandlerError> {
        match event {
            DomainEvent::SagaCompensationCompleted(e) => self.handler.handle(e.clone()).await,
            _ => Err(HandlerError::ProcessingFailed(
                "Event type mismatch".to_string(),
            )),
        }
    }

    fn can_handle(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::SagaCompensationCompleted(_))
    }

    fn handler_name(&self) -> &str {
        &self.name
    }

    fn supports_schema_version(&self, version: u32) -> bool {
        version >= 1
    }
}

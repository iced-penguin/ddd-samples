use crate::domain::model::{CustomerId, Money, OrderId, OrderLine, ShippingAddress};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// イベントメタデータ
/// 全てのドメインイベントに共通するメタデータ情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// イベントの一意識別子
    pub event_id: Uuid,
    /// イベント発生日時
    pub occurred_at: DateTime<Utc>,
    /// 相関ID（サーガやトランザクションの追跡用）
    pub correlation_id: Uuid,
    /// イベントバージョン（スキーマ進化対応）
    pub event_version: u32,
    /// 追加のメタデータ（拡張可能）
    pub additional_metadata: HashMap<String, String>,
}

impl EventMetadata {
    /// 新しいイベントメタデータを作成
    pub fn new() -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            correlation_id: Uuid::new_v4(),
            event_version: 1,
            additional_metadata: HashMap::new(),
        }
    }

    /// 相関IDを指定してイベントメタデータを作成
    pub fn with_correlation_id(correlation_id: Uuid) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            correlation_id,
            event_version: 1,
            additional_metadata: HashMap::new(),
        }
    }

    /// 追加メタデータを設定
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.additional_metadata.insert(key, value);
        self
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// ドメインイベント列挙型
/// ビジネス上の重要なイベントを表現する
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "event_data")]
pub enum DomainEvent {
    /// 注文が確定された
    OrderConfirmed(OrderConfirmed),
    /// 注文がキャンセルされた
    OrderCancelled(OrderCancelled),
    /// 注文が発送された
    OrderShipped(OrderShipped),
    /// 注文が配達完了した
    OrderDelivered(OrderDelivered),
    /// 在庫が予約された
    InventoryReserved(InventoryReserved),
    /// 在庫が解放された
    InventoryReleased(InventoryReleased),

    // 補償イベント（サーガ失敗時のロールバック用）
    /// 在庫予約失敗（補償イベント）
    InventoryReservationFailed(InventoryReservationFailed),
    /// 発送失敗（補償イベント）
    ShippingFailed(ShippingFailed),
    /// 配達失敗（補償イベント）
    DeliveryFailed(DeliveryFailed),
    /// サーガ補償開始（補償プロセス開始の通知）
    SagaCompensationStarted(SagaCompensationStarted),
    /// サーガ補償完了（補償プロセス完了の通知）
    SagaCompensationCompleted(SagaCompensationCompleted),
}

impl DomainEvent {
    /// イベントのメタデータを取得
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            DomainEvent::OrderConfirmed(event) => &event.metadata,
            DomainEvent::OrderCancelled(event) => &event.metadata,
            DomainEvent::OrderShipped(event) => &event.metadata,
            DomainEvent::OrderDelivered(event) => &event.metadata,
            DomainEvent::InventoryReserved(event) => &event.metadata,
            DomainEvent::InventoryReleased(event) => &event.metadata,
            DomainEvent::InventoryReservationFailed(event) => &event.metadata,
            DomainEvent::ShippingFailed(event) => &event.metadata,
            DomainEvent::DeliveryFailed(event) => &event.metadata,
            DomainEvent::SagaCompensationStarted(event) => &event.metadata,
            DomainEvent::SagaCompensationCompleted(event) => &event.metadata,
        }
    }

    /// イベントタイプを文字列として取得
    pub fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::OrderConfirmed(_) => "OrderConfirmed",
            DomainEvent::OrderCancelled(_) => "OrderCancelled",
            DomainEvent::OrderShipped(_) => "OrderShipped",
            DomainEvent::OrderDelivered(_) => "OrderDelivered",
            DomainEvent::InventoryReserved(_) => "InventoryReserved",
            DomainEvent::InventoryReleased(_) => "InventoryReleased",
            DomainEvent::InventoryReservationFailed(_) => "InventoryReservationFailed",
            DomainEvent::ShippingFailed(_) => "ShippingFailed",
            DomainEvent::DeliveryFailed(_) => "DeliveryFailed",
            DomainEvent::SagaCompensationStarted(_) => "SagaCompensationStarted",
            DomainEvent::SagaCompensationCompleted(_) => "SagaCompensationCompleted",
        }
    }
}

/// 注文確定イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfirmed {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 顧客ID
    pub customer_id: CustomerId,
    /// 注文明細のリスト
    pub order_lines: Vec<OrderLine>,
    /// 合計金額
    pub total_amount: Money,
}

impl OrderConfirmed {
    /// 新しい注文確定イベントを作成
    pub fn new(
        order_id: OrderId,
        customer_id: CustomerId,
        order_lines: Vec<OrderLine>,
        total_amount: Money,
    ) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
            customer_id,
            order_lines,
            total_amount,
        }
    }
}

/// 注文キャンセルイベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancelled {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 顧客ID
    pub customer_id: CustomerId,
    /// 注文明細のリスト（在庫解放のため）
    pub order_lines: Vec<OrderLine>,
}

impl OrderCancelled {
    /// 新しい注文キャンセルイベントを作成
    pub fn new(order_id: OrderId, customer_id: CustomerId, order_lines: Vec<OrderLine>) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
            customer_id,
            order_lines,
        }
    }

    /// 相関IDを指定して注文キャンセルイベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        customer_id: CustomerId,
        order_lines: Vec<OrderLine>,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
            customer_id,
            order_lines,
        }
    }
}

/// 注文発送イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderShipped {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 配送先住所
    pub shipping_address: ShippingAddress,
}

impl OrderShipped {
    /// 新しい注文発送イベントを作成
    pub fn new(order_id: OrderId, shipping_address: ShippingAddress) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
            shipping_address,
        }
    }

    /// 相関IDを指定して注文発送イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        shipping_address: ShippingAddress,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
            shipping_address,
        }
    }
}

/// 注文配達完了イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDelivered {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
}

impl OrderDelivered {
    /// 新しい注文配達完了イベントを作成
    pub fn new(order_id: OrderId) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
        }
    }

    /// 相関IDを指定して注文配達完了イベントを作成
    pub fn with_correlation_id(order_id: OrderId, correlation_id: Uuid) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string()),
            order_id,
        }
    }
}
/// 在庫予約イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReserved {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 注文明細のリスト
    pub order_lines: Vec<OrderLine>,
}

impl InventoryReserved {
    /// 相関IDを指定して在庫予約イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        order_lines: Vec<OrderLine>,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Inventory".to_string())
                .with_metadata("related_order_id".to_string(), order_id.to_string()),
            order_id,
            order_lines,
        }
    }
}

/// 在庫解放イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReleased {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 注文明細のリスト
    pub order_lines: Vec<OrderLine>,
}

impl InventoryReleased {
    /// 相関IDを指定して在庫解放イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        order_lines: Vec<OrderLine>,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Inventory".to_string())
                .with_metadata("related_order_id".to_string(), order_id.to_string()),
            order_id,
            order_lines,
        }
    }
}

// ========== 補償イベント（サーガ失敗時のロールバック用） ==========

/// 在庫予約失敗イベント（補償イベント）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReservationFailed {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 注文明細のリスト
    pub order_lines: Vec<OrderLine>,
    /// 失敗理由
    pub failure_reason: String,
    /// 元のイベントID（失敗したイベントの追跡用）
    pub original_event_id: Uuid,
}

impl InventoryReservationFailed {
    /// 新しい在庫予約失敗イベントを作成
    pub fn new(
        order_id: OrderId,
        order_lines: Vec<OrderLine>,
        failure_reason: String,
        original_event_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Inventory".to_string())
                .with_metadata("related_order_id".to_string(), order_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            order_id,
            order_lines,
            failure_reason,
            original_event_id,
        }
    }

    /// 相関IDを指定して在庫予約失敗イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        order_lines: Vec<OrderLine>,
        failure_reason: String,
        original_event_id: Uuid,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Inventory".to_string())
                .with_metadata("related_order_id".to_string(), order_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            order_id,
            order_lines,
            failure_reason,
            original_event_id,
        }
    }
}

/// 発送失敗イベント（補償イベント）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingFailed {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 失敗理由
    pub failure_reason: String,
    /// 元のイベントID（失敗したイベントの追跡用）
    pub original_event_id: Uuid,
}

impl ShippingFailed {
    /// 新しい発送失敗イベントを作成
    pub fn new(order_id: OrderId, failure_reason: String, original_event_id: Uuid) -> Self {
        Self {
            metadata: EventMetadata::new()
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            order_id,
            failure_reason,
            original_event_id,
        }
    }

    /// 相関IDを指定して発送失敗イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        failure_reason: String,
        original_event_id: Uuid,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            order_id,
            failure_reason,
            original_event_id,
        }
    }
}

/// 配達失敗イベント（補償イベント）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryFailed {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// 注文ID
    pub order_id: OrderId,
    /// 失敗理由
    pub failure_reason: String,
    /// 元のイベントID（失敗したイベントの追跡用）
    pub original_event_id: Uuid,
}

impl DeliveryFailed {
    /// 相関IDを指定して配達失敗イベントを作成
    pub fn with_correlation_id(
        order_id: OrderId,
        failure_reason: String,
        original_event_id: Uuid,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(correlation_id)
                .with_metadata("aggregate_type".to_string(), "Order".to_string())
                .with_metadata("aggregate_id".to_string(), order_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            order_id,
            failure_reason,
            original_event_id,
        }
    }
}

/// サーガ補償開始イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaCompensationStarted {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// サーガID（相関IDと同じ）
    pub saga_id: Uuid,
    /// 失敗したステップ
    pub failed_step: String,
    /// 失敗理由
    pub failure_reason: String,
    /// 補償が必要なステップのリスト（逆順）
    pub compensation_steps: Vec<String>,
}

impl SagaCompensationStarted {
    /// 新しいサーガ補償開始イベントを作成
    pub fn new(
        saga_id: Uuid,
        failed_step: String,
        failure_reason: String,
        compensation_steps: Vec<String>,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(saga_id)
                .with_metadata("aggregate_type".to_string(), "Saga".to_string())
                .with_metadata("saga_id".to_string(), saga_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            saga_id,
            failed_step,
            failure_reason,
            compensation_steps,
        }
    }
}

/// サーガ補償完了イベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaCompensationCompleted {
    /// イベントメタデータ
    pub metadata: EventMetadata,
    /// サーガID（相関IDと同じ）
    pub saga_id: Uuid,
    /// 補償されたステップのリスト
    pub compensated_steps: Vec<String>,
    /// 補償結果（成功/部分的成功/失敗）
    pub compensation_result: CompensationResult,
}

impl SagaCompensationCompleted {
    /// 新しいサーガ補償完了イベントを作成
    pub fn new(
        saga_id: Uuid,
        compensated_steps: Vec<String>,
        compensation_result: CompensationResult,
    ) -> Self {
        Self {
            metadata: EventMetadata::with_correlation_id(saga_id)
                .with_metadata("aggregate_type".to_string(), "Saga".to_string())
                .with_metadata("saga_id".to_string(), saga_id.to_string())
                .with_metadata("compensation_event".to_string(), "true".to_string()),
            saga_id,
            compensated_steps,
            compensation_result,
        }
    }
}

/// 補償結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompensationResult {
    /// 全ての補償が成功
    Success,
    /// 一部の補償が失敗（部分的成功）
    PartialSuccess { failed_steps: Vec<String> },
    /// 全ての補償が失敗
    Failed { error_message: String },
}

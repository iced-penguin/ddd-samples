use chrono::{DateTime, Utc};
use crate::domain::model::{OrderId, CustomerId, OrderLine, ShippingAddress, Money};

/// ドメインイベント列挙型
/// ビジネス上の重要なイベントを表現する
#[derive(Debug, Clone)]
pub enum DomainEvent {
    /// 注文が確定された
    OrderConfirmed(OrderConfirmed),
    /// 注文がキャンセルされた
    OrderCancelled(OrderCancelled),
    /// 注文が発送された
    OrderShipped(OrderShipped),
    /// 注文が配達完了した
    OrderDelivered(OrderDelivered),
}

/// 注文確定イベント
#[derive(Debug, Clone)]
pub struct OrderConfirmed {
    /// 注文ID
    pub order_id: OrderId,
    /// 顧客ID
    pub customer_id: CustomerId,
    /// 注文明細のリスト
    pub order_lines: Vec<OrderLine>,
    /// 合計金額
    pub total_amount: Money,
    /// イベント発生日時
    pub occurred_at: DateTime<Utc>,
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
            order_id,
            customer_id,
            order_lines,
            total_amount,
            occurred_at: Utc::now(),
        }
    }
}

/// 注文キャンセルイベント
#[derive(Debug, Clone)]
pub struct OrderCancelled {
    /// 注文ID
    pub order_id: OrderId,
    /// 顧客ID
    pub customer_id: CustomerId,
    /// 注文明細のリスト（在庫解放のため）
    pub order_lines: Vec<OrderLine>,
    /// イベント発生日時
    pub occurred_at: DateTime<Utc>,
}

impl OrderCancelled {
    /// 新しい注文キャンセルイベントを作成
    pub fn new(
        order_id: OrderId,
        customer_id: CustomerId,
        order_lines: Vec<OrderLine>,
    ) -> Self {
        Self {
            order_id,
            customer_id,
            order_lines,
            occurred_at: Utc::now(),
        }
    }
}

/// 注文発送イベント
#[derive(Debug, Clone)]
pub struct OrderShipped {
    /// 注文ID
    pub order_id: OrderId,
    /// 配送先住所
    pub shipping_address: ShippingAddress,
    /// イベント発生日時
    pub occurred_at: DateTime<Utc>,
}

impl OrderShipped {
    /// 新しい注文発送イベントを作成
    pub fn new(order_id: OrderId, shipping_address: ShippingAddress) -> Self {
        Self {
            order_id,
            shipping_address,
            occurred_at: Utc::now(),
        }
    }
}

/// 注文配達完了イベント
#[derive(Debug, Clone)]
pub struct OrderDelivered {
    /// 注文ID
    pub order_id: OrderId,
    /// イベント発生日時
    pub occurred_at: DateTime<Utc>,
}

impl OrderDelivered {
    /// 新しい注文配達完了イベントを作成
    pub fn new(order_id: OrderId) -> Self {
        Self {
            order_id,
            occurred_at: Utc::now(),
        }
    }
}

use crate::domain::event::DomainEvent;
use crate::domain::port::{EventPublisher, PublisherError};

/// コンソールイベント発行者
/// ドメインイベントをコンソールに出力する
pub struct ConsoleEventPublisher;

impl ConsoleEventPublisher {
    /// 新しいコンソールイベント発行者を作成
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventPublisher for ConsoleEventPublisher {
    fn publish(&self, event: &DomainEvent) -> Result<(), PublisherError> {
        match event {
            DomainEvent::OrderConfirmed(e) => {
                println!("📦 [イベント] 注文確定");
                println!("  注文ID: {:?}", e.order_id);
                println!("  顧客ID: {:?}", e.customer_id);
                println!("  注文明細数: {}", e.order_lines.len());
                println!("  合計金額: {}円", e.total_amount.amount());
                println!("  発生日時: {}", e.occurred_at.format("%Y-%m-%d %H:%M:%S"));
            }
            DomainEvent::OrderCancelled(e) => {
                println!("❌ [イベント] 注文キャンセル");
                println!("  注文ID: {:?}", e.order_id);
                println!("  顧客ID: {:?}", e.customer_id);
                println!("  注文明細数: {}", e.order_lines.len());
                println!("  発生日時: {}", e.occurred_at.format("%Y-%m-%d %H:%M:%S"));
            }
            DomainEvent::OrderShipped(e) => {
                println!("🚚 [イベント] 注文発送");
                println!("  注文ID: {:?}", e.order_id);
                println!("  配送先: 〒{} {} {} {}", 
                    e.shipping_address.postal_code(),
                    e.shipping_address.prefecture(),
                    e.shipping_address.city(),
                    e.shipping_address.street()
                );
                if let Some(building) = e.shipping_address.building() {
                    println!("  建物名: {}", building);
                }
                println!("  発生日時: {}", e.occurred_at.format("%Y-%m-%d %H:%M:%S"));
            }
            DomainEvent::OrderDelivered(e) => {
                println!("✅ [イベント] 注文配達完了");
                println!("  注文ID: {:?}", e.order_id);
                println!("  発生日時: {}", e.occurred_at.format("%Y-%m-%d %H:%M:%S"));
            }
        }
        println!(); // 空行を追加
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{OrderId, CustomerId, BookId, OrderLine, ShippingAddress, Money};
    use crate::domain::event::{OrderConfirmed, OrderCancelled, OrderShipped, OrderDelivered};

    #[test]
    fn test_publish_order_confirmed_event() {
        let publisher = ConsoleEventPublisher::new();
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let book_id = BookId::new();
        let order_line = OrderLine::new(book_id, 2, Money::jpy(1000)).unwrap();
        let event = OrderConfirmed::new(
            order_id,
            customer_id,
            vec![order_line],
            Money::jpy(2500),
        );

        let result = publisher.publish(&DomainEvent::OrderConfirmed(event));
        assert!(result.is_ok());
    }

    #[test]
    fn test_publish_order_cancelled_event() {
        let publisher = ConsoleEventPublisher::new();
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let book_id = BookId::new();
        let order_line = OrderLine::new(book_id, 2, Money::jpy(1000)).unwrap();
        let event = OrderCancelled::new(
            order_id,
            customer_id,
            vec![order_line],
        );

        let result = publisher.publish(&DomainEvent::OrderCancelled(event));
        assert!(result.is_ok());
    }

    #[test]
    fn test_publish_order_shipped_event() {
        let publisher = ConsoleEventPublisher::new();
        let order_id = OrderId::new();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            Some("テストビル".to_string()),
        ).unwrap();
        let event = OrderShipped::new(order_id, address);

        let result = publisher.publish(&DomainEvent::OrderShipped(event));
        assert!(result.is_ok());
    }

    #[test]
    fn test_publish_order_delivered_event() {
        let publisher = ConsoleEventPublisher::new();
        let order_id = OrderId::new();
        let event = OrderDelivered::new(order_id);

        let result = publisher.publish(&DomainEvent::OrderDelivered(event));
        assert!(result.is_ok());
    }
}
use crate::domain::error::DomainError;
use crate::domain::event::{DomainEvent, OrderConfirmed, OrderCancelled, OrderShipped, OrderDelivered};
use crate::domain::model::{OrderId, CustomerId, BookId, OrderLine, ShippingAddress, OrderStatus, Money};

/// Order集約
/// 注文のライフサイクルを管理し、ビジネスルールを適用する
#[derive(Debug, Clone)]
pub struct Order {
    id: OrderId,
    customer_id: CustomerId,
    order_lines: Vec<OrderLine>,
    shipping_address: Option<ShippingAddress>,
    status: OrderStatus,
    domain_events: Vec<DomainEvent>,
}

impl Order {
    /// 新しい注文を作成
    /// 初期ステータスはPending
    pub fn new(id: OrderId, customer_id: CustomerId) -> Self {
        Self {
            id,
            customer_id,
            order_lines: Vec::new(),
            shipping_address: None,
            status: OrderStatus::Pending,
            domain_events: Vec::new(),
        }
    }

    /// データベースから取得したデータで注文を再構築
    /// リポジトリでの使用を想定
    pub fn reconstruct(
        id: OrderId,
        customer_id: CustomerId,
        order_lines: Vec<OrderLine>,
        shipping_address: Option<ShippingAddress>,
        status: OrderStatus,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            customer_id,
            order_lines,
            shipping_address,
            status,
            domain_events: Vec::new(),
        })
    }

    /// 注文IDを取得
    pub fn id(&self) -> OrderId {
        self.id
    }

    /// 顧客IDを取得
    pub fn customer_id(&self) -> CustomerId {
        self.customer_id
    }

    /// 注文明細のリストを取得
    pub fn order_lines(&self) -> &[OrderLine] {
        &self.order_lines
    }

    /// 配送先住所を取得
    pub fn shipping_address(&self) -> Option<&ShippingAddress> {
        self.shipping_address.as_ref()
    }

    /// 注文ステータスを取得
    pub fn status(&self) -> OrderStatus {
        self.status
    }

    /// ドメインイベントを取得してクリア
    pub fn take_domain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.domain_events)
    }

    /// 書籍を注文に追加
    /// 同じ書籍が既に存在する場合は数量を増加
    pub fn add_book(&mut self, book_id: BookId, quantity: u32, unit_price: Money) -> Result<(), DomainError> {
        // 数量のバリデーション（1以上）
        if quantity == 0 {
            return Err(DomainError::InvalidQuantity);
        }

        // 同じ書籍が既に存在するか確認
        if let Some(existing_line) = self.order_lines.iter_mut().find(|line| line.book_id() == book_id) {
            // 既存の注文明細の数量を増加
            existing_line.increase_quantity(quantity)?;
        } else {
            // 新しい注文明細を作成して追加
            let order_line = OrderLine::new(book_id, quantity, unit_price)?;
            self.order_lines.push(order_line);
        }

        Ok(())
    }

    /// 配送先住所を設定
    pub fn set_shipping_address(&mut self, address: ShippingAddress) {
        self.shipping_address = Some(address);
    }

    /// 合計金額を計算
    /// 小計 + 配送料（10,000円以上なら0円、未満なら500円）
    pub fn calculate_total(&self) -> Money {
        // 全注文明細の小計を合算
        let subtotal = self.order_lines.iter()
            .map(|line| line.subtotal())
            .fold(Money::jpy(0), |acc, amount| {
                acc.add(&amount).unwrap_or(acc)
            });

        // 配送料の計算（10,000円以上なら0円、未満なら500円）
        let shipping_fee = if subtotal.amount() >= 10_000 {
            Money::jpy(0)
        } else {
            Money::jpy(500)
        };

        // 最終金額 = 小計 + 配送料
        subtotal.add(&shipping_fee).unwrap_or(subtotal)
    }

    /// 注文を確定
    /// 事前条件:
    /// - ステータスがPending
    /// - 注文明細が1つ以上
    /// - 配送先住所が設定済み
    pub fn confirm(&mut self) -> Result<(), DomainError> {
        // ステータスがPendingであることを確認
        if self.status != OrderStatus::Pending {
            return Err(DomainError::InvalidOrderState(
                "注文を確定できるのはPending状態のみです".to_string()
            ));
        }

        // 注文明細が1つ以上あることを確認
        if self.order_lines.is_empty() {
            return Err(DomainError::OrderValidation(
                "注文明細が空です。少なくとも1つの書籍を追加してください".to_string()
            ));
        }

        // 配送先住所が設定されていることを確認
        if self.shipping_address.is_none() {
            return Err(DomainError::OrderValidation(
                "配送先住所が設定されていません".to_string()
            ));
        }

        // ステータスをConfirmedに変更
        self.status = OrderStatus::Confirmed;

        // OrderConfirmedイベントを生成
        let total_amount = self.calculate_total();
        let event = OrderConfirmed::new(
            self.id,
            self.customer_id,
            self.order_lines.clone(),
            total_amount,
        );
        self.domain_events.push(DomainEvent::OrderConfirmed(event));

        Ok(())
    }

    /// 注文をキャンセル
    /// 事前条件:
    /// - ステータスがPendingまたはConfirmed
    pub fn cancel(&mut self) -> Result<(), DomainError> {
        // ステータスがPendingまたはConfirmedであることを確認
        match self.status {
            OrderStatus::Pending | OrderStatus::Confirmed => {
                // キャンセル可能
            }
            OrderStatus::Shipped | OrderStatus::Delivered => {
                return Err(DomainError::InvalidOrderState(
                    "発送済みまたは配達完了の注文はキャンセルできません".to_string()
                ));
            }
            OrderStatus::Cancelled => {
                return Err(DomainError::InvalidOrderState(
                    "既にキャンセル済みの注文です".to_string()
                ));
            }
        }

        // ステータスをCancelledに変更
        self.status = OrderStatus::Cancelled;

        // OrderCancelledイベントを生成
        let event = OrderCancelled::new(
            self.id,
            self.customer_id,
            self.order_lines.clone(),
        );
        self.domain_events.push(DomainEvent::OrderCancelled(event));

        Ok(())
    }

    /// 注文を発送済みにマーク
    /// 事前条件:
    /// - ステータスがConfirmed
    pub fn mark_as_shipped(&mut self) -> Result<(), DomainError> {
        // ステータスがConfirmedであることを確認
        if self.status != OrderStatus::Confirmed {
            return Err(DomainError::InvalidOrderState(
                "発送済みにマークできるのはConfirmed状態のみです".to_string()
            ));
        }

        // ステータスをShippedに変更
        self.status = OrderStatus::Shipped;

        // OrderShippedイベントを生成
        let shipping_address = self.shipping_address.clone()
            .expect("Confirmed状態の注文には配送先住所が必須です");
        let event = OrderShipped::new(self.id, shipping_address);
        self.domain_events.push(DomainEvent::OrderShipped(event));

        Ok(())
    }

    /// 注文を配達完了にマーク
    /// 事前条件:
    /// - ステータスがShipped
    pub fn mark_as_delivered(&mut self) -> Result<(), DomainError> {
        // ステータスがShippedであることを確認
        if self.status != OrderStatus::Shipped {
            return Err(DomainError::InvalidOrderState(
                "配達完了にマークできるのはShipped状態のみです".to_string()
            ));
        }

        // ステータスをDeliveredに変更
        self.status = OrderStatus::Delivered;

        // OrderDeliveredイベントを生成
        let event = OrderDelivered::new(self.id);
        self.domain_events.push(DomainEvent::OrderDelivered(event));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_order_has_pending_status() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let order = Order::new(order_id, customer_id);

        assert_eq!(order.status(), OrderStatus::Pending);
        assert_eq!(order.order_lines().len(), 0);
        assert!(order.shipping_address().is_none());
    }

    #[test]
    fn test_add_book_creates_order_line() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let book_id = BookId::new();
        let price = Money::jpy(1000);
        let result = order.add_book(book_id, 2, price);

        assert!(result.is_ok());
        assert_eq!(order.order_lines().len(), 1);
        assert_eq!(order.order_lines()[0].quantity(), 2);
    }

    #[test]
    fn test_add_same_book_increases_quantity() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let book_id = BookId::new();
        let price = Money::jpy(1000);
        
        order.add_book(book_id, 2, price).unwrap();
        order.add_book(book_id, 3, price).unwrap();

        assert_eq!(order.order_lines().len(), 1);
        assert_eq!(order.order_lines()[0].quantity(), 5);
    }

    #[test]
    fn test_add_book_with_zero_quantity_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let book_id = BookId::new();
        let price = Money::jpy(1000);
        let result = order.add_book(book_id, 0, price);

        assert!(result.is_err());
    }

    #[test]
    fn test_set_shipping_address() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();

        order.set_shipping_address(address);
        assert!(order.shipping_address().is_some());
    }

    #[test]
    fn test_calculate_total_with_shipping_fee() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        let total = order.calculate_total();
        // 2000円 + 配送料500円 = 2500円
        assert_eq!(total.amount(), 2500);
    }

    #[test]
    fn test_calculate_total_without_shipping_fee() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let book_id = BookId::new();
        let price = Money::jpy(5000);
        order.add_book(book_id, 3, price).unwrap();

        let total = order.calculate_total();
        // 15000円（配送料なし）
        assert_eq!(total.amount(), 15000);
    }

    #[test]
    fn test_confirm_order_success() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        // 配送先住所を設定
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);

        // 注文を確定
        let result = order.confirm();
        assert!(result.is_ok());
        assert_eq!(order.status(), OrderStatus::Confirmed);
        
        // イベントが生成されたことを確認
        let events = order.take_domain_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_confirm_order_without_order_lines_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 配送先住所を設定
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);

        // 注文明細なしで確定を試みる
        let result = order.confirm();
        assert!(result.is_err());
    }

    #[test]
    fn test_confirm_order_without_shipping_address_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        // 配送先住所なしで確定を試みる
        let result = order.confirm();
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_pending_order() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let result = order.cancel();
        assert!(result.is_ok());
        assert_eq!(order.status(), OrderStatus::Cancelled);
    }

    #[test]
    fn test_cancel_confirmed_order() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 注文を確定状態にする
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);
        order.confirm().unwrap();

        // キャンセル
        let result = order.cancel();
        assert!(result.is_ok());
        assert_eq!(order.status(), OrderStatus::Cancelled);
    }

    #[test]
    fn test_cancel_shipped_order_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 注文を発送済み状態にする
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);
        order.confirm().unwrap();
        order.mark_as_shipped().unwrap();

        // キャンセルを試みる
        let result = order.cancel();
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_as_shipped_success() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 注文を確定状態にする
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);
        order.confirm().unwrap();

        // 発送済みにマーク
        let result = order.mark_as_shipped();
        assert!(result.is_ok());
        assert_eq!(order.status(), OrderStatus::Shipped);
    }

    #[test]
    fn test_mark_as_shipped_from_pending_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        let result = order.mark_as_shipped();
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_as_delivered_success() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 注文を発送済み状態にする
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);
        order.confirm().unwrap();
        order.mark_as_shipped().unwrap();

        // 配達完了にマーク
        let result = order.mark_as_delivered();
        assert!(result.is_ok());
        assert_eq!(order.status(), OrderStatus::Delivered);
    }

    #[test]
    fn test_mark_as_delivered_from_confirmed_fails() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 注文を確定状態にする
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order.set_shipping_address(address);
        order.confirm().unwrap();

        // 配達完了にマークを試みる（Shipped状態でないので失敗）
        let result = order.mark_as_delivered();
        assert!(result.is_err());
    }
}

use crate::domain::model::{Inventory, Money, Order, OrderLine, ShippingAddress};
use serde::Serialize;

/// 注文一覧用のレスポンスDTO
#[derive(Serialize)]
pub struct OrderSummaryResponse {
    pub order_id: String,
    pub customer_id: String,
    pub status: String,
    pub total_amount: i64,
    pub total_currency: String,
    pub created_at: String,
}

/// 注文詳細用のレスポンスDTO
#[derive(Serialize)]
pub struct OrderDetailResponse {
    pub order_id: String,
    pub customer_id: String,
    pub status: String,
    pub order_lines: Vec<OrderLineResponse>,
    pub shipping_address: Option<ShippingAddressResponse>,
    pub subtotal_amount: i64,
    pub subtotal_currency: String,
    pub shipping_fee_amount: i64,
    pub shipping_fee_currency: String,
    pub total_amount: i64,
    pub total_currency: String,
}

/// 注文明細用のレスポンスDTO
#[derive(Serialize)]
pub struct OrderLineResponse {
    pub book_id: String,
    pub quantity: u32,
    pub unit_price_amount: i64,
    pub unit_price_currency: String,
    pub subtotal_amount: i64,
    pub subtotal_currency: String,
}

/// 配送先住所用のレスポンスDTO
#[derive(Serialize)]
pub struct ShippingAddressResponse {
    pub postal_code: String,
    pub prefecture: String,
    pub city: String,
    pub street: String,
    pub building: Option<String>,
}

/// 在庫用のレスポンスDTO
#[derive(Serialize)]
pub struct InventoryResponse {
    pub book_id: String,
    pub quantity_on_hand: u32,
}

impl OrderSummaryResponse {
    /// ドメインオブジェクトからOrderSummaryResponseを作成
    /// 注意: created_atは現在のシステムでは利用できないため、固定値を使用
    pub fn from_order(order: &Order) -> Self {
        let total = order.calculate_total();
        Self {
            order_id: order.id().to_string(),
            customer_id: order.customer_id().to_string(),
            status: order.status().to_string(),
            total_amount: total.amount(),
            total_currency: total.currency(),
            created_at: "2024-01-01T00:00:00Z".to_string(), // TODO: 実際の作成日時を使用
        }
    }
}

impl OrderDetailResponse {
    /// ドメインオブジェクトからOrderDetailResponseを作成
    pub fn from_order(order: &Order) -> Self {
        let order_lines: Vec<OrderLineResponse> = order
            .order_lines()
            .iter()
            .map(OrderLineResponse::from_order_line)
            .collect();

        let shipping_address = order
            .shipping_address()
            .map(ShippingAddressResponse::from_shipping_address);

        // 小計を計算（配送料を除く）
        let subtotal = order
            .order_lines()
            .iter()
            .map(|line| line.subtotal())
            .fold(Money::jpy(0), |acc, amount| acc.add(&amount).unwrap_or(acc));

        // 配送料を計算
        let shipping_fee = if subtotal.amount() >= 10_000 {
            Money::jpy(0)
        } else {
            Money::jpy(500)
        };

        let total = order.calculate_total();

        Self {
            order_id: order.id().to_string(),
            customer_id: order.customer_id().to_string(),
            status: order.status().to_string(),
            order_lines,
            shipping_address,
            subtotal_amount: subtotal.amount(),
            subtotal_currency: subtotal.currency(),
            shipping_fee_amount: shipping_fee.amount(),
            shipping_fee_currency: shipping_fee.currency(),
            total_amount: total.amount(),
            total_currency: total.currency(),
        }
    }
}

impl OrderLineResponse {
    /// ドメインオブジェクトからOrderLineResponseを作成
    pub fn from_order_line(order_line: &OrderLine) -> Self {
        let unit_price = order_line.unit_price();
        let subtotal = order_line.subtotal();

        Self {
            book_id: order_line.book_id().to_string(),
            quantity: order_line.quantity(),
            unit_price_amount: unit_price.amount(),
            unit_price_currency: unit_price.currency(),
            subtotal_amount: subtotal.amount(),
            subtotal_currency: subtotal.currency(),
        }
    }
}

impl ShippingAddressResponse {
    /// ドメインオブジェクトからShippingAddressResponseを作成
    pub fn from_shipping_address(address: &ShippingAddress) -> Self {
        Self {
            postal_code: address.postal_code().to_string(),
            prefecture: address.prefecture().to_string(),
            city: address.city().to_string(),
            street: address.street().to_string(),
            building: address.building().map(|s| s.to_string()),
        }
    }
}

impl InventoryResponse {
    /// ドメインオブジェクトからInventoryResponseを作成
    pub fn from_inventory(inventory: &Inventory) -> Self {
        Self {
            book_id: inventory.book_id().to_string(),
            quantity_on_hand: inventory.quantity_on_hand(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{
        BookId, CustomerId, Inventory, Money, OrderId, OrderLine, ShippingAddress,
    };

    #[test]
    fn test_order_summary_response_from_order() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 書籍を追加
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order.add_book(book_id, 2, price).unwrap();

        let response = OrderSummaryResponse::from_order(&order);

        assert_eq!(response.order_id, order_id.to_string());
        assert_eq!(response.customer_id, customer_id.to_string());
        assert_eq!(response.status, "Pending");
        assert_eq!(response.total_amount, 2500); // 2000 + 500 (shipping)
        assert_eq!(response.total_currency, "JPY");
    }

    #[test]
    fn test_order_detail_response_from_order() {
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
            Some("ビル名".to_string()),
        )
        .unwrap();
        order.set_shipping_address(address);

        let response = OrderDetailResponse::from_order(&order);

        assert_eq!(response.order_id, order_id.to_string());
        assert_eq!(response.customer_id, customer_id.to_string());
        assert_eq!(response.status, "Pending");
        assert_eq!(response.order_lines.len(), 1);
        assert_eq!(response.subtotal_amount, 2000);
        assert_eq!(response.shipping_fee_amount, 500);
        assert_eq!(response.total_amount, 2500);
        assert!(response.shipping_address.is_some());
    }

    #[test]
    fn test_order_line_response_from_order_line() {
        let book_id = BookId::new();
        let price = Money::jpy(1500);
        let order_line = OrderLine::new(book_id, 3, price).unwrap();

        let response = OrderLineResponse::from_order_line(&order_line);

        assert_eq!(response.book_id, book_id.to_string());
        assert_eq!(response.quantity, 3);
        assert_eq!(response.unit_price_amount, 1500);
        assert_eq!(response.unit_price_currency, "JPY");
        assert_eq!(response.subtotal_amount, 4500);
        assert_eq!(response.subtotal_currency, "JPY");
    }

    #[test]
    fn test_shipping_address_response_from_shipping_address() {
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            Some("ビル名".to_string()),
        )
        .unwrap();

        let response = ShippingAddressResponse::from_shipping_address(&address);

        assert_eq!(response.postal_code, "1234567");
        assert_eq!(response.prefecture, "東京都");
        assert_eq!(response.city, "渋谷区");
        assert_eq!(response.street, "道玄坂1-1-1");
        assert_eq!(response.building, Some("ビル名".to_string()));
    }

    #[test]
    fn test_shipping_address_response_without_building() {
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        )
        .unwrap();

        let response = ShippingAddressResponse::from_shipping_address(&address);

        assert_eq!(response.building, None);
    }

    #[test]
    fn test_inventory_response_from_inventory() {
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 50);

        let response = InventoryResponse::from_inventory(&inventory);

        assert_eq!(response.book_id, book_id.to_string());
        assert_eq!(response.quantity_on_hand, 50);
    }

    #[test]
    fn test_order_detail_response_no_shipping_fee_for_large_order() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);

        // 高額な書籍を追加（配送料無料になる）
        let book_id = BookId::new();
        let price = Money::jpy(12000);
        order.add_book(book_id, 1, price).unwrap();

        let response = OrderDetailResponse::from_order(&order);

        assert_eq!(response.subtotal_amount, 12000);
        assert_eq!(response.shipping_fee_amount, 0); // 配送料無料
        assert_eq!(response.total_amount, 12000);
    }
}

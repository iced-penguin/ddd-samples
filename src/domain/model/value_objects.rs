use crate::domain::error::DomainError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::fmt;

/// 注文の一意識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(Uuid);

impl OrderId {
    /// 新しい一意のOrderIdを生成
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// UUIDから OrderId を作成
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 文字列からOrderIdを作成
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        let uuid = Uuid::parse_str(s)?;
        Ok(Self(uuid))
    }

    /// 内部のUUIDを取得
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

/// 書籍の一意識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BookId(Uuid);

impl BookId {
    /// 新しい一意のBookIdを生成
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// UUIDから BookId を作成
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 文字列からBookIdを作成
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        let uuid = Uuid::parse_str(s)?;
        Ok(Self(uuid))
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for BookId {
    fn default() -> Self {
        Self::new()
    }
}

/// 顧客の一意識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomerId(Uuid);

impl CustomerId {
    /// 新しい一意のCustomerIdを生成
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// UUIDから CustomerId を作成
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 文字列からCustomerIdを作成
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        let uuid = Uuid::parse_str(s)?;
        Ok(Self(uuid))
    }

    /// 内部のUUIDを取得
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for CustomerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for CustomerId {
    fn default() -> Self {
        Self::new()
    }
}

/// 通貨
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    /// 日本円
    #[allow(clippy::upper_case_acronyms)]
    JPY,
}

/// 金額を表す値オブジェクト
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    amount: i64,
    currency: Currency,
}

impl Money {
    /// 金額と通貨から作成
    pub fn new(amount: i64, currency: String) -> Result<Self, DomainError> {
        let currency = match currency.as_str() {
            "JPY" => Currency::JPY,
            _ => {
                return Err(DomainError::InvalidValue(format!(
                    "サポートされていない通貨: {}",
                    currency
                )))
            }
        };
        Ok(Self { amount, currency })
    }

    /// 日本円の金額を作成
    pub fn jpy(amount: i64) -> Self {
        Self {
            amount,
            currency: Currency::JPY,
        }
    }

    /// 金額を取得
    pub fn amount(&self) -> i64 {
        self.amount
    }

    /// 通貨を文字列として取得
    pub fn currency(&self) -> String {
        match self.currency {
            Currency::JPY => "JPY".to_string(),
        }
    }

    /// 金額を加算
    pub fn add(&self, other: &Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch);
        }
        Ok(Money {
            amount: self.amount + other.amount,
            currency: self.currency,
        })
    }

    /// 金額を乗算
    pub fn multiply(&self, factor: u32) -> Money {
        Money {
            amount: self.amount * factor as i64,
            currency: self.currency,
        }
    }
}

/// 注文明細を表す値オブジェクト
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderLine {
    book_id: BookId,
    quantity: u32,
    unit_price: Money,
}

impl OrderLine {
    /// 新しい注文明細を作成
    /// 数量は1以上である必要がある
    pub fn new(book_id: BookId, quantity: u32, unit_price: Money) -> Result<Self, DomainError> {
        if quantity == 0 {
            return Err(DomainError::InvalidQuantity);
        }
        Ok(Self {
            book_id,
            quantity,
            unit_price,
        })
    }

    /// 書籍IDを取得
    pub fn book_id(&self) -> BookId {
        self.book_id
    }

    /// 数量を取得
    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    /// 単価を取得
    pub fn unit_price(&self) -> Money {
        self.unit_price
    }

    /// 小計を計算（単価 × 数量）
    pub fn subtotal(&self) -> Money {
        self.unit_price.multiply(self.quantity)
    }

    /// 数量を増加させる（同じ書籍を追加する場合）
    pub fn increase_quantity(&mut self, additional_quantity: u32) -> Result<(), DomainError> {
        if additional_quantity == 0 {
            return Err(DomainError::InvalidQuantity);
        }
        self.quantity += additional_quantity;
        Ok(())
    }
}

/// 配送先住所を表す値オブジェクト
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShippingAddress {
    postal_code: String,
    prefecture: String,
    city: String,
    street: String,
    building: Option<String>,
}

impl ShippingAddress {
    /// 新しい配送先住所を作成
    /// バリデーション:
    /// - 郵便番号は7桁の数字である必要がある
    /// - 都道府県、市区町村、番地は空でない必要がある
    pub fn new(
        postal_code: String,
        prefecture: String,
        city: String,
        street: String,
        building: Option<String>,
    ) -> Result<Self, DomainError> {
        // 郵便番号のバリデーション（7桁の数字）
        if !Self::is_valid_postal_code(&postal_code) {
            return Err(DomainError::InvalidAddress(
                "郵便番号は7桁の数字である必要があります".to_string(),
            ));
        }

        // 必須フィールドのバリデーション
        if prefecture.trim().is_empty() {
            return Err(DomainError::InvalidAddress(
                "都道府県は空にできません".to_string(),
            ));
        }
        if city.trim().is_empty() {
            return Err(DomainError::InvalidAddress(
                "市区町村は空にできません".to_string(),
            ));
        }
        if street.trim().is_empty() {
            return Err(DomainError::InvalidAddress(
                "番地は空にできません".to_string(),
            ));
        }

        Ok(Self {
            postal_code,
            prefecture,
            city,
            street,
            building,
        })
    }

    /// 郵便番号が有効かチェック（7桁の数字）
    fn is_valid_postal_code(postal_code: &str) -> bool {
        postal_code.len() == 7 && postal_code.chars().all(|c| c.is_ascii_digit())
    }

    /// 郵便番号を取得
    pub fn postal_code(&self) -> &str {
        &self.postal_code
    }

    /// 都道府県を取得
    pub fn prefecture(&self) -> &str {
        &self.prefecture
    }

    /// 市区町村を取得
    pub fn city(&self) -> &str {
        &self.city
    }

    /// 番地を取得
    pub fn street(&self) -> &str {
        &self.street
    }

    /// 建物名を取得
    pub fn building(&self) -> Option<&str> {
        self.building.as_deref()
    }
}

/// 注文のステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    /// 保留中（作成直後）
    Pending,
    /// 確認済み（在庫予約済み）
    Confirmed,
    /// 発送済み
    Shipped,
    /// 配達完了
    Delivered,
    /// キャンセル済み
    Cancelled,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            OrderStatus::Pending => "Pending",
            OrderStatus::Confirmed => "Confirmed",
            OrderStatus::Shipped => "Shipped",
            OrderStatus::Delivered => "Delivered",
            OrderStatus::Cancelled => "Cancelled",
        };
        write!(f, "{}", status_str)
    }
}

impl OrderStatus {
    /// 文字列からOrderStatusを作成
    pub fn from_string(s: &str) -> Result<Self, DomainError> {
        match s {
            "Pending" => Ok(OrderStatus::Pending),
            "Confirmed" => Ok(OrderStatus::Confirmed),
            "Shipped" => Ok(OrderStatus::Shipped),
            "Delivered" => Ok(OrderStatus::Delivered),
            "Cancelled" => Ok(OrderStatus::Cancelled),
            _ => Err(DomainError::InvalidValue(format!(
                "無効な注文ステータス: {}",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_id_creation() {
        let id1 = OrderId::new();
        let id2 = OrderId::new();
        assert_ne!(id1, id2, "Each OrderId should be unique");
    }

    #[test]
    fn test_money_addition() {
        let money1 = Money::jpy(1000);
        let money2 = Money::jpy(500);
        let result = money1.add(&money2).unwrap();
        assert_eq!(result.amount(), 1500);
    }

    #[test]
    fn test_money_multiplication() {
        let money = Money::jpy(100);
        let result = money.multiply(5);
        assert_eq!(result.amount(), 500);
    }

    #[test]
    fn test_order_line_creation() {
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        let line = OrderLine::new(book_id, 2, price).unwrap();
        assert_eq!(line.quantity(), 2);
        assert_eq!(line.subtotal().amount(), 2000);
    }

    #[test]
    fn test_order_line_invalid_quantity() {
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        let result = OrderLine::new(book_id, 0, price);
        assert!(result.is_err());
    }

    #[test]
    fn test_shipping_address_valid() {
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            Some("ビル名".to_string()),
        );
        assert!(address.is_ok());
    }

    #[test]
    fn test_shipping_address_invalid_postal_code() {
        let result = ShippingAddress::new(
            "12345".to_string(), // 7桁でない
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_shipping_address_empty_required_field() {
        let result = ShippingAddress::new(
            "1234567".to_string(),
            "".to_string(), // 空の都道府県
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        );
        assert!(result.is_err());
    }
}

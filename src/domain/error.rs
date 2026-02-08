/// ドメイン層のエラー型
/// ビジネスルール違反を表現する
#[derive(Debug, Clone, PartialEq)]
pub enum DomainError {
    /// 無効な注文状態（例: 発送済みの注文をキャンセルしようとした）
    InvalidOrderState(String),
    /// 在庫不足
    InsufficientInventory,
    /// 無効な数量（例: 0以下の数量）
    InvalidQuantity,
    /// 無効な住所（例: 郵便番号が7桁でない）
    InvalidAddress(String),
    /// 注文の検証失敗（例: 注文明細が空の状態で確定しようとした）
    OrderValidation(String),
    /// 通貨の不一致
    CurrencyMismatch,
    /// 無効な値
    InvalidValue(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidOrderState(msg) => write!(f, "Invalid order state: {}", msg),
            DomainError::InsufficientInventory => write!(f, "Insufficient inventory"),
            DomainError::InvalidQuantity => write!(f, "Invalid quantity"),
            DomainError::InvalidAddress(msg) => write!(f, "Invalid address: {}", msg),
            DomainError::OrderValidation(msg) => write!(f, "Order validation failed: {}", msg),
            DomainError::CurrencyMismatch => write!(f, "Currency mismatch"),
            DomainError::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
        }
    }
}

impl std::error::Error for DomainError {}

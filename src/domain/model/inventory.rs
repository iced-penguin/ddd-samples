use crate::domain::error::DomainError;
use crate::domain::model::BookId;

/// 在庫集約
/// 書籍の在庫数を管理する
#[derive(Debug, Clone, PartialEq)]
pub struct Inventory {
    book_id: BookId,
    quantity_on_hand: u32,
}

impl Inventory {
    /// 新しい在庫を作成
    ///
    /// # Arguments
    /// * `book_id` - 書籍ID
    /// * `quantity_on_hand` - 在庫数
    pub fn new(book_id: BookId, quantity_on_hand: u32) -> Self {
        Self {
            book_id,
            quantity_on_hand,
        }
    }

    /// 書籍IDを取得
    pub fn book_id(&self) -> BookId {
        self.book_id
    }

    /// 在庫数を取得
    pub fn quantity_on_hand(&self) -> u32 {
        self.quantity_on_hand
    }

    /// 在庫を予約する
    ///
    /// # Arguments
    /// * `quantity` - 予約する数量
    ///
    /// # Returns
    /// * `Ok(())` - 予約成功
    /// * `Err(DomainError::InsufficientInventory)` - 在庫不足
    pub fn reserve(&mut self, quantity: u32) -> Result<(), DomainError> {
        if !self.has_available_stock(quantity) {
            return Err(DomainError::InsufficientInventory);
        }
        self.quantity_on_hand -= quantity;
        Ok(())
    }

    /// 在庫を解放する（キャンセル時など）
    ///
    /// # Arguments
    /// * `quantity` - 解放する数量
    pub fn release(&mut self, quantity: u32) -> Result<(), DomainError> {
        self.quantity_on_hand += quantity;
        Ok(())
    }

    /// 指定された数量の在庫が利用可能かチェック
    ///
    /// # Arguments
    /// * `quantity` - チェックする数量
    ///
    /// # Returns
    /// * `true` - 在庫が十分にある
    /// * `false` - 在庫が不足している
    pub fn has_available_stock(&self, quantity: u32) -> bool {
        self.quantity_on_hand >= quantity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_creation() {
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 10);
        assert_eq!(inventory.book_id(), book_id);
        assert_eq!(inventory.quantity_on_hand(), 10);
    }

    #[test]
    fn test_reserve_success() {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, 10);
        let result = inventory.reserve(5);
        assert!(result.is_ok());
        assert_eq!(inventory.quantity_on_hand(), 5);
    }

    #[test]
    fn test_reserve_insufficient_inventory() {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, 5);
        let result = inventory.reserve(10);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), DomainError::InsufficientInventory);
        assert_eq!(inventory.quantity_on_hand(), 5); // 在庫数は変わらない
    }

    #[test]
    fn test_release() {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, 5);
        let result = inventory.release(3);
        assert!(result.is_ok());
        assert_eq!(inventory.quantity_on_hand(), 8);
    }

    #[test]
    fn test_has_available_stock() {
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 10);
        assert!(inventory.has_available_stock(5));
        assert!(inventory.has_available_stock(10));
        assert!(!inventory.has_available_stock(11));
    }

    #[test]
    fn test_reserve_exact_quantity() {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, 10);
        let result = inventory.reserve(10);
        assert!(result.is_ok());
        assert_eq!(inventory.quantity_on_hand(), 0);
    }
}

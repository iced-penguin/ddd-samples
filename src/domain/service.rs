// ドメインサービス
// 複数の集約にまたがるビジネスロジックを実装

use crate::domain::error::DomainError;
use crate::domain::model::Order;
use crate::domain::port::InventoryRepository;

/// 在庫サービス
/// 注文確定時の在庫予約、キャンセル時の在庫解放を担当
pub struct InventoryService<R: InventoryRepository> {
    inventory_repository: R,
}

impl<R: InventoryRepository> InventoryService<R> {
    /// 新しい在庫サービスを作成
    ///
    /// # Arguments
    /// * `inventory_repository` - 在庫リポジトリ
    pub fn new(inventory_repository: R) -> Self {
        Self {
            inventory_repository,
        }
    }

    /// 注文の全書籍の在庫を予約する
    ///
    /// # Arguments
    /// * `order` - 在庫を予約する注文
    ///
    /// # Returns
    /// * `Ok(())` - 予約成功
    /// * `Err(DomainError)` - 予約失敗（在庫不足など）
    pub async fn reserve_inventory_for_order(&self, order: &Order) -> Result<(), DomainError> {
        // 注文の各注文明細について在庫を予約
        for order_line in order.order_lines() {
            let book_id = order_line.book_id();
            let quantity = order_line.quantity();

            // 在庫を取得
            let mut inventory = self
                .inventory_repository
                .find_by_book_id(book_id)
                .await
                .map_err(|e| DomainError::RepositoryError(format!("在庫の取得に失敗: {}", e)))?
                .ok_or_else(|| DomainError::InsufficientInventory)?;

            // 在庫を予約
            inventory.reserve(quantity)?;

            // 在庫を保存
            self.inventory_repository
                .save(&inventory)
                .await
                .map_err(|e| DomainError::RepositoryError(format!("在庫の保存に失敗: {}", e)))?;
        }

        Ok(())
    }

    /// 注文の全書籍の在庫を解放する（キャンセル時など）
    ///
    /// # Arguments
    /// * `order` - 在庫を解放する注文
    ///
    /// # Returns
    /// * `Ok(())` - 解放成功
    /// * `Err(DomainError)` - 解放失敗
    pub async fn release_inventory_for_order(&self, order: &Order) -> Result<(), DomainError> {
        // 注文の各注文明細について在庫を解放
        for order_line in order.order_lines() {
            let book_id = order_line.book_id();
            let quantity = order_line.quantity();

            // 在庫を取得
            let mut inventory = self
                .inventory_repository
                .find_by_book_id(book_id)
                .await
                .map_err(|e| DomainError::RepositoryError(format!("在庫の取得に失敗: {}", e)))?
                .ok_or_else(|| DomainError::InsufficientInventory)?;

            // 在庫を解放
            inventory.release(quantity)?;

            // 在庫を保存
            self.inventory_repository
                .save(&inventory)
                .await
                .map_err(|e| DomainError::RepositoryError(format!("在庫の保存に失敗: {}", e)))?;
        }

        Ok(())
    }

    /// 指定された書籍の在庫が存在するかチェックする
    ///
    /// # Arguments
    /// * `book_id` - 書籍ID
    ///
    /// # Returns
    /// * `Ok(true)` - 在庫が存在する
    /// * `Ok(false)` - 在庫が存在しない
    /// * `Err(DomainError)` - チェック失敗
    pub async fn book_exists(
        &self,
        book_id: &crate::domain::model::BookId,
    ) -> Result<bool, DomainError> {
        let inventory = self
            .inventory_repository
            .find_by_book_id(*book_id)
            .await
            .map_err(|e| DomainError::RepositoryError(format!("在庫の取得に失敗: {}", e)))?;

        Ok(inventory.is_some())
    }
}

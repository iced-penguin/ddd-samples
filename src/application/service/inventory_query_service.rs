use crate::application::ApplicationError;
use crate::domain::model::{Inventory, BookId};
use crate::domain::port::InventoryRepository;
use std::sync::Arc;

/// 在庫クエリサービス
/// 読み取り専用の在庫操作を提供する
/// 要件: 4.1, 5.1, 6.1
pub struct InventoryQueryService {
    inventory_repository: Arc<dyn InventoryRepository>,
}

impl InventoryQueryService {
    /// 新しい在庫クエリサービスを作成
    /// 
    /// # Arguments
    /// * `inventory_repository` - 在庫リポジトリ
    pub fn new(inventory_repository: Arc<dyn InventoryRepository>) -> Self {
        Self {
            inventory_repository,
        }
    }

    /// 書籍IDで在庫を取得
    /// 
    /// # Arguments
    /// * `book_id` - 書籍ID
    /// 
    /// # Returns
    /// * `Ok(Some(Inventory))` - 在庫が見つかった
    /// * `Ok(None)` - 在庫が見つからなかった
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 4.1
    pub async fn get_inventory_by_book_id(&self, book_id: BookId) -> Result<Option<Inventory>, ApplicationError> {
        self.inventory_repository.find_by_book_id(book_id).await
            .map_err(ApplicationError::from)
    }

    /// すべての在庫を取得
    /// 書籍IDの昇順で並べて返す
    /// 
    /// # Returns
    /// * `Ok(Vec<Inventory>)` - 在庫のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 5.1
    pub async fn get_all_inventories(&self) -> Result<Vec<Inventory>, ApplicationError> {
        self.inventory_repository.find_all().await
            .map_err(ApplicationError::from)
    }

    /// 指定された最大在庫数以下の在庫を取得
    /// 書籍IDの昇順で並べて返す
    /// 
    /// # Arguments
    /// * `max_quantity` - 最大在庫数（この数以下の在庫を取得）
    /// 
    /// # Returns
    /// * `Ok(Vec<Inventory>)` - 指定された条件の在庫のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 6.1
    pub async fn get_low_stock_inventories(&self, max_quantity: u32) -> Result<Vec<Inventory>, ApplicationError> {
        self.inventory_repository.find_by_max_quantity(max_quantity).await
            .map_err(ApplicationError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::port::RepositoryError;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // テスト用のモックリポジトリ
    struct MockInventoryRepository {
        inventories: Mutex<HashMap<BookId, Inventory>>,
    }

    impl MockInventoryRepository {
        fn new() -> Self {
            Self {
                inventories: Mutex::new(HashMap::new()),
            }
        }

        fn add_inventory(&self, inventory: Inventory) {
            let mut inventories = self.inventories.lock().unwrap();
            inventories.insert(inventory.book_id(), inventory);
        }
    }

    #[async_trait]
    impl InventoryRepository for MockInventoryRepository {
        async fn save(&self, inventory: &Inventory) -> Result<(), RepositoryError> {
            let mut inventories = self.inventories.lock().unwrap();
            inventories.insert(inventory.book_id(), inventory.clone());
            Ok(())
        }

        async fn find_by_book_id(&self, book_id: BookId) -> Result<Option<Inventory>, RepositoryError> {
            let inventories = self.inventories.lock().unwrap();
            Ok(inventories.get(&book_id).cloned())
        }

        async fn find_all(&self) -> Result<Vec<Inventory>, RepositoryError> {
            let inventories = self.inventories.lock().unwrap();
            let mut result: Vec<Inventory> = inventories.values().cloned().collect();
            // 書籍IDの昇順でソート
            result.sort_by(|a, b| a.book_id().to_string().cmp(&b.book_id().to_string()));
            Ok(result)
        }

        async fn find_by_max_quantity(&self, max_quantity: u32) -> Result<Vec<Inventory>, RepositoryError> {
            let inventories = self.inventories.lock().unwrap();
            let mut result: Vec<Inventory> = inventories.values()
                .filter(|inventory| inventory.quantity_on_hand() <= max_quantity)
                .cloned()
                .collect();
            // 書籍IDの昇順でソート
            result.sort_by(|a, b| a.book_id().to_string().cmp(&b.book_id().to_string()));
            Ok(result)
        }
    }

    #[tokio::test]
    async fn test_get_inventory_by_book_id_found() {
        let repository = Arc::new(MockInventoryRepository::new());
        let service = InventoryQueryService::new(repository.clone());

        // テスト用の在庫を作成
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, 10);
        repository.add_inventory(inventory.clone());

        // 在庫を取得
        let result = service.get_inventory_by_book_id(book_id).await;
        assert!(result.is_ok());
        let found_inventory = result.unwrap();
        assert!(found_inventory.is_some());
        let found = found_inventory.unwrap();
        assert_eq!(found.book_id(), book_id);
        assert_eq!(found.quantity_on_hand(), 10);
    }

    #[tokio::test]
    async fn test_get_inventory_by_book_id_not_found() {
        let repository = Arc::new(MockInventoryRepository::new());
        let service = InventoryQueryService::new(repository);

        let book_id = BookId::new();
        let result = service.get_inventory_by_book_id(book_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_all_inventories() {
        let repository = Arc::new(MockInventoryRepository::new());
        let service = InventoryQueryService::new(repository.clone());

        // テスト用の在庫を複数作成
        let inventory1 = Inventory::new(BookId::new(), 10);
        let inventory2 = Inventory::new(BookId::new(), 5);
        repository.add_inventory(inventory1);
        repository.add_inventory(inventory2);

        // すべての在庫を取得
        let result = service.get_all_inventories().await;
        assert!(result.is_ok());
        let inventories = result.unwrap();
        assert_eq!(inventories.len(), 2);
    }

    #[tokio::test]
    async fn test_get_low_stock_inventories() {
        let repository = Arc::new(MockInventoryRepository::new());
        let service = InventoryQueryService::new(repository.clone());

        // 異なる在庫数の在庫を作成
        let inventory1 = Inventory::new(BookId::new(), 3); // 低在庫
        let inventory2 = Inventory::new(BookId::new(), 10); // 通常在庫
        let inventory3 = Inventory::new(BookId::new(), 1); // 低在庫
        repository.add_inventory(inventory1);
        repository.add_inventory(inventory2);
        repository.add_inventory(inventory3);

        // 在庫数5以下の在庫のみを取得
        let result = service.get_low_stock_inventories(5).await;
        assert!(result.is_ok());
        let inventories = result.unwrap();
        assert_eq!(inventories.len(), 2);
        
        // すべての在庫が5以下であることを確認
        for inventory in inventories {
            assert!(inventory.quantity_on_hand() <= 5);
        }
    }

    #[tokio::test]
    async fn test_get_low_stock_inventories_none_match() {
        let repository = Arc::new(MockInventoryRepository::new());
        let service = InventoryQueryService::new(repository.clone());

        // 高在庫の在庫のみを作成
        let inventory1 = Inventory::new(BookId::new(), 10);
        let inventory2 = Inventory::new(BookId::new(), 15);
        repository.add_inventory(inventory1);
        repository.add_inventory(inventory2);

        // 在庫数5以下の在庫を取得（該当なし）
        let result = service.get_low_stock_inventories(5).await;
        assert!(result.is_ok());
        let inventories = result.unwrap();
        assert_eq!(inventories.len(), 0);
    }
}
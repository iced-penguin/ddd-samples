use crate::domain::model::{Inventory, BookId};
use crate::domain::port::{InventoryRepository, RepositoryError};
use crate::adapter::database_error::DatabaseError;
use async_trait::async_trait;

// MySQL関連のインポート
use sqlx::{MySql, Pool, Row};

/// MySQL在庫リポジトリ
/// MySQLデータベースを使用して在庫を永続化する
#[derive(Clone)]
pub struct MySqlInventoryRepository {
    pool: Pool<MySql>,
}

impl MySqlInventoryRepository {
    /// 新しいMySQL在庫リポジトリを作成
    /// 
    /// # Arguments
    /// * `pool` - MySQLコネクションプール
    /// 
    /// # Returns
    /// * MySqlInventoryRepositoryのインスタンス
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryRepository for MySqlInventoryRepository {
    async fn save(&self, inventory: &Inventory) -> Result<(), RepositoryError> {
        // 在庫データをinventoriesテーブルにUPSERT
        sqlx::query(
            r#"
            INSERT INTO inventories (book_id, quantity_on_hand)
            VALUES (?, ?)
            ON DUPLICATE KEY UPDATE
                quantity_on_hand = VALUES(quantity_on_hand)
            "#
        )
        .bind(inventory.book_id().to_string())
        .bind(inventory.quantity_on_hand())
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("在庫の保存に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        Ok(())
    }

    async fn find_by_book_id(&self, book_id: BookId) -> Result<Option<Inventory>, RepositoryError> {
        // inventoriesテーブルから在庫を取得
        let row = sqlx::query("SELECT book_id, quantity_on_hand FROM inventories WHERE book_id = ?")
            .bind(book_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("在庫の取得に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        match row {
            Some(row) => {
                let book_id = BookId::from_string(row.get("book_id"))
                    .map_err(|e| RepositoryError::FetchFailed(format!("書籍IDの解析に失敗しました: {}", e)))?;
                
                let inventory = Inventory::new(book_id, row.get::<u32, _>("quantity_on_hand"));
                Ok(Some(inventory))
            }
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Inventory>, RepositoryError> {
        // inventoriesテーブルからすべての在庫を取得
        // 書籍IDの昇順で並べる
        let rows = sqlx::query("SELECT book_id, quantity_on_hand FROM inventories ORDER BY book_id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("在庫一覧の取得に失敗しました: {}", e)))
            .map_err(RepositoryError::from)?;

        let mut inventories = Vec::new();
        for row in rows {
            let book_id = BookId::from_string(row.get("book_id"))
                .map_err(|e| RepositoryError::FetchFailed(format!("書籍IDの解析に失敗しました: {}", e)))?;
            
            let inventory = Inventory::new(book_id, row.get::<u32, _>("quantity_on_hand"));
            inventories.push(inventory);
        }

        Ok(inventories)
    }

    async fn find_by_max_quantity(&self, max_quantity: u32) -> Result<Vec<Inventory>, RepositoryError> {
        // 指定された最大在庫数以下の在庫を取得
        // 書籍IDの昇順で並べる
        let rows = sqlx::query(
            "SELECT book_id, quantity_on_hand FROM inventories WHERE quantity_on_hand <= ? ORDER BY book_id ASC"
        )
        .bind(max_quantity)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("在庫フィルタリングの取得に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        let mut inventories = Vec::new();
        for row in rows {
            let book_id = BookId::from_string(row.get("book_id"))
                .map_err(|e| RepositoryError::FetchFailed(format!("書籍IDの解析に失敗しました: {}", e)))?;
            
            let inventory = Inventory::new(book_id, row.get::<u32, _>("quantity_on_hand"));
            inventories.push(inventory);
        }

        Ok(inventories)
    }
}
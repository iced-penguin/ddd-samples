use sqlx::{MySql, Pool};
use crate::adapter::database_error::DatabaseError;

/// データベースマイグレーションを管理する構造体
pub struct DatabaseMigration {
    pool: Pool<MySql>,
}

impl DatabaseMigration {
    /// 新しいDatabaseMigrationインスタンスを作成
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// マイグレーションを実行
    /// べき等性を保証（CREATE TABLE IF NOT EXISTS）
    pub async fn run(&self) -> Result<(), DatabaseError> {
        // マイグレーションファイルのリスト
        let migrations = vec![
            include_str!("../../migrations/001_create_orders_table.sql"),
            include_str!("../../migrations/002_create_order_lines_table.sql"),
            include_str!("../../migrations/003_create_inventories_table.sql"),
        ];

        // 各マイグレーションを順番に実行
        for (index, migration_sql) in migrations.iter().enumerate() {
            println!("Running migration {}...", index + 1);
            sqlx::query(migration_sql)
                .execute(&self.pool)
                .await
                .map_err(|e| DatabaseError::MigrationError(format!("Migration {} failed: {}", index + 1, e)))?;
            println!("Migration {} completed successfully", index + 1);
        }

        println!("All migrations completed successfully");
        Ok(())
    }
}

use crate::adapter::database_error::DatabaseError;
use crate::domain::port::Logger;
use sqlx::{MySql, Pool};
use std::collections::HashMap;
use std::sync::Arc;

/// データベースマイグレーションを管理する構造体
pub struct DatabaseMigration {
    pool: Pool<MySql>,
    logger: Arc<dyn Logger>,
}

impl DatabaseMigration {
    /// 新しいDatabaseMigrationインスタンスを作成
    pub fn new(pool: Pool<MySql>, logger: Arc<dyn Logger>) -> Self {
        Self { pool, logger }
    }

    /// マイグレーションを実行
    /// べき等性を保証（CREATE TABLE IF NOT EXISTS）
    pub async fn run(&self) -> Result<(), DatabaseError> {
        // マイグレーションファイルのリスト
        let migrations = [
            include_str!("../../migrations/001_create_orders_table.sql"),
            include_str!("../../migrations/002_create_order_lines_table.sql"),
            include_str!("../../migrations/003_create_inventories_table.sql"),
        ];

        // 各マイグレーションを順番に実行
        for (index, migration_sql) in migrations.iter().enumerate() {
            let mut context = HashMap::new();
            context.insert("migration_index".to_string(), (index + 1).to_string());
            context.insert("status".to_string(), "starting".to_string());
            
            self.logger.debug(
                "DatabaseMigration",
                &format!("Migration {} starting", index + 1),
                None,
                Some(context),
            );

            sqlx::query(migration_sql)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    DatabaseError::MigrationError(format!("Migration {} failed: {}", index + 1, e))
                })?;

            let mut context = HashMap::new();
            context.insert("migration_index".to_string(), (index + 1).to_string());
            context.insert("status".to_string(), "completed successfully".to_string());
            
            self.logger.debug(
                "DatabaseMigration",
                &format!("Migration {} completed successfully", index + 1),
                None,
                Some(context),
            );
        }

        self.logger.debug(
            "DatabaseMigration",
            "All migrations completed successfully",
            None,
            None,
        );
        Ok(())
    }
}

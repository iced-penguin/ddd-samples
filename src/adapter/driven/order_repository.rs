use crate::adapter::database_error::DatabaseError;
use crate::domain::model::{Order, OrderId};
use crate::domain::port::{OrderRepository, RepositoryError};
use async_trait::async_trait;

// MySQL関連のインポート
use crate::domain::model::{BookId, CustomerId, Money, OrderLine, OrderStatus, ShippingAddress};
use sqlx::{MySql, Pool, Row};

/// MySQL注文リポジトリ
/// MySQLデータベースを使用して注文を永続化する
pub struct MySqlOrderRepository {
    pool: Pool<MySql>,
}

impl MySqlOrderRepository {
    /// 新しいMySQL注文リポジトリを作成
    ///
    /// # Arguments
    /// * `pool` - MySQLコネクションプール
    ///
    /// # Returns
    /// * MySqlOrderRepositoryのインスタンス
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// データベースの行から注文オブジェクトのリストを構築する
    /// JOINされた結果から複数の注文を再構築する
    async fn build_orders_from_rows(
        &self,
        rows: Vec<sqlx::mysql::MySqlRow>,
    ) -> Result<Vec<Order>, RepositoryError> {
        use std::collections::HashMap;

        // 注文IDごとにグループ化
        let mut order_groups: HashMap<String, Vec<&sqlx::mysql::MySqlRow>> = HashMap::new();
        for row in &rows {
            let order_id: String = row.get("id");
            order_groups
                .entry(order_id)
                .or_insert_with(Vec::new)
                .push(row);
        }

        let mut orders = Vec::new();

        for (order_id_str, order_rows) in order_groups {
            if order_rows.is_empty() {
                continue;
            }

            // 最初の行から注文の基本情報を取得
            let first_row = order_rows[0];

            let order_id = OrderId::from_string(&order_id_str).map_err(|e| {
                RepositoryError::FetchFailed(format!("注文IDの解析に失敗しました: {}", e))
            })?;

            let customer_id =
                CustomerId::from_string(first_row.get("customer_id")).map_err(|e| {
                    RepositoryError::FetchFailed(format!("顧客IDの解析に失敗しました: {}", e))
                })?;

            let status = OrderStatus::from_string(first_row.get("status")).map_err(|e| {
                RepositoryError::FetchFailed(format!("注文ステータスの解析に失敗しました: {}", e))
            })?;

            // 配送先住所を再構築
            let shipping_address =
                if let (Some(postal_code), Some(prefecture), Some(city), Some(street)) = (
                    first_row.get::<Option<String>, _>("postal_code"),
                    first_row.get::<Option<String>, _>("prefecture"),
                    first_row.get::<Option<String>, _>("city"),
                    first_row.get::<Option<String>, _>("street"),
                ) {
                    Some(
                        ShippingAddress::new(
                            postal_code,
                            prefecture,
                            city,
                            street,
                            first_row.get::<Option<String>, _>("building"),
                        )
                        .map_err(|e| {
                            RepositoryError::FetchFailed(format!(
                                "配送先住所の構築に失敗しました: {}",
                                e
                            ))
                        })?,
                    )
                } else {
                    None
                };

            // 注文明細を再構築
            let mut order_lines = Vec::new();
            for row in &order_rows {
                if let (Some(book_id_str), Some(quantity), Some(amount), Some(currency)) = (
                    row.get::<Option<String>, _>("book_id"),
                    row.get::<Option<u32>, _>("quantity"),
                    row.get::<Option<i64>, _>("unit_price_amount"),
                    row.get::<Option<String>, _>("unit_price_currency"),
                ) {
                    let book_id = BookId::from_string(&book_id_str).map_err(|e| {
                        RepositoryError::FetchFailed(format!("書籍IDの解析に失敗しました: {}", e))
                    })?;

                    let unit_price = Money::new(amount, currency).map_err(|e| {
                        RepositoryError::FetchFailed(format!("金額の構築に失敗しました: {}", e))
                    })?;

                    let order_line =
                        OrderLine::new(book_id, quantity as u32, unit_price).map_err(|e| {
                            RepositoryError::FetchFailed(format!(
                                "注文明細の構築に失敗しました: {}",
                                e
                            ))
                        })?;

                    order_lines.push(order_line);
                }
            }

            // 注文集約を再構築
            let order =
                Order::reconstruct(order_id, customer_id, order_lines, shipping_address, status)
                    .map_err(|e| {
                        RepositoryError::FetchFailed(format!(
                            "注文集約の再構築に失敗しました: {}",
                            e
                        ))
                    })?;

            orders.push(order);
        }

        Ok(orders)
    }
}

#[async_trait]
impl OrderRepository for MySqlOrderRepository {
    async fn save(&self, order: &Order) -> Result<(), RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| {
                DatabaseError::ConnectionError(format!("トランザクション開始に失敗しました: {}", e))
            })
            .map_err(RepositoryError::from)?;

        // 注文データをordersテーブルにUPSERT
        let shipping_address = order.shipping_address();
        let (postal_code, prefecture, city, street, building) = match shipping_address {
            Some(addr) => (
                Some(addr.postal_code()),
                Some(addr.prefecture()),
                Some(addr.city()),
                Some(addr.street()),
                addr.building(),
            ),
            None => (None, None, None, None, None),
        };

        sqlx::query(
            r#"
            INSERT INTO orders (id, customer_id, status, postal_code, prefecture, city, street, building)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                status = VALUES(status),
                postal_code = VALUES(postal_code),
                prefecture = VALUES(prefecture),
                city = VALUES(city),
                street = VALUES(street),
                building = VALUES(building)
            "#
        )
        .bind(order.id().to_string())
        .bind(order.customer_id().to_string())
        .bind(order.status().to_string())
        .bind(postal_code)
        .bind(prefecture)
        .bind(city)
        .bind(street)
        .bind(building)
        .execute(&mut *tx)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("注文の保存に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        // 既存の注文明細を削除
        sqlx::query("DELETE FROM order_lines WHERE order_id = ?")
            .bind(order.id().to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("注文明細の削除に失敗しました: {}", e)))
            .map_err(RepositoryError::from)?;

        // 注文明細データをorder_linesテーブルにINSERT
        for order_line in order.order_lines() {
            sqlx::query(
                r#"
                INSERT INTO order_lines (order_id, book_id, quantity, unit_price_amount, unit_price_currency)
                VALUES (?, ?, ?, ?, ?)
                "#
            )
            .bind(order.id().to_string())
            .bind(order_line.book_id().to_string())
            .bind(order_line.quantity())
            .bind(order_line.unit_price().amount())
            .bind(order_line.unit_price().currency())
            .execute(&mut *tx)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("注文明細の保存に失敗しました: {}", e)))
            .map_err(RepositoryError::from)?;
        }

        // トランザクションをコミット
        tx.commit()
            .await
            .map_err(|e| {
                DatabaseError::QueryError(format!(
                    "トランザクションのコミットに失敗しました: {}",
                    e
                ))
            })
            .map_err(RepositoryError::from)?;

        Ok(())
    }

    async fn find_by_id(&self, order_id: OrderId) -> Result<Option<Order>, RepositoryError> {
        // ordersテーブルとorder_linesテーブルをJOINして取得
        let rows = sqlx::query(
            r#"
            SELECT 
                o.id, o.customer_id, o.status,
                o.postal_code, o.prefecture, o.city, o.street, o.building,
                ol.book_id, ol.quantity, ol.unit_price_amount, ol.unit_price_currency
            FROM orders o
            LEFT JOIN order_lines ol ON o.id = ol.order_id
            WHERE o.id = ?
            "#,
        )
        .bind(order_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("注文の取得に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        if rows.is_empty() {
            return Ok(None);
        }

        // 最初の行から注文の基本情報を取得
        let first_row = &rows[0];
        let customer_id = CustomerId::from_string(first_row.get("customer_id")).map_err(|e| {
            RepositoryError::FetchFailed(format!("顧客IDの解析に失敗しました: {}", e))
        })?;

        let status = OrderStatus::from_string(first_row.get("status")).map_err(|e| {
            RepositoryError::FetchFailed(format!("注文ステータスの解析に失敗しました: {}", e))
        })?;

        // 配送先住所を再構築
        let shipping_address =
            if let (Some(postal_code), Some(prefecture), Some(city), Some(street)) = (
                first_row.get::<Option<String>, _>("postal_code"),
                first_row.get::<Option<String>, _>("prefecture"),
                first_row.get::<Option<String>, _>("city"),
                first_row.get::<Option<String>, _>("street"),
            ) {
                Some(
                    ShippingAddress::new(
                        postal_code,
                        prefecture,
                        city,
                        street,
                        first_row.get::<Option<String>, _>("building"),
                    )
                    .map_err(|e| {
                        RepositoryError::FetchFailed(format!(
                            "配送先住所の構築に失敗しました: {}",
                            e
                        ))
                    })?,
                )
            } else {
                None
            };

        // 注文明細を再構築
        let mut order_lines = Vec::new();
        for row in &rows {
            if let (Some(book_id_str), Some(quantity), Some(amount), Some(currency)) = (
                row.get::<Option<String>, _>("book_id"),
                row.get::<Option<u32>, _>("quantity"),
                row.get::<Option<i64>, _>("unit_price_amount"),
                row.get::<Option<String>, _>("unit_price_currency"),
            ) {
                let book_id = BookId::from_string(&book_id_str).map_err(|e| {
                    RepositoryError::FetchFailed(format!("書籍IDの解析に失敗しました: {}", e))
                })?;

                let unit_price = Money::new(amount, currency).map_err(|e| {
                    RepositoryError::FetchFailed(format!("金額の構築に失敗しました: {}", e))
                })?;

                let order_line =
                    OrderLine::new(book_id, quantity as u32, unit_price).map_err(|e| {
                        RepositoryError::FetchFailed(format!("注文明細の構築に失敗しました: {}", e))
                    })?;

                order_lines.push(order_line);
            }
        }

        // 注文集約を再構築
        let order =
            Order::reconstruct(order_id, customer_id, order_lines, shipping_address, status)
                .map_err(|e| {
                    RepositoryError::FetchFailed(format!("注文集約の再構築に失敗しました: {}", e))
                })?;

        Ok(Some(order))
    }

    async fn find_all(&self) -> Result<Vec<Order>, RepositoryError> {
        // ordersテーブルとorder_linesテーブルをJOINして全注文を取得
        // 作成日時の降順で並べる
        let rows = sqlx::query(
            r#"
            SELECT 
                o.id, o.customer_id, o.status,
                o.postal_code, o.prefecture, o.city, o.street, o.building,
                ol.book_id, ol.quantity, ol.unit_price_amount, ol.unit_price_currency
            FROM orders o
            LEFT JOIN order_lines ol ON o.id = ol.order_id
            ORDER BY o.created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("注文一覧の取得に失敗しました: {}", e)))
        .map_err(RepositoryError::from)?;

        self.build_orders_from_rows(rows).await
    }

    async fn find_by_status(&self, status: OrderStatus) -> Result<Vec<Order>, RepositoryError> {
        // 指定されたステータスの注文を取得
        // 作成日時の降順で並べる
        let rows = sqlx::query(
            r#"
            SELECT 
                o.id, o.customer_id, o.status,
                o.postal_code, o.prefecture, o.city, o.street, o.building,
                ol.book_id, ol.quantity, ol.unit_price_amount, ol.unit_price_currency
            FROM orders o
            LEFT JOIN order_lines ol ON o.id = ol.order_id
            WHERE o.status = ?
            ORDER BY o.created_at DESC
            "#,
        )
        .bind(status.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            DatabaseError::QueryError(format!("ステータス別注文一覧の取得に失敗しました: {}", e))
        })
        .map_err(RepositoryError::from)?;

        self.build_orders_from_rows(rows).await
    }

    fn next_identity(&self) -> OrderId {
        OrderId::new()
    }
}

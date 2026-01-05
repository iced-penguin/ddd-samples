use crate::application::ApplicationError;
use crate::domain::model::{Order, OrderId, OrderStatus};
use crate::domain::port::OrderRepository;
use std::sync::Arc;

/// 注文クエリサービス
/// 読み取り専用の注文操作を提供する
/// 要件: 1.1, 2.1, 3.1
pub struct OrderQueryService {
    order_repository: Arc<dyn OrderRepository>,
}

impl OrderQueryService {
    /// 新しい注文クエリサービスを作成
    /// 
    /// # Arguments
    /// * `order_repository` - 注文リポジトリ
    pub fn new(order_repository: Arc<dyn OrderRepository>) -> Self {
        Self {
            order_repository,
        }
    }

    /// 注文IDで注文を取得
    /// 
    /// # Arguments
    /// * `id` - 注文ID
    /// 
    /// # Returns
    /// * `Ok(Some(Order))` - 注文が見つかった
    /// * `Ok(None)` - 注文が見つからなかった
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 1.1
    pub async fn get_order_by_id(&self, id: OrderId) -> Result<Option<Order>, ApplicationError> {
        self.order_repository.find_by_id(id).await
            .map_err(ApplicationError::from)
    }

    /// すべての注文を取得
    /// 作成日時の降順で並べて返す
    /// 
    /// # Returns
    /// * `Ok(Vec<Order>)` - 注文のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 2.1
    pub async fn get_all_orders(&self) -> Result<Vec<Order>, ApplicationError> {
        self.order_repository.find_all().await
            .map_err(ApplicationError::from)
    }

    /// 指定されたステータスの注文を取得
    /// 作成日時の降順で並べて返す
    /// 
    /// # Arguments
    /// * `status` - フィルタリングする注文ステータス
    /// 
    /// # Returns
    /// * `Ok(Vec<Order>)` - 指定されたステータスの注文のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    /// 
    /// # 要件
    /// 3.1
    pub async fn get_orders_by_status(&self, status: OrderStatus) -> Result<Vec<Order>, ApplicationError> {
        self.order_repository.find_by_status(status).await
            .map_err(ApplicationError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{CustomerId, BookId, Money, ShippingAddress};
    use crate::domain::port::RepositoryError;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // テスト用のモックリポジトリ
    struct MockOrderRepository {
        orders: Mutex<HashMap<OrderId, Order>>,
    }

    impl MockOrderRepository {
        fn new() -> Self {
            Self {
                orders: Mutex::new(HashMap::new()),
            }
        }

        fn add_order(&self, order: Order) {
            let mut orders = self.orders.lock().unwrap();
            orders.insert(order.id(), order);
        }
    }

    #[async_trait]
    impl OrderRepository for MockOrderRepository {
        async fn save(&self, order: &Order) -> Result<(), RepositoryError> {
            let mut orders = self.orders.lock().unwrap();
            orders.insert(order.id(), order.clone());
            Ok(())
        }

        async fn find_by_id(&self, order_id: OrderId) -> Result<Option<Order>, RepositoryError> {
            let orders = self.orders.lock().unwrap();
            Ok(orders.get(&order_id).cloned())
        }

        async fn find_all(&self) -> Result<Vec<Order>, RepositoryError> {
            let orders = self.orders.lock().unwrap();
            Ok(orders.values().cloned().collect())
        }

        async fn find_by_status(&self, status: OrderStatus) -> Result<Vec<Order>, RepositoryError> {
            let orders = self.orders.lock().unwrap();
            Ok(orders.values()
                .filter(|order| order.status() == status)
                .cloned()
                .collect())
        }

        fn next_identity(&self) -> OrderId {
            OrderId::new()
        }
    }

    #[tokio::test]
    async fn test_get_order_by_id_found() {
        let repository = Arc::new(MockOrderRepository::new());
        let service = OrderQueryService::new(repository.clone());

        // テスト用の注文を作成
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let order = Order::new(order_id, customer_id);
        repository.add_order(order.clone());

        // 注文を取得
        let result = service.get_order_by_id(order_id).await;
        assert!(result.is_ok());
        let found_order = result.unwrap();
        assert!(found_order.is_some());
        assert_eq!(found_order.unwrap().id(), order_id);
    }

    #[tokio::test]
    async fn test_get_order_by_id_not_found() {
        let repository = Arc::new(MockOrderRepository::new());
        let service = OrderQueryService::new(repository);

        let order_id = OrderId::new();
        let result = service.get_order_by_id(order_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_all_orders() {
        let repository = Arc::new(MockOrderRepository::new());
        let service = OrderQueryService::new(repository.clone());

        // テスト用の注文を複数作成
        let order1 = Order::new(OrderId::new(), CustomerId::new());
        let order2 = Order::new(OrderId::new(), CustomerId::new());
        repository.add_order(order1);
        repository.add_order(order2);

        // すべての注文を取得
        let result = service.get_all_orders().await;
        assert!(result.is_ok());
        let orders = result.unwrap();
        assert_eq!(orders.len(), 2);
    }

    #[tokio::test]
    async fn test_get_orders_by_status() {
        let repository = Arc::new(MockOrderRepository::new());
        let service = OrderQueryService::new(repository.clone());

        // 異なるステータスの注文を作成
        let mut order1 = Order::new(OrderId::new(), CustomerId::new());
        let book_id = BookId::new();
        let price = Money::jpy(1000);
        order1.add_book(book_id, 1, price).unwrap();
        let address = ShippingAddress::new(
            "1234567".to_string(),
            "東京都".to_string(),
            "渋谷区".to_string(),
            "道玄坂1-1-1".to_string(),
            None,
        ).unwrap();
        order1.set_shipping_address(address);
        order1.confirm().unwrap(); // Confirmedステータス

        let order2 = Order::new(OrderId::new(), CustomerId::new()); // Pendingステータス

        repository.add_order(order1);
        repository.add_order(order2);

        // Confirmedステータスの注文のみを取得
        let result = service.get_orders_by_status(OrderStatus::Confirmed).await;
        assert!(result.is_ok());
        let orders = result.unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status(), OrderStatus::Confirmed);
    }
}
use crate::application::ApplicationError;
use crate::domain::event::{
    DomainEvent, OrderCancelled, OrderConfirmed, OrderDelivered, OrderShipped,
};
use crate::domain::model::{
    BookId, CustomerId, Inventory, Money, Order, OrderId, OrderStatus, ShippingAddress,
};
use crate::domain::port::{EventBus, InventoryRepository, OrderRepository};
use std::sync::Arc;
use uuid::Uuid;

/// 注文アプリケーションサービス
pub struct OrderApplicationService<OR>
where
    OR: OrderRepository,
{
    order_repository: OR,
    event_bus: Arc<dyn EventBus>,
}

impl<OR> OrderApplicationService<OR>
where
    OR: OrderRepository,
{
    /// 新しいアプリケーションサービスを作成
    ///
    /// # Arguments
    /// * `order_repository` - 注文リポジトリ
    /// * `event_bus` - イベントバス
    pub fn new(order_repository: OR, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            order_repository,
            event_bus,
        }
    }

    /// イベントに相関IDを設定するヘルパー関数
    fn set_correlation_id_to_event(
        &self,
        mut event: DomainEvent,
        correlation_id: Uuid,
    ) -> DomainEvent {
        match &mut event {
            DomainEvent::OrderConfirmed(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::OrderCancelled(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::OrderShipped(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::OrderDelivered(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::InventoryReserved(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::InventoryReleased(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::InventoryReservationFailed(ref mut e) => {
                e.metadata.correlation_id = correlation_id
            }
            DomainEvent::ShippingFailed(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::DeliveryFailed(ref mut e) => e.metadata.correlation_id = correlation_id,
            DomainEvent::SagaCompensationStarted(ref mut e) => {
                e.metadata.correlation_id = correlation_id
            }
            DomainEvent::SagaCompensationCompleted(ref mut e) => {
                e.metadata.correlation_id = correlation_id
            }
        }
        event
    }

    /// 新しい注文を作成
    ///
    /// # Arguments
    /// * `customer_id` - 顧客ID
    ///
    /// # Returns
    /// * `Ok(OrderId)` - 作成された注文のID
    /// * `Err(ApplicationError)` - 作成失敗
    pub async fn create_order(&self, customer_id: CustomerId) -> Result<OrderId, ApplicationError> {
        let order_id = self.order_repository.next_identity();
        let order = crate::domain::model::Order::new(order_id, customer_id);
        self.order_repository.save(&order).await?;
        Ok(order_id)
    }

    /// 注文に書籍を追加
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    /// * `book_id` - 書籍ID
    /// * `quantity` - 数量
    /// * `price` - 単価
    ///
    /// # Returns
    /// * `Ok(())` - 追加成功
    /// * `Err(ApplicationError)` - 追加失敗
    pub async fn add_book_to_order(
        &self,
        order_id: OrderId,
        book_id: BookId,
        quantity: u32,
        price: Money,
    ) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;
        order.add_book(book_id, quantity, price)?;
        self.order_repository.save(&order).await?;
        Ok(())
    }

    /// 注文に配送先住所を設定
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    /// * `postal_code` - 郵便番号
    /// * `prefecture` - 都道府県
    /// * `city` - 市区町村
    /// * `address_line1` - 住所1
    /// * `address_line2` - 住所2（オプション）
    ///
    /// # Returns
    /// * `Ok(())` - 設定成功
    /// * `Err(ApplicationError)` - 設定失敗
    pub async fn set_shipping_address_from_request(
        &self,
        order_id: OrderId,
        postal_code: String,
        prefecture: String,
        city: String,
        address_line1: String,
        address_line2: Option<String>,
    ) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;
        let address =
            ShippingAddress::new(postal_code, prefecture, city, address_line1, address_line2)?;
        order.set_shipping_address(address);
        self.order_repository.save(&order).await?;
        Ok(())
    }

    /// 注文を確定
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    ///
    /// # Returns
    /// * `Ok(())` - 確定成功
    /// * `Err(ApplicationError)` - 確定失敗
    pub async fn confirm_order(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;

        order.confirm()?;
        self.order_repository.save(&order).await?;

        let correlation_id = Uuid::new_v4();
        let total_amount = order.calculate_total();
        let event = OrderConfirmed::new(
            order.id(),
            order.customer_id(),
            order.order_lines().to_vec(),
            total_amount,
        );
        let event_with_correlation =
            self.set_correlation_id_to_event(DomainEvent::OrderConfirmed(event), correlation_id);

        self.event_bus
            .publish(event_with_correlation)
            .await
            .map_err(|e| ApplicationError::EventPublishingFailed(e.to_string()))?;

        Ok(())
    }

    /// 注文をキャンセル
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    ///
    /// # Returns
    /// * `Ok(())` - キャンセル成功
    /// * `Err(ApplicationError)` - キャンセル失敗
    pub async fn cancel_order(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;

        order.cancel()?;
        self.order_repository.save(&order).await?;

        let correlation_id = Uuid::new_v4();
        let event = OrderCancelled::new(
            order.id(),
            order.customer_id(),
            order.order_lines().to_vec(),
        );
        let event_with_correlation =
            self.set_correlation_id_to_event(DomainEvent::OrderCancelled(event), correlation_id);

        self.event_bus
            .publish(event_with_correlation)
            .await
            .map_err(|e| ApplicationError::EventPublishingFailed(e.to_string()))?;

        Ok(())
    }

    /// 注文を発送済みにマーク
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    ///
    /// # Returns
    /// * `Ok(())` - マーク成功
    /// * `Err(ApplicationError)` - マーク失敗
    pub async fn mark_order_as_shipped(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;

        order.mark_as_shipped()?;
        self.order_repository.save(&order).await?;

        let correlation_id = Uuid::new_v4();
        let shipping_address = order
            .shipping_address()
            .expect("Confirmed状態の注文には配送先住所が必須です")
            .clone();
        let event = OrderShipped::new(order.id(), shipping_address);
        let event_with_correlation =
            self.set_correlation_id_to_event(DomainEvent::OrderShipped(event), correlation_id);

        self.event_bus
            .publish(event_with_correlation)
            .await
            .map_err(|e| ApplicationError::EventPublishingFailed(e.to_string()))?;

        Ok(())
    }

    /// 注文を配達完了にマーク
    ///
    /// # Arguments
    /// * `order_id` - 注文ID
    ///
    /// # Returns
    /// * `Ok(())` - マーク成功
    /// * `Err(ApplicationError)` - マーク失敗
    pub async fn mark_order_as_delivered(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        let mut order = self
            .order_repository
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "注文が見つかりません: {}",
                    order_id
                ))
            })?;

        order.mark_as_delivered()?;
        self.order_repository.save(&order).await?;

        let correlation_id = Uuid::new_v4();
        let event = OrderDelivered::new(order.id());
        let event_with_correlation =
            self.set_correlation_id_to_event(DomainEvent::OrderDelivered(event), correlation_id);

        self.event_bus
            .publish(event_with_correlation)
            .await
            .map_err(|e| ApplicationError::EventPublishingFailed(e.to_string()))?;

        Ok(())
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
    pub async fn get_order_by_id(&self, id: OrderId) -> Result<Option<Order>, ApplicationError> {
        self.order_repository
            .find_by_id(id)
            .await
            .map_err(ApplicationError::from)
    }

    /// すべての注文を取得
    /// 作成日時の降順で並べて返す
    ///
    /// # Returns
    /// * `Ok(Vec<Order>)` - 注文のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    pub async fn get_all_orders(&self) -> Result<Vec<Order>, ApplicationError> {
        self.order_repository
            .find_all()
            .await
            .map_err(ApplicationError::from)
    }

    /// 指定されたステータス文字列の注文を取得
    /// 作成日時の降順で並べて返す
    ///
    /// # Arguments
    /// * `status_str` - フィルタリングする注文ステータス文字列
    ///
    /// # Returns
    /// * `Ok(Vec<Order>)` - 指定されたステータスの注文のリスト
    /// * `Err(ApplicationError)` - 取得失敗またはステータス文字列が無効
    pub async fn get_orders_by_status_string(
        &self,
        status_str: String,
    ) -> Result<Vec<Order>, ApplicationError> {
        let status = OrderStatus::from_string(&status_str).map_err(|_| {
            ApplicationError::NotFound(format!("無効なステータス値: {}", status_str))
        })?;

        self.get_orders_by_status(status).await
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
    pub async fn get_orders_by_status(
        &self,
        status: OrderStatus,
    ) -> Result<Vec<Order>, ApplicationError> {
        self.order_repository
            .find_by_status(status)
            .await
            .map_err(ApplicationError::from)
    }
}

/// 在庫アプリケーションサービス
pub struct InventoryApplicationService {
    inventory_repository: Arc<dyn InventoryRepository>,
}

impl InventoryApplicationService {
    /// 新しい在庫アプリケーションサービスを作成
    ///
    /// # Arguments
    /// * `inventory_repository` - 在庫リポジトリ
    pub fn new(inventory_repository: Arc<dyn InventoryRepository>) -> Self {
        Self {
            inventory_repository,
        }
    }

    /// 新しい在庫を作成
    ///
    /// # Arguments
    /// * `book_id` - 書籍ID
    /// * `quantity` - 初期在庫数
    ///
    /// # Returns
    /// * `Ok(())` - 作成成功
    /// * `Err(ApplicationError)` - 作成失敗
    pub async fn create_inventory(
        &self,
        book_id: BookId,
        quantity: u32,
    ) -> Result<(), ApplicationError> {
        let inventory = Inventory::new(book_id, quantity);
        self.inventory_repository
            .save(&inventory)
            .await
            .map_err(ApplicationError::from)
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
    pub async fn get_inventory_by_book_id(
        &self,
        book_id: BookId,
    ) -> Result<Option<Inventory>, ApplicationError> {
        self.inventory_repository
            .find_by_book_id(book_id)
            .await
            .map_err(ApplicationError::from)
    }

    /// すべての在庫を取得
    /// 書籍IDの昇順で並べて返す
    ///
    /// # Returns
    /// * `Ok(Vec<Inventory>)` - 在庫のリスト
    /// * `Err(ApplicationError)` - 取得失敗
    pub async fn get_all_inventories(&self) -> Result<Vec<Inventory>, ApplicationError> {
        self.inventory_repository
            .find_all()
            .await
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
    pub async fn get_low_stock_inventories(
        &self,
        max_quantity: u32,
    ) -> Result<Vec<Inventory>, ApplicationError> {
        self.inventory_repository
            .find_by_max_quantity(max_quantity)
            .await
            .map_err(ApplicationError::from)
    }
}

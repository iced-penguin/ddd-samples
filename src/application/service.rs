// アプリケーションサービス
// ユースケースの調整、トランザクション境界の管理、ドメインイベントの発行

pub mod order_query_service;
pub mod inventory_query_service;

pub use order_query_service::OrderQueryService;
pub use inventory_query_service::InventoryQueryService;

use crate::application::ApplicationError;
use crate::domain::model::{OrderId, CustomerId, BookId, Money, ShippingAddress};
use crate::domain::port::{OrderRepository, InventoryRepository, EventPublisher};
use crate::domain::service::InventoryService;

/// 注文アプリケーションサービス
/// ユースケースの調整を行い、ドメインイベントを発行する
pub struct OrderApplicationService<OR, IR, EP>
where
    OR: OrderRepository,
    IR: InventoryRepository,
    EP: EventPublisher,
{
    order_repository: OR,
    inventory_service: InventoryService<IR>,
    event_publisher: EP,
}

impl<OR, IR, EP> OrderApplicationService<OR, IR, EP>
where
    OR: OrderRepository,
    IR: InventoryRepository,
    EP: EventPublisher,
{
    /// 新しいアプリケーションサービスを作成
    /// 
    /// # Arguments
    /// * `order_repository` - 注文リポジトリ
    /// * `inventory_repository` - 在庫リポジトリ
    /// * `event_publisher` - イベント発行者
    pub fn new(order_repository: OR, inventory_repository: IR, event_publisher: EP) -> Self {
        let inventory_service = InventoryService::new(inventory_repository);
        Self {
            order_repository,
            inventory_service,
            event_publisher,
        }
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
        // 新しい注文IDを生成
        let order_id = self.order_repository.next_identity();

        // 新しい注文を作成（初期ステータスはPending）
        let order = crate::domain::model::Order::new(order_id, customer_id);

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        // 注文IDを返す
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
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 書籍の在庫が存在するかチェック
        let book_exists = self.inventory_service.book_exists(&book_id).await?;
        
        if !book_exists {
            return Err(ApplicationError::NotFound(format!("指定された書籍が見つかりません: {}", book_id.to_string())));
        }

        // 書籍を追加
        order.add_book(book_id, quantity, price)?;

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        Ok(())
    }

    /// 注文に配送先住所を設定
    /// 
    /// # Arguments
    /// * `order_id` - 注文ID
    /// * `address` - 配送先住所
    /// 
    /// # Returns
    /// * `Ok(())` - 設定成功
    /// * `Err(ApplicationError)` - 設定失敗
    pub async fn set_shipping_address(
        &self,
        order_id: OrderId,
        address: ShippingAddress,
    ) -> Result<(), ApplicationError> {
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 配送先住所を設定
        order.set_shipping_address(address);

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        Ok(())
    }

    /// 注文を確定
    /// 在庫を予約し、注文を確定状態にする
    /// 
    /// # Arguments
    /// * `order_id` - 注文ID
    /// 
    /// # Returns
    /// * `Ok(())` - 確定成功
    /// * `Err(ApplicationError)` - 確定失敗
    pub async fn confirm_order(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 在庫サービスで在庫を予約
        self.inventory_service.reserve_inventory_for_order(&order).await?;

        // 注文を確定
        order.confirm()?;

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        // ドメインイベントを発行
        let events = order.take_domain_events();
        for event in events {
            self.event_publisher.publish(&event)?;
        }

        Ok(())
    }

    /// 注文をキャンセル
    /// 注文をキャンセル状態にし、在庫を解放する
    /// 
    /// # Arguments
    /// * `order_id` - 注文ID
    /// 
    /// # Returns
    /// * `Ok(())` - キャンセル成功
    /// * `Err(ApplicationError)` - キャンセル失敗
    pub async fn cancel_order(&self, order_id: OrderId) -> Result<(), ApplicationError> {
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 注文をキャンセル
        order.cancel()?;

        // 在庫サービスで在庫を解放
        self.inventory_service.release_inventory_for_order(&order).await?;

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        // ドメインイベントを発行
        let events = order.take_domain_events();
        for event in events {
            self.event_publisher.publish(&event)?;
        }

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
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 発送済みにマーク
        order.mark_as_shipped()?;

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        // ドメインイベントを発行
        let events = order.take_domain_events();
        for event in events {
            self.event_publisher.publish(&event)?;
        }

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
        // 注文を取得
        let mut order = self.order_repository
            .find_by_id(order_id).await?
            .ok_or_else(|| ApplicationError::NotFound(format!("注文が見つかりません: {}", order_id.to_string())))?;

        // 配達完了にマーク
        order.mark_as_delivered()?;

        // リポジトリに保存
        self.order_repository.save(&order).await?;

        // ドメインイベントを発行
        let events = order.take_domain_events();
        for event in events {
            self.event_publisher.publish(&event)?;
        }

        Ok(())
    }
}
// 出力ポート
// ドメイン層が外部に依存する機能をトレイトとして定義
// アダプター層でこれらのトレイトを実装する

use crate::domain::model::{Order, OrderId, Inventory, BookId, OrderStatus};
use crate::domain::event::DomainEvent;
use async_trait::async_trait;

/// リポジトリエラー型
/// リポジトリ操作で発生するエラーを表現する
#[derive(Debug, Clone, PartialEq)]
pub enum RepositoryError {
    /// データベース接続に失敗
    ConnectionFailed(String),
    /// 操作に失敗
    OperationFailed(String),
    /// データの取得に失敗
    FetchFailed(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            RepositoryError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
            RepositoryError::FetchFailed(msg) => write!(f, "Fetch failed: {}", msg),
        }
    }
}

impl std::error::Error for RepositoryError {}

/// 注文リポジトリトレイト
/// 注文集約の永続化を抽象化する
#[async_trait]
pub trait OrderRepository: Send + Sync {
    /// 注文を保存する
    /// 
    /// # Arguments
    /// * `order` - 保存する注文
    /// 
    /// # Returns
    /// * `Ok(())` - 保存成功
    /// * `Err(RepositoryError)` - 保存失敗
    async fn save(&self, order: &Order) -> Result<(), RepositoryError>;

    /// 注文IDで注文を検索する
    /// 
    /// # Arguments
    /// * `order_id` - 検索する注文ID
    /// 
    /// # Returns
    /// * `Ok(Some(Order))` - 注文が見つかった
    /// * `Ok(None)` - 注文が見つからなかった
    /// * `Err(RepositoryError)` - 検索失敗
    async fn find_by_id(&self, order_id: OrderId) -> Result<Option<Order>, RepositoryError>;

    /// すべての注文を取得する
    /// 作成日時の降順で並べて返す
    /// 
    /// # Returns
    /// * `Ok(Vec<Order>)` - 注文のリスト
    /// * `Err(RepositoryError)` - 取得失敗
    async fn find_all(&self) -> Result<Vec<Order>, RepositoryError>;

    /// 指定されたステータスの注文を取得する
    /// 作成日時の降順で並べて返す
    /// 
    /// # Arguments
    /// * `status` - フィルタリングする注文ステータス
    /// 
    /// # Returns
    /// * `Ok(Vec<Order>)` - 指定されたステータスの注文のリスト
    /// * `Err(RepositoryError)` - 取得失敗
    async fn find_by_status(&self, status: OrderStatus) -> Result<Vec<Order>, RepositoryError>;

    /// 新しい一意の注文IDを生成する
    /// 
    /// # Returns
    /// * 新しい注文ID
    fn next_identity(&self) -> OrderId;
}

/// 在庫リポジトリトレイト
/// 在庫集約の永続化を抽象化する
#[async_trait]
pub trait InventoryRepository: Send + Sync {
    /// 在庫を保存する
    /// 
    /// # Arguments
    /// * `inventory` - 保存する在庫
    /// 
    /// # Returns
    /// * `Ok(())` - 保存成功
    /// * `Err(RepositoryError)` - 保存失敗
    async fn save(&self, inventory: &Inventory) -> Result<(), RepositoryError>;

    /// 書籍IDで在庫を検索する
    /// 
    /// # Arguments
    /// * `book_id` - 検索する書籍ID
    /// 
    /// # Returns
    /// * `Ok(Some(Inventory))` - 在庫が見つかった
    /// * `Ok(None)` - 在庫が見つからなかった
    /// * `Err(RepositoryError)` - 検索失敗
    async fn find_by_book_id(&self, book_id: BookId) -> Result<Option<Inventory>, RepositoryError>;

    /// すべての在庫を取得する
    /// 書籍IDの昇順で並べて返す
    /// 
    /// # Returns
    /// * `Ok(Vec<Inventory>)` - 在庫のリスト
    /// * `Err(RepositoryError)` - 取得失敗
    async fn find_all(&self) -> Result<Vec<Inventory>, RepositoryError>;

    /// 指定された最大在庫数以下の在庫を取得する
    /// 書籍IDの昇順で並べて返す
    /// 
    /// # Arguments
    /// * `max_quantity` - 最大在庫数（この数以下の在庫を取得）
    /// 
    /// # Returns
    /// * `Ok(Vec<Inventory>)` - 指定された条件の在庫のリスト
    /// * `Err(RepositoryError)` - 取得失敗
    async fn find_by_max_quantity(&self, max_quantity: u32) -> Result<Vec<Inventory>, RepositoryError>;
}

/// イベント発行者エラー型
/// イベント発行で発生するエラーを表現する
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PublisherError {
    /// イベントの発行に失敗
    PublishFailed(String),
}

impl std::fmt::Display for PublisherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublisherError::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
        }
    }
}

impl std::error::Error for PublisherError {}

/// イベント発行者トレイト
/// ドメインイベントの発行を抽象化する
pub trait EventPublisher {
    /// ドメインイベントを発行する
    /// 
    /// # Arguments
    /// * `event` - 発行するドメインイベント
    /// 
    /// # Returns
    /// * `Ok(())` - 発行成功
    /// * `Err(PublisherError)` - 発行失敗
    fn publish(&self, event: &DomainEvent) -> Result<(), PublisherError>;
}

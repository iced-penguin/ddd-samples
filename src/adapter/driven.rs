// 駆動される側アダプター（リポジトリ実装など）

mod order_repository;
mod inventory_repository;
mod event_publisher;

pub use order_repository::MySqlOrderRepository;
pub use inventory_repository::MySqlInventoryRepository;
pub use event_publisher::ConsoleEventPublisher;
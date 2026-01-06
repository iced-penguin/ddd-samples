// ドメインモデル（エンティティと値オブジェクト）

mod inventory;
mod order;
mod value_objects;

pub use value_objects::{
    BookId, CustomerId, Money, OrderId, OrderLine, OrderStatus, ShippingAddress,
};

pub use inventory::Inventory;
pub use order::Order;

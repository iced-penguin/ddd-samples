// ドメインモデル（エンティティと値オブジェクト）

mod value_objects;
mod order;
mod inventory;

pub use value_objects::{
    OrderId, BookId, CustomerId,
    Money, 
    OrderLine,
    ShippingAddress,
    OrderStatus,
};

pub use order::Order;
pub use inventory::Inventory;

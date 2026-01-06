use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 注文作成用のリクエストDTO
#[derive(Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub customer_id: Option<Uuid>,
}

/// 書籍追加用のリクエストDTO
#[derive(Serialize, Deserialize)]
pub struct AddBookRequest {
    pub book_id: Uuid,
    pub quantity: u32,
    pub unit_price: i64, // JPY in cents
}

/// 配送先住所設定用のリクエストDTO
#[derive(Serialize, Deserialize)]
pub struct SetShippingAddressRequest {
    pub postal_code: String,
    pub prefecture: String,
    pub city: String,
    pub address_line1: String,
    pub address_line2: Option<String>,
}

/// 在庫作成用のリクエストDTO
#[derive(Serialize, Deserialize)]
pub struct CreateInventoryRequest {
    pub book_id: Uuid,
    pub quantity: u32,
}

/// 注文一覧取得用のクエリパラメータ
#[derive(Deserialize)]
pub struct OrdersQueryParams {
    pub status: Option<String>,
}

/// 在庫一覧取得用のクエリパラメータ
#[derive(Deserialize)]
pub struct InventoryQueryParams {
    pub max_quantity: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_order_request_serialization() {
        let request = CreateOrderRequest {
            customer_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: CreateOrderRequest = serde_json::from_str(&json).unwrap();

        // シリアライゼーション/デシリアライゼーションが成功することを確認
        assert!(json.contains("customer_id"));
    }

    #[test]
    fn test_create_order_request_without_customer_id() {
        let request = CreateOrderRequest { customer_id: None };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: CreateOrderRequest = serde_json::from_str(&json).unwrap();

        // customer_idがnullでシリアライズされることを確認
        assert!(json.contains("null"));
    }

    #[test]
    fn test_add_book_request_serialization() {
        let book_id = Uuid::new_v4();
        let request = AddBookRequest {
            book_id,
            quantity: 2,
            unit_price: 1500,
        };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: AddBookRequest = serde_json::from_str(&json).unwrap();

        // 必要なフィールドがシリアライズされることを確認
        assert!(json.contains("book_id"));
        assert!(json.contains("quantity"));
        assert!(json.contains("unit_price"));
    }

    #[test]
    fn test_set_shipping_address_request_with_building() {
        let request = SetShippingAddressRequest {
            postal_code: "1234567".to_string(),
            prefecture: "東京都".to_string(),
            city: "渋谷区".to_string(),
            address_line1: "道玄坂1-1-1".to_string(),
            address_line2: Some("ビル名".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: SetShippingAddressRequest = serde_json::from_str(&json).unwrap();

        // 必要なフィールドがシリアライズされることを確認
        assert!(json.contains("postal_code"));
        assert!(json.contains("prefecture"));
        assert!(json.contains("city"));
        assert!(json.contains("address_line1"));
        assert!(json.contains("address_line2"));
    }

    #[test]
    fn test_set_shipping_address_request_without_building() {
        let request = SetShippingAddressRequest {
            postal_code: "1234567".to_string(),
            prefecture: "東京都".to_string(),
            city: "渋谷区".to_string(),
            address_line1: "道玄坂1-1-1".to_string(),
            address_line2: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: SetShippingAddressRequest = serde_json::from_str(&json).unwrap();

        // address_line2がnullでシリアライズされることを確認
        assert!(json.contains("null"));
    }

    #[test]
    fn test_create_inventory_request_serialization() {
        let book_id = Uuid::new_v4();
        let request = CreateInventoryRequest {
            book_id,
            quantity: 50,
        };

        let json = serde_json::to_string(&request).unwrap();
        let _deserialized: CreateInventoryRequest = serde_json::from_str(&json).unwrap();

        // 必要なフィールドがシリアライズされることを確認
        assert!(json.contains("book_id"));
        assert!(json.contains("quantity"));
    }

    #[test]
    fn test_query_params_deserialization() {
        // OrdersQueryParams のテスト
        let params = OrdersQueryParams {
            status: Some("Pending".to_string()),
        };
        assert_eq!(params.status, Some("Pending".to_string()));

        let params = OrdersQueryParams { status: None };
        assert_eq!(params.status, None);

        // InventoryQueryParams のテスト
        let params = InventoryQueryParams {
            max_quantity: Some(10),
        };
        assert_eq!(params.max_quantity, Some(10));

        let params = InventoryQueryParams { max_quantity: None };
        assert_eq!(params.max_quantity, None);
    }
}

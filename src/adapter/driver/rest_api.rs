use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use uuid::Uuid;

use crate::application::service::{OrderApplicationService, OrderQueryService, InventoryQueryService};
use crate::application::ApplicationError;
use crate::domain::model::{BookId, CustomerId, Money, OrderId, ShippingAddress, OrderStatus};
use crate::adapter::driven::{MySqlOrderRepository, MySqlInventoryRepository, ConsoleEventPublisher};
use crate::adapter::driver::response_dto::{OrderSummaryResponse, OrderDetailResponse, InventoryResponse};
use crate::adapter::driver::request_dto::{
    CreateOrderRequest, AddBookRequest, SetShippingAddressRequest, CreateInventoryRequest,
    OrdersQueryParams, InventoryQueryParams
};
use crate::domain::model::Inventory;
use crate::domain::port::InventoryRepository;

// REST API用のレスポンスDTO
#[derive(Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: Uuid,
    pub customer_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

// アプリケーションサービスを含む状態
pub type AppState = AppStateInner;

#[derive(Clone)]
pub struct AppStateInner {
    pub order_service: Arc<OrderApplicationService<MySqlOrderRepository, MySqlInventoryRepository, ConsoleEventPublisher>>,
    pub inventory_repository: Arc<MySqlInventoryRepository>,
    pub order_query_service: Arc<OrderQueryService>,
    pub inventory_query_service: Arc<InventoryQueryService>,
}

// REST APIルーターを作成
pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/orders", post(create_order))
        .route("/orders/:order_id/books", post(add_book_to_order))
        .route("/orders/:order_id/shipping-address", put(set_shipping_address))
        .route("/orders/:order_id/confirm", post(confirm_order))
        .route("/orders/:order_id/cancel", post(cancel_order))
        .route("/orders/:order_id/ship", post(mark_order_as_shipped))
        .route("/orders/:order_id/deliver", post(mark_order_as_delivered))
        .route("/inventory", post(create_inventory))
        // 新しいGETエンドポイント
        .route("/orders", get(get_orders))
        .route("/orders/:order_id", get(get_order_by_id))
        .route("/inventory", get(get_inventories))
        .route("/inventory/:book_id", get(get_inventory_by_book_id))
}

// ヘルスチェックエンドポイント
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "bookstore-order-management",
        "version": "0.1.0"
    }))
}

// 注文作成エンドポイント
async fn create_order(
    State(state): State<AppState>,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, (StatusCode, Json<ApiError>)> {
    let customer_id = request.customer_id
        .map(CustomerId::from_uuid)
        .unwrap_or_else(CustomerId::new);

    match state.order_service.create_order(customer_id).await {
        Ok(order_id) => Ok(Json(CreateOrderResponse {
            order_id: order_id.as_uuid(),
            customer_id: customer_id.as_uuid(),
        })),
        Err(err) => Err(map_application_error(err)),
    }
}

// 本を注文に追加するエンドポイント
async fn add_book_to_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(request): Json<AddBookRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);
    let book_id = BookId::from_uuid(request.book_id);
    let unit_price = Money::jpy(request.unit_price);

    match state.order_service.add_book_to_order(
        order_id,
        book_id,
        request.quantity,
        unit_price,
    ).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 配送先住所設定エンドポイント
async fn set_shipping_address(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(request): Json<SetShippingAddressRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);
    
    let shipping_address = match ShippingAddress::new(
        request.postal_code,
        request.prefecture,
        request.city,
        request.address_line1,
        request.address_line2,
    ) {
        Ok(addr) => addr,
        Err(err) => return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Invalid shipping address: {}", err),
                code: "INVALID_ADDRESS".to_string(),
            }),
        )),
    };

    match state.order_service.set_shipping_address(order_id, shipping_address).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 注文確定エンドポイント
async fn confirm_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);

    match state.order_service.confirm_order(order_id).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 注文キャンセルエンドポイント
async fn cancel_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);

    match state.order_service.cancel_order(order_id).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 注文発送エンドポイント
async fn mark_order_as_shipped(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);

    match state.order_service.mark_order_as_shipped(order_id).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 注文配達完了エンドポイント
async fn mark_order_as_delivered(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let order_id = OrderId::from_uuid(order_id);

    match state.order_service.mark_order_as_delivered(order_id).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => Err(map_application_error(err)),
    }
}

// 在庫作成エンドポイント（テスト用）
async fn create_inventory(
    State(state): State<AppState>,
    Json(request): Json<CreateInventoryRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let book_id = BookId::from_uuid(request.book_id);
    let inventory = Inventory::new(book_id, request.quantity);
    
    // 在庫リポジトリに直接保存（本来はアプリケーションサービス経由が望ましい）
    match state.inventory_repository.save(&inventory).await {
        Ok(()) => Ok(StatusCode::CREATED),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("{}", err),
                code: "REPOSITORY_ERROR".to_string(),
            }),
        )),
    }
}

// 注文一覧取得エンドポイント
async fn get_orders(
    State(state): State<AppState>,
    query: Result<Query<OrdersQueryParams>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<OrderSummaryResponse>>, (StatusCode, Json<ApiError>)> {
    let Query(params) = query.map_err(|_| (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: "無効なクエリパラメータです".to_string(),
            code: "INVALID_PARAMETER".to_string(),
        }),
    ))?;
    let orders = if let Some(status_str) = params.status {
        // ステータスでフィルタリング
        let status = match OrderStatus::from_string(&status_str) {
            Ok(status) => status,
            Err(_) => return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("無効なステータス値: {}", status_str),
                    code: "INVALID_STATUS".to_string(),
                }),
            )),
        };
        
        match state.order_query_service.get_orders_by_status(status).await {
            Ok(orders) => orders,
            Err(err) => return Err(map_application_error(err)),
        }
    } else {
        // 全注文を取得
        match state.order_query_service.get_all_orders().await {
            Ok(orders) => orders,
            Err(err) => return Err(map_application_error(err)),
        }
    };

    let response: Vec<OrderSummaryResponse> = orders
        .iter()
        .map(OrderSummaryResponse::from_order)
        .collect();

    Ok(Json(response))
}

// 注文詳細取得エンドポイント
async fn get_order_by_id(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderDetailResponse>, (StatusCode, Json<ApiError>)> {
    let order_id = match OrderId::from_string(&order_id.to_string()) {
        Ok(id) => id,
        Err(_) => return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "無効な注文ID形式です".to_string(),
                code: "INVALID_UUID".to_string(),
            }),
        )),
    };

    match state.order_query_service.get_order_by_id(order_id).await {
        Ok(Some(order)) => {
            let response = OrderDetailResponse::from_order(&order);
            Ok(Json(response))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "指定された注文が見つかりません".to_string(),
                code: "ORDER_NOT_FOUND".to_string(),
            }),
        )),
        Err(err) => Err(map_application_error(err)),
    }
}

// 在庫一覧取得エンドポイント
async fn get_inventories(
    State(state): State<AppState>,
    query: Result<Query<InventoryQueryParams>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<InventoryResponse>>, (StatusCode, Json<ApiError>)> {
    let Query(params) = query.map_err(|_| (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: "無効なクエリパラメータです".to_string(),
            code: "INVALID_PARAMETER".to_string(),
        }),
    ))?;
    let inventories = if let Some(max_quantity) = params.max_quantity {
        // パラメータ値の妥当性チェック（負の値は既にu32で防がれているが、追加の検証を行う場合）
        // 最大在庫数でフィルタリング
        match state.inventory_query_service.get_low_stock_inventories(max_quantity).await {
            Ok(inventories) => inventories,
            Err(err) => return Err(map_application_error(err)),
        }
    } else {
        // 全在庫を取得
        match state.inventory_query_service.get_all_inventories().await {
            Ok(inventories) => inventories,
            Err(err) => return Err(map_application_error(err)),
        }
    };

    let response: Vec<InventoryResponse> = inventories
        .iter()
        .map(InventoryResponse::from_inventory)
        .collect();

    Ok(Json(response))
}

// 在庫詳細取得エンドポイント
async fn get_inventory_by_book_id(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<InventoryResponse>, (StatusCode, Json<ApiError>)> {
    let book_id = match BookId::from_string(&book_id.to_string()) {
        Ok(id) => id,
        Err(_) => return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "無効な書籍ID形式です".to_string(),
                code: "INVALID_UUID".to_string(),
            }),
        )),
    };

    match state.inventory_query_service.get_inventory_by_book_id(book_id).await {
        Ok(Some(inventory)) => {
            let response = InventoryResponse::from_inventory(&inventory);
            Ok(Json(response))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "指定された書籍の在庫が見つかりません".to_string(),
                code: "INVENTORY_NOT_FOUND".to_string(),
            }),
        )),
        Err(err) => Err(map_application_error(err)),
    }
}

// アプリケーションエラーをHTTPエラーにマッピング
fn map_application_error(err: ApplicationError) -> (StatusCode, Json<ApiError>) {
    match err {
        ApplicationError::DomainError(domain_err) => map_domain_error(domain_err),
        ApplicationError::RepositoryError(repo_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("{}", repo_err),
                code: "REPOSITORY_ERROR".to_string(),
            }),
        ),
        ApplicationError::PublisherError(pub_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("{}", pub_err),
                code: "PUBLISHER_ERROR".to_string(),
            }),
        ),
        ApplicationError::NotFound(msg) => (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: msg,
                code: "NOT_FOUND".to_string(),
            }),
        ),
    }
}

// ドメインエラーを適切なHTTPステータスコードとエラーコードにマッピング
fn map_domain_error(domain_err: crate::domain::error::DomainError) -> (StatusCode, Json<ApiError>) {
    use crate::domain::error::DomainError;
    
    match domain_err {
        DomainError::InvalidAddress(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: msg,
                code: "INVALID_ADDRESS".to_string(),
            }),
        ),
        DomainError::InvalidQuantity => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "無効な数量です".to_string(),
                code: "INVALID_QUANTITY".to_string(),
            }),
        ),
        DomainError::InvalidValue(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: msg,
                code: "INVALID_VALUE".to_string(),
            }),
        ),
        DomainError::InvalidOrderState(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: msg,
                code: "INVALID_ORDER_STATE".to_string(),
            }),
        ),
        DomainError::InsufficientInventory => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "在庫不足です".to_string(),
                code: "INSUFFICIENT_INVENTORY".to_string(),
            }),
        ),
        DomainError::OrderValidation(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: msg,
                code: "ORDER_VALIDATION".to_string(),
            }),
        ),
        DomainError::CurrencyMismatch => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "通貨が一致しません".to_string(),
                code: "CURRENCY_MISMATCH".to_string(),
            }),
        ),
        DomainError::RepositoryError(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: msg,
                code: "REPOSITORY_ERROR".to_string(),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_status_from_string_valid() {
        assert!(OrderStatus::from_string("Pending").is_ok());
        assert!(OrderStatus::from_string("Confirmed").is_ok());
        assert!(OrderStatus::from_string("Shipped").is_ok());
        assert!(OrderStatus::from_string("Delivered").is_ok());
        assert!(OrderStatus::from_string("Cancelled").is_ok());
    }

    #[test]
    fn test_order_status_from_string_invalid() {
        assert!(OrderStatus::from_string("Invalid").is_err());
        assert!(OrderStatus::from_string("pending").is_err()); // 大文字小文字が違う
        assert!(OrderStatus::from_string("").is_err());
    }
}
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use crate::application::ApplicationError;

    #[test]
    fn test_map_application_error_not_found() {
        let app_error = ApplicationError::NotFound("リソースが見つかりません".to_string());
        let (status, Json(api_error)) = map_application_error(app_error);
        
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(api_error.code, "NOT_FOUND");
        assert_eq!(api_error.error, "リソースが見つかりません");
    }

    #[test]
    fn test_api_error_structure() {
        let api_error = ApiError {
            error: "テストエラー".to_string(),
            code: "TEST_ERROR".to_string(),
        };
        
        // JSON シリアライゼーションのテスト
        let json = serde_json::to_string(&api_error).unwrap();
        assert!(json.contains("テストエラー"));
        assert!(json.contains("TEST_ERROR"));
        
        // JSON デシリアライゼーションのテスト
        let deserialized: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.error, "テストエラー");
        assert_eq!(deserialized.code, "TEST_ERROR");
    }
}
mod adapter;
mod application;
mod domain;

use adapter::driven::{ConsoleLogger, EventBusConfig, InMemoryEventBus, MySqlInventoryRepository, MySqlOrderRepository};
use adapter::driver::rest_api::{create_router, AppStateInner};
use adapter::{DatabaseConfig, DatabaseMigration};
use application::service::{InventoryApplicationService, OrderApplicationService};
use domain::port::Logger;

use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ロガーを作成
    let logger: Arc<dyn Logger> = Arc::new(ConsoleLogger::new());

    logger.debug("Main", "=== 書店注文管理システム REST API ===", None, None);
    logger.debug("Main", "ドメイン駆動設計サンプルプロジェクト", None, None);

    // .envファイルから環境変数を読み込む
    dotenvy::dotenv().ok();

    // データベース設定を読み込む
    let config = DatabaseConfig::from_env()?;
    logger.debug(
        "Main",
        &format!("データベース設定を読み込みました: {}:{}", config.host, config.port),
        None,
        None,
    );

    // 接続プールを作成
    let pool = MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.connection_string())
        .await?;
    logger.debug("Main", "データベース接続プールを作成しました", None, None);

    // マイグレーションを実行
    let migration = DatabaseMigration::new(pool.clone(), logger.clone());
    migration.run().await?;
    logger.debug("Main", "データベースマイグレーションを実行しました", None, None);

    // MySQLリポジトリを作成
    let order_repository = Arc::new(MySqlOrderRepository::new(pool.clone()));
    let inventory_repository = Arc::new(MySqlInventoryRepository::new(pool.clone()));

    // イベントバスを作成
    let event_bus = Arc::new(InMemoryEventBus::new(EventBusConfig::default()));

    // イベントハンドラーを作成して登録
    let inventory_handler = crate::domain::handler::InventoryReservationHandler::new(
        inventory_repository.clone(),
        order_repository.clone(),
        event_bus.clone(),
        logger.clone(),
    );
    let _shipping_handler = crate::domain::handler::ShippingHandler::new(
        order_repository.clone(),
        event_bus.clone(),
        logger.clone(),
    );
    let _delivery_handler = crate::domain::handler::DeliveryHandler::new(
        order_repository.clone(),
        event_bus.clone(),
        logger.clone(),
    );
    let notification_handler = crate::domain::handler::NotificationHandler::new(logger.clone());
    let consistency_verifier = crate::domain::handler::EventualConsistencyVerifier::new(
        order_repository.clone(),
        inventory_repository.clone(),
        logger.clone(),
    );

    // 補償ハンドラーを作成
    let inventory_compensation_handler =
        crate::domain::handler::InventoryReservationFailureCompensationHandler::new(
            order_repository.clone(),
            event_bus.clone(),
            logger.clone(),
        );
    let shipping_compensation_handler =
        crate::domain::handler::ShippingFailureCompensationHandler::new(
            inventory_repository.clone(),
            order_repository.clone(),
            event_bus.clone(),
            logger.clone(),
        );
    let delivery_compensation_handler =
        crate::domain::handler::DeliveryFailureCompensationHandler::new(
            order_repository.clone(),
            event_bus.clone(),
            logger.clone(),
        );
    let saga_coordinator =
        crate::domain::handler::SagaCompensationCoordinator::new(event_bus.clone(), logger.clone());
    let compensation_completion_handler =
        crate::domain::handler::CompensationCompletionHandler::new(logger.clone());

    // イベントハンドラーをイベントバスに登録
    // 注文確定時は在庫予約のみ自動実行（発送・配達は手動操作）
    event_bus
        .subscribe_order_confirmed(inventory_handler)
        .await?;

    // 通知ハンドラーを各イベントに登録（並行処理で通知送信）
    event_bus
        .subscribe_order_confirmed(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_shipped(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_delivered(notification_handler.clone())
        .await?;
    event_bus
        .subscribe_order_cancelled(notification_handler)
        .await?;

    // 整合性検証ハンドラーを登録（並行処理で検証実行）
    event_bus
        .subscribe_order_confirmed(consistency_verifier.clone())
        .await?;
    event_bus
        .subscribe_order_delivered(consistency_verifier)
        .await?;

    // 補償ハンドラーを登録
    event_bus
        .subscribe_inventory_reservation_failed(inventory_compensation_handler)
        .await?;
    event_bus
        .subscribe_shipping_failed(shipping_compensation_handler)
        .await?;
    event_bus
        .subscribe_delivery_failed(delivery_compensation_handler)
        .await?;
    event_bus
        .subscribe_saga_compensation_started(saga_coordinator)
        .await?;
    event_bus
        .subscribe_saga_compensation_completed(compensation_completion_handler)
        .await?;

    logger.debug("Main", "イベントハンドラーを登録しました", None, None);
    logger.debug("Main", "注文フロー:", None, None);
    logger.debug("Main", "  1. 注文確定 → 在庫予約（自動）+ 通知送信", None, None);
    logger.debug("Main", "  2. 注文発送 → 手動操作（POST /orders/:id/ship）", None, None);
    logger.debug("Main", "  3. 配達完了 → 手動操作（POST /orders/:id/deliver）", None, None);

    // アプリケーションサービスを作成（Arcを外して渡す）
    let order_service =
        OrderApplicationService::new(MySqlOrderRepository::new(pool.clone()), event_bus.clone());

    // 在庫サービスを作成
    let inventory_service = InventoryApplicationService::new(inventory_repository.clone());

    // アプリケーション状態を作成
    let app_state = AppStateInner {
        order_service: Arc::new(order_service),
        inventory_service: Arc::new(inventory_service),
    };

    // REST APIルーターを作成
    let app = create_router()
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // サーバーを起動
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    logger.debug("Main", "REST APIサーバーが起動しました: http://localhost:3000", None, None);
    logger.debug("Main", "ヘルスチェック: GET http://localhost:3000/health", None, None);
    logger.debug("Main", "API仕様:", None, None);
    logger.debug("Main", "  POST /orders - 注文作成", None, None);
    logger.debug("Main", "  GET  /orders - 注文一覧取得", None, None);
    logger.debug("Main", "  GET  /orders/:id - 注文詳細取得", None, None);
    logger.debug("Main", "  POST /orders/:id/books - 本を注文に追加", None, None);
    logger.debug("Main", "  PUT  /orders/:id/shipping-address - 配送先住所設定", None, None);
    logger.debug("Main", "  POST /orders/:id/confirm - 注文確定", None, None);
    logger.debug("Main", "  POST /orders/:id/cancel - 注文キャンセル", None, None);
    logger.debug("Main", "  POST /orders/:id/ship - 注文発送", None, None);
    logger.debug("Main", "  POST /orders/:id/deliver - 注文配達完了", None, None);
    logger.debug("Main", "  POST /inventory - 在庫作成（テスト用）", None, None);
    logger.debug("Main", "  GET  /inventory - 在庫一覧取得", None, None);
    logger.debug("Main", "  GET  /inventory/:book_id - 在庫詳細取得", None, None);

    axum::serve(listener, app).await?;

    Ok(())
}

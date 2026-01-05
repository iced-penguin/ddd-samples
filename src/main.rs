mod domain;
mod application;
mod adapter;

use adapter::driven::{
    MySqlOrderRepository,
    MySqlInventoryRepository,
    ConsoleEventPublisher,
};
use adapter::driver::rest_api::{create_router, AppStateInner};
use adapter::{DatabaseConfig, DatabaseMigration};
use application::service::{OrderApplicationService, OrderQueryService, InventoryQueryService};

use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 書店注文管理システム REST API ===");
    println!("ドメイン駆動設計サンプルプロジェクト");
    println!();

    // .envファイルから環境変数を読み込む
    dotenvy::dotenv().ok();
    
    // データベース設定を読み込む
    let config = DatabaseConfig::from_env()?;
    println!("データベース設定を読み込みました: {}:{}", config.host, config.port);
    
    // 接続プールを作成
    let pool = MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.connection_string())
        .await?;
    println!("データベース接続プールを作成しました");
    
    // マイグレーションを実行
    let migration = DatabaseMigration::new(pool.clone());
    migration.run().await?;
    println!("データベースマイグレーションを実行しました");
    
    // MySQLリポジトリを作成
    let order_repository = Arc::new(MySqlOrderRepository::new(pool.clone()));
    let inventory_repository = Arc::new(MySqlInventoryRepository::new(pool.clone()));
    let event_publisher = ConsoleEventPublisher::new();
    
    // アプリケーションサービスを作成（Arcを外して渡す）
    let order_service = OrderApplicationService::new(
        MySqlOrderRepository::new(pool.clone()),
        MySqlInventoryRepository::new(pool.clone()),
        event_publisher,
    );
    
    // クエリサービスを作成
    let order_query_service = OrderQueryService::new(order_repository.clone());
    let inventory_query_service = InventoryQueryService::new(inventory_repository.clone());
    
    // アプリケーション状態を作成
    let app_state = AppStateInner {
        order_service: Arc::new(order_service),
        inventory_repository: inventory_repository.clone(),
        order_query_service: Arc::new(order_query_service),
        inventory_query_service: Arc::new(inventory_query_service),
    };
    
    // REST APIルーターを作成
    let app = create_router()
        .layer(CorsLayer::permissive())
        .with_state(app_state);
    
    // サーバーを起動
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("REST APIサーバーが起動しました: http://localhost:3000");
    println!("ヘルスチェック: GET http://localhost:3000/health");
    println!("API仕様:");
    println!("  POST /orders - 注文作成");
    println!("  GET  /orders - 注文一覧取得");
    println!("  GET  /orders/:id - 注文詳細取得");
    println!("  POST /orders/:id/books - 本を注文に追加");
    println!("  PUT  /orders/:id/shipping-address - 配送先住所設定");
    println!("  POST /orders/:id/confirm - 注文確定");
    println!("  POST /orders/:id/cancel - 注文キャンセル");
    println!("  POST /orders/:id/ship - 注文発送");
    println!("  POST /orders/:id/deliver - 注文配達完了");
    println!("  POST /inventory - 在庫作成（テスト用）");
    println!("  GET  /inventory - 在庫一覧取得");
    println!("  GET  /inventory/:book_id - 在庫詳細取得");
    println!();
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

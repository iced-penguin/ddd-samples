# 書店注文管理システム

ドメイン駆動設計（Domain-Driven Design, DDD）の主要概念を実践的に学ぶためのサンプルプロジェクトです。オンライン書店における注文管理システムを題材に、エンティティ、値オブジェクト、集約、ドメインイベント、リポジトリパターンなどのDDD戦術的パターンをRustで実装しています。

## 📚 プロジェクト概要

本システムは、顧客が書籍を選択し、注文を作成し、配送を受け取るまでのプロセスを管理します。ビジネスロジックをドメイン層に集約し、ポートとアダプターアーキテクチャ（ヘキサゴナルアーキテクチャ）を採用することで、技術的な詳細からドメインモデルを分離しています。

### 主な機能

- **注文管理**: 注文の作成、書籍の追加、配送先住所の設定
- **注文ライフサイクル**: 注文の確定、キャンセル、発送、配達完了
- **在庫管理**: 書籍の在庫登録、在庫の予約と解放
- **ドメインイベント**: 注文確定、キャンセル、発送、配達完了時のイベント発行
- **ビジネスルール**: 在庫チェック、配送料計算、ステータス遷移制御

## 🎯 学習目標

このプロジェクトを通じて、以下のDDDの主要概念を実践的に学ぶことができます：

- **エンティティと値オブジェクト**: 同一性と値の等価性の違い
- **集約**: トランザクション境界と不変条件の保護
- **ドメインイベント**: ビジネスイベントの表現と統合
- **リポジトリパターン**: 永続化の抽象化
- **ポートとアダプターアーキテクチャ**: 技術的詳細からの分離

詳細な学習ポイントについては、[DDDコンセプトガイド](docs/DDD_CONCEPTS.md)を参照してください。

## 🚀 クイックスタート

### 前提条件

- Rust 1.70以上
- Docker & Docker Compose

### セットアップと実行

```bash
# リポジトリをクローン
git clone <repository-url>
cd ddd-samples

# データベースを起動
docker compose up -d

# 環境変数ファイルを用意
cp .env.example .env

# アプリケーションを実行
cargo run
```

以下のエンドポイントで起動確認：

```bash
curl http://localhost:3000/health
# Expected response: {"status":"ok"}
```

詳細なセットアップ手順については、[セットアップガイド](docs/SETUP_GUIDE.md)を参照してください。

## 🌐 API使用方法

サーバーが起動すると、`http://localhost:3000` でREST APIが利用可能になります。

### 基本的な注文フロー

```bash
# 1. 在庫作成
curl -X POST http://localhost:3000/inventory \
  -H "Content-Type: application/json" \
  -d '{"book_id":"550e8400-e29b-41d4-a716-446655440000","quantity":10}'

# 2. 注文作成
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{}'

# 3. 書籍を注文に追加
curl -X POST http://localhost:3000/orders/{order_id}/books \
  -H "Content-Type: application/json" \
  -d '{"book_id":"550e8400-e29b-41d4-a716-446655440000","quantity":2,"unit_price":1500}'

# 4. 注文確定
curl -X POST http://localhost:3000/orders/{order_id}/confirm
```

詳細なAPIリファレンスについては、[APIリファレンス](docs/API_REFERENCE.md)と[注文フローガイド](docs/ORDER_FLOW_GUIDE.md)を参照してください。

## 🛠️ 開発

### ホットリロード開発

cargo watchを使用してファイル変更時の自動ビルド・実行が可能です：

```bash
# 開発サーバーをホットリロードで起動
cargo watch -x 'run'

# テストをホットリロードで実行
cargo watch -x 'test'

# テスト実行後にサーバー起動（推奨）
cargo watch -x 'test' -x 'run'
```

### その他の開発コマンド

```bash
# コードフォーマット
cargo fmt

# リンターチェック
cargo clippy -- -D warnings

# データベース起動
docker compose up -d

# データベース停止
docker compose down
```

## 🧪 テスト

```bash
# すべてのテストを実行
cargo test

# 単体テストのみ実行
cargo test --lib

# プロパティベーステストを実行
cargo test --test '*'
```

詳細なテスト戦略については、[テストガイド](docs/TESTING_GUIDE.md)を参照してください。

## データベース操作

### データベースのバックアップ

```bash
# データベースをダンプ
docker exec bookstore_mysql mysqldump -u bookstore_user -p bookstore_db > backup.sql

# バックアップからリストア
docker exec -i bookstore_mysql mysql -u bookstore_user -p bookstore_db < backup.sql
```

### データベースの直接操作

```bash
# MySQLクライアントに接続
docker exec -it bookstore_mysql mysql -u bookstore_user -p bookstore_db
```

## � 参ドキュメント

- [DDDコンセプトガイド](docs/DDD_CONCEPTS.md) - ドメイン駆動設計の学習ポイント
- [アーキテクチャガイド](docs/ARCHITECTURE.md) - システムアーキテクチャの詳細
- [セットアップガイド](docs/SETUP_GUIDE.md) - 詳細なセットアップ手順
- [APIリファレンス](docs/API_REFERENCE.md) - REST API の詳細仕様
- [テストガイド](docs/TESTING_GUIDE.md) - テスト戦略と実行方法
- [注文フローガイド](docs/ORDER_FLOW_GUIDE.md) - 注文処理の詳細フロー

## 📖 参考資料

### ドメイン駆動設計

- Vlad Khononov『ドメイン駆動設計を始めよう』（O'Reilly）

## 📝 ライセンス

このプロジェクトは学習目的のサンプルプロジェクトです。

## 🤝 貢献

このプロジェクトは学習用のサンプルですが、改善提案やバグ報告は歓迎します。

---

**注意**: このプロジェクトは、ドメイン駆動設計の学習を目的としたサンプル実装です。本番環境での使用は想定していません。

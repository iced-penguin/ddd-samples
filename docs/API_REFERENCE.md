# API リファレンス

## 概要

書店注文管理システムは、REST APIを通じて注文管理機能を提供します。すべてのエンドポイントはJSON形式でデータを送受信します。

**ベースURL**: `http://localhost:3000`

## 認証

現在のバージョンでは認証は実装されていません。すべてのエンドポイントは認証なしでアクセス可能です。

## エラーレスポンス

すべてのエラーレスポンスは以下の形式で返されます：

```json
{
  "error": "エラーメッセージ",
  "code": "ERROR_CODE"
}
```

### HTTPステータスコード

- `200 OK`: 成功
- `201 Created`: リソースの作成成功
- `400 Bad Request`: リクエストが不正
- `404 Not Found`: リソースが見つからない
- `409 Conflict`: リソースの競合（在庫不足など）
- `500 Internal Server Error`: サーバー内部エラー

## エンドポイント一覧

### データ取得エンドポイント

**注文関連**:
- `GET /orders` - 注文一覧の取得（ステータスフィルタリング対応）
- `GET /orders/:id` - 注文詳細の取得

**在庫関連**:
- `GET /inventory` - 在庫一覧の取得（在庫数フィルタリング対応）
- `GET /inventory/:book_id` - 特定書籍の在庫取得

### ヘルスチェック

#### GET /health

システムの稼働状況を確認します。

**レスポンス**:
```json
{
  "status": "ok"
}
```

**例**:
```bash
curl http://localhost:3000/health
```

---

### 注文管理

#### GET /orders

すべての注文の一覧を取得します。ステータスでフィルタリングすることも可能です。

**クエリパラメータ**:
- `status` (オプション): 注文ステータス（`Pending`, `Confirmed`, `Shipped`, `Delivered`, `Cancelled`）

**レスポンス**:
```json
[
  {
    "order_id": "550e8400-e29b-41d4-a716-446655440000",
    "customer_id": "customer-123",
    "status": "Confirmed",
    "total_amount": 3500,
    "total_currency": "JPY",
    "created_at": "2024-01-15T10:30:00Z"
  },
  {
    "order_id": "550e8400-e29b-41d4-a716-446655440001",
    "customer_id": "customer-456",
    "status": "Pending",
    "total_amount": 2000,
    "total_currency": "JPY",
    "created_at": "2024-01-15T09:15:00Z"
  }
]
```

**例**:
```bash
# すべての注文を取得
curl http://localhost:3000/orders

# 確定済みの注文のみを取得
curl "http://localhost:3000/orders?status=Confirmed"
```

**エラーケース**:
- `400 Bad Request`: 無効なステータス値を指定した場合
```json
{
  "error": "Invalid status parameter: InvalidStatus",
  "code": "INVALID_PARAMETER"
}
```

#### GET /orders/:id

特定の注文の詳細情報を取得します。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**レスポンス**:
```json
{
  "order_id": "550e8400-e29b-41d4-a716-446655440000",
  "customer_id": "customer-123",
  "status": "Confirmed",
  "order_lines": [
    {
      "book_id": "550e8400-e29b-41d4-a716-446655440001",
      "quantity": 2,
      "unit_price_amount": 1500,
      "unit_price_currency": "JPY",
      "subtotal_amount": 3000,
      "subtotal_currency": "JPY"
    }
  ],
  "shipping_address": {
    "postal_code": "1500001",
    "prefecture": "東京都",
    "city": "渋谷区",
    "street": "神宮前1-1-1",
    "building": "サンプルビル 3F"
  },
  "subtotal_amount": 3000,
  "subtotal_currency": "JPY",
  "shipping_fee_amount": 0,
  "shipping_fee_currency": "JPY",
  "total_amount": 3000,
  "total_currency": "JPY"
}
```

**例**:
```bash
curl http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000
```

**エラーケース**:
- `400 Bad Request`: 無効なUUID形式を指定した場合
```json
{
  "error": "Invalid UUID format",
  "code": "INVALID_UUID"
}
```

- `404 Not Found`: 存在しない注文IDを指定した場合
```json
{
  "error": "Order not found: 550e8400-e29b-41d4-a716-446655440000",
  "code": "ORDER_NOT_FOUND"
}
```

#### POST /orders

新しい注文を作成します。

**リクエストボディ**: なし（空のJSONオブジェクト `{}` を送信）

**レスポンス**:
```json
{
  "order_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**例**:
```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{}'
```

#### POST /orders/:id/books

注文に書籍を追加します。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**:
```json
{
  "book_id": "550e8400-e29b-41d4-a716-446655440001",
  "quantity": 2,
  "unit_price": 1500
}
```

**フィールド説明**:
- `book_id`: 書籍ID（UUID形式）
- `quantity`: 数量（正の整数）
- `unit_price`: 単価（円、正の整数）

**レスポンス**: `200 OK`（レスポンスボディなし）

**例**:
```bash
curl -X POST http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/books \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity": 2,
    "unit_price": 1500
  }'
```

#### PUT /orders/:id/shipping-address

注文の配送先住所を設定します。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**:
```json
{
  "postal_code": "1500001",
  "prefecture": "東京都",
  "city": "渋谷区",
  "address_line1": "神宮前1-1-1",
  "address_line2": "サンプルビル 3F"
}
```

**フィールド説明**:
- `postal_code`: 郵便番号（7桁、ハイフンなし）
- `prefecture`: 都道府県
- `city`: 市区町村
- `address_line1`: 住所1（必須）
- `address_line2`: 住所2（オプション、建物名・部屋番号など）

**レスポンス**: `200 OK`（レスポンスボディなし）

**例**:
```bash
curl -X PUT http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/shipping-address \
  -H "Content-Type: application/json" \
  -d '{
    "postal_code": "1500001",
    "prefecture": "東京都",
    "city": "渋谷区",
    "address_line1": "神宮前1-1-1",
    "address_line2": "サンプルビル 3F"
  }'
```

#### POST /orders/:id/confirm

注文を確定します。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**: なし

**レスポンス**: `200 OK`（レスポンスボディなし）

**前提条件**:
- 注文に少なくとも1つの書籍が追加されている
- 配送先住所が設定されている
- 注文ステータスが`Pending`である
- すべての書籍の在庫が十分にある

**例**:
```bash
curl -X POST http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/confirm
```

#### POST /orders/:id/cancel

注文をキャンセルします。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**: なし

**レスポンス**: `200 OK`（レスポンスボディなし）

**前提条件**:
- 注文ステータスが`Pending`または`Confirmed`である
- 注文が発送済み（`Shipped`）または配達完了（`Delivered`）でない

**例**:
```bash
curl -X POST http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/cancel
```

#### POST /orders/:id/ship

注文を発送済みにします。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**: なし

**レスポンス**: `200 OK`（レスポンスボディなし）

**前提条件**:
- 注文ステータスが`Confirmed`である

**例**:
```bash
curl -X POST http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/ship
```

#### POST /orders/:id/deliver

注文を配達完了にします。

**パスパラメータ**:
- `id`: 注文ID（UUID形式）

**リクエストボディ**: なし

**レスポンス**: `200 OK`（レスポンスボディなし）

**前提条件**:
- 注文ステータスが`Shipped`である

**例**:
```bash
curl -X POST http://localhost:3000/orders/550e8400-e29b-41d4-a716-446655440000/deliver
```

---

### 在庫管理

#### GET /inventory

すべての在庫情報の一覧を取得します。最大在庫数でフィルタリングすることも可能です。

**クエリパラメータ**:
- `max_quantity` (オプション): 最大在庫数（指定した数以下の在庫のみを返す）

**レスポンス**:
```json
[
  {
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity_on_hand": 10
  },
  {
    "book_id": "550e8400-e29b-41d4-a716-446655440002",
    "quantity_on_hand": 5
  },
  {
    "book_id": "550e8400-e29b-41d4-a716-446655440003",
    "quantity_on_hand": 0
  }
]
```

**例**:
```bash
# すべての在庫を取得
curl http://localhost:3000/inventory

# 在庫数が5以下の書籍のみを取得
curl "http://localhost:3000/inventory?max_quantity=5"
```

**エラーケース**:
- `400 Bad Request`: 無効な在庫数パラメータを指定した場合
```json
{
  "error": "Invalid max_quantity parameter: must be a non-negative integer",
  "code": "INVALID_PARAMETER"
}
```

#### GET /inventory/:book_id

特定の書籍の在庫情報を取得します。

**パスパラメータ**:
- `book_id`: 書籍ID（UUID形式）

**レスポンス**:
```json
{
  "book_id": "550e8400-e29b-41d4-a716-446655440001",
  "quantity_on_hand": 10
}
```

**例**:
```bash
curl http://localhost:3000/inventory/550e8400-e29b-41d4-a716-446655440001
```

**エラーケース**:
- `400 Bad Request`: 無効なUUID形式を指定した場合
```json
{
  "error": "Invalid UUID format",
  "code": "INVALID_UUID"
}
```

- `404 Not Found`: 存在しない書籍IDを指定した場合
```json
{
  "error": "Inventory not found for book: 550e8400-e29b-41d4-a716-446655440001",
  "code": "INVENTORY_NOT_FOUND"
}
```

#### POST /inventory

在庫を作成します（テスト用エンドポイント）。

**リクエストボディ**:
```json
{
  "book_id": "550e8400-e29b-41d4-a716-446655440001",
  "quantity": 10
}
```

**フィールド説明**:
- `book_id`: 書籍ID（UUID形式）
- `quantity`: 在庫数（非負の整数）

**レスポンス**: `201 Created`（レスポンスボディなし）

**例**:
```bash
curl -X POST http://localhost:3000/inventory \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity": 10
  }'
```

## ビジネスルール

### 注文ステータス遷移

```
Pending → Confirmed → Shipped → Delivered
   ↓           ↓
Cancelled   Cancelled
```

**許可される遷移**:
- `Pending` → `Confirmed`: 注文確定
- `Pending` → `Cancelled`: 注文キャンセル
- `Confirmed` → `Shipped`: 発送
- `Confirmed` → `Cancelled`: 注文キャンセル
- `Shipped` → `Delivered`: 配達完了

**禁止される遷移**:
- `Shipped` → `Cancelled`: 発送済みはキャンセル不可
- `Delivered` → `Cancelled`: 配達完了はキャンセル不可
- `Cancelled` → その他: キャンセル済みは状態変更不可

### 配送料計算

- **小計が10,000円以上**: 配送料0円
- **小計が10,000円未満**: 配送料500円

### 在庫管理

- **注文確定時**: 在庫を予約（利用可能在庫から減算）
- **注文キャンセル時**: 予約在庫を解放（利用可能在庫に加算）
- **在庫不足時**: 注文確定でエラーを返す

## 完全な注文フロー例

以下は、注文作成から配達完了までの完全なフロー例です：

```bash
# 1. 在庫作成
curl -X POST http://localhost:3000/inventory \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity": 10
  }'

# 2. 注文作成
ORDER_RESPONSE=$(curl -s -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{}')
ORDER_ID=$(echo $ORDER_RESPONSE | jq -r '.order_id')
echo "Created order: $ORDER_ID"

# 3. 書籍を注文に追加
curl -X POST http://localhost:3000/orders/$ORDER_ID/books \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity": 2,
    "unit_price": 1500
  }'

# 4. 配送先住所設定
curl -X PUT http://localhost:3000/orders/$ORDER_ID/shipping-address \
  -H "Content-Type: application/json" \
  -d '{
    "postal_code": "1500001",
    "prefecture": "東京都",
    "city": "渋谷区",
    "address_line1": "神宮前1-1-1",
    "address_line2": "サンプルビル 3F"
  }'

# 5. 注文確定
curl -X POST http://localhost:3000/orders/$ORDER_ID/confirm

# 6. 発送
curl -X POST http://localhost:3000/orders/$ORDER_ID/ship

# 7. 配達完了
curl -X POST http://localhost:3000/orders/$ORDER_ID/deliver

# 8. 注文詳細を確認
curl http://localhost:3000/orders/$ORDER_ID

# 9. すべての注文一覧を確認
curl http://localhost:3000/orders

# 10. 在庫状況を確認
curl http://localhost:3000/inventory/550e8400-e29b-41d4-a716-446655440001

# 11. 在庫一覧を確認
curl http://localhost:3000/inventory
```

## エラーケース例

### 在庫不足エラー

```bash
# 在庫を超える数量で注文確定を試行
curl -X POST http://localhost:3000/orders/$ORDER_ID/confirm
```

**レスポンス**:
```json
{
  "error": "Insufficient inventory for book: 550e8400-e29b-41d4-a716-446655440001",
  "code": "INSUFFICIENT_INVENTORY"
}
```

### 不正なステータス遷移エラー

```bash
# 既にキャンセルされた注文を確定しようとする
curl -X POST http://localhost:3000/orders/$ORDER_ID/confirm
```

**レスポンス**:
```json
{
  "error": "Invalid status transition from Cancelled to Confirmed",
  "code": "INVALID_STATUS_TRANSITION"
}
```

### 注文が見つからないエラー

```bash
# 存在しない注文IDを指定
curl -X POST http://localhost:3000/orders/00000000-0000-0000-0000-000000000000/confirm
```

**レスポンス**:
```json
{
  "error": "Order not found: 00000000-0000-0000-0000-000000000000",
  "code": "ORDER_NOT_FOUND"
}
```

### 無効なUUID形式エラー

```bash
# 無効なUUID形式で注文詳細を取得しようとする
curl http://localhost:3000/orders/invalid-uuid
```

**レスポンス**:
```json
{
  "error": "Invalid UUID format",
  "code": "INVALID_UUID"
}
```

### 無効なパラメータエラー

```bash
# 無効なステータスパラメータで注文一覧を取得しようとする
curl "http://localhost:3000/orders?status=InvalidStatus"
```

**レスポンス**:
```json
{
  "error": "Invalid status parameter: InvalidStatus",
  "code": "INVALID_PARAMETER"
}
```

### 在庫が見つからないエラー

```bash
# 存在しない書籍IDで在庫を取得しようとする
curl http://localhost:3000/inventory/00000000-0000-0000-0000-000000000000
```

**レスポンス**:
```json
{
  "error": "Inventory not found for book: 00000000-0000-0000-0000-000000000000",
  "code": "INVENTORY_NOT_FOUND"
}
```

## レート制限

現在のバージョンではレート制限は実装されていません。

## バージョニング

現在のAPIバージョンは v1 です。将来的にはURLパスにバージョン情報を含める予定です（例: `/api/v1/orders`）。

## 開発・テスト用ツール

### cURLを使用したテスト

上記の例で示したように、cURLを使用してAPIをテストできます。

### Postmanコレクション

`postman_collection.json`が用意されています。
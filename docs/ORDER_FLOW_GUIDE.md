# 注文フロー ガイド

このドキュメントでは、書店注文管理システムの注文処理の一連の流れについて説明します。

## 概要

書店注文管理システムは、以下の主要なステップで注文を処理します：

1. **注文作成** - 新しい注文を作成（Pending状態）
2. **商品追加** - 注文に書籍を追加
3. **配送先設定** - 配送先住所を設定
4. **注文確定** - 在庫確認と注文の確定（Confirmed状態）、在庫予約を自動実行
5. **発送処理** - 注文の発送（手動操作でShipped状態に変更）
6. **配達完了** - 配達の完了（手動操作でDelivered状態に変更）

**注意**: 発送処理と配達完了は手動操作です。注文確定後は在庫予約のみが自動実行され、発送・配達は管理者がAPIを呼び出して実行します。

## REST API を使用した注文フロー

### 前提条件

- システムが起動していること (`cargo run`)
- MySQLデータベースが起動していること (`docker-compose up -d`)
- 必要な書籍の在庫が存在すること

### ステップ 1: 在庫作成（テスト用）

まず、注文する書籍の在庫を作成します：

```bash
curl -X POST http://localhost:3000/inventory \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440000",
    "quantity": 10
  }'
```

**レスポンス**: `201 Created`

### ステップ 2: 注文作成

新しい注文を作成します：

```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{}'
```

**レスポンス例**:
```json
{
  "order_id": "b837da02-f37e-4ecd-aed8-e4cb87df9ce9",
  "customer_id": "a21b01c5-283c-484a-accd-d8563033bda2"
}
```

### ステップ 3: 書籍を注文に追加

作成した注文に書籍を追加します：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/books \
  -H "Content-Type: application/json" \
  -d '{
    "book_id": "550e8400-e29b-41d4-a716-446655440000",
    "quantity": 2,
    "unit_price": 1500
  }'
```

**レスポンス**: `200 OK`

### ステップ 4: 配送先住所設定

注文の配送先住所を設定します：

```bash
curl -X PUT http://localhost:3000/orders/{order_id}/shipping-address \
  -H "Content-Type: application/json" \
  -d '{
    "postal_code": "1500001",
    "prefecture": "東京都",
    "city": "渋谷区",
    "address_line1": "神宮前1-1-1",
    "address_line2": "サンプルビル 3F"
  }'
```

**レスポンス**: `200 OK`

### ステップ 5: 注文確定

注文を確定します。この時点で在庫の確認と予約が行われます：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/confirm
```

**レスポンス**: `200 OK`

**注意**: 
- 在庫が不足している場合は `400 Bad Request` が返されます
- 注文確定後、在庫予約が自動実行されます
- 発送・配達は手動操作で実行する必要があります

**エラー例**:
```json
{
  "error": "在庫不足です",
  "code": "INSUFFICIENT_INVENTORY"
}
```

### ステップ 6: 注文発送（手動操作）

確定した注文を発送状態にします：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/ship
```

**レスポンス**: `200 OK`

**注意**: この操作は手動で実行する必要があります。注文確定後に自動実行されません。

### ステップ 7: 配達完了（手動操作）

発送した注文を配達完了状態にします：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/deliver
```

**レスポンス**: `200 OK`

**注意**: この操作は手動で実行する必要があります。発送後に自動実行されません。

## 注文状態の遷移

注文は以下の状態を遷移します：

```
Pending → Confirmed → Shipped → Delivered
   ↓           ↓
Cancelled   Cancelled
```

### 状態の説明

- **保留中 (Pending)**: 注文が作成された初期状態
- **確定済み (Confirmed)**: 在庫が確保され、注文が確定された状態
- **発送済み (Shipped)**: 商品が発送された状態
- **配達完了 (Delivered)**: 商品が顧客に配達された最終状態
- **キャンセル済み (Cancelled)**: 注文がキャンセルされた状態

### 注文キャンセル

確定前の注文はキャンセルできます：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/cancel
```

## ドメインイベント

注文の状態変更時には、以下のドメインイベントが発行されます：

- `OrderConfirmed`: 注文が確定された時（在庫予約を自動実行）
- `OrderCancelled`: 注文がキャンセルされた時
- `OrderShipped`: 注文が発送された時（手動操作時）
- `OrderDelivered`: 注文が配達完了した時（手動操作時）

これらのイベントは各種ハンドラーによって処理され、ログ出力や通知送信が行われます。

**自動処理**: 注文確定時のみ在庫予約が自動実行されます。
**手動処理**: 発送・配達は管理者がAPIを呼び出して実行します。

## エラーハンドリング

### よくあるエラー

1. **在庫不足**
   ```json
   {
     "error": "在庫不足です",
     "code": "INSUFFICIENT_INVENTORY"
   }
   ```

2. **無効な注文状態**
   ```json
   {
     "error": "注文を確定できるのはPending状態のみです",
     "code": "INVALID_ORDER_STATE"
   }
   ```

3. **注文が見つからない**
   ```json
   {
     "error": "注文が見つかりません",
     "code": "NOT_FOUND"
   }
   ```

4. **無効な住所**
   ```json
   {
     "error": "郵便番号は7桁の数字である必要があります",
     "code": "INVALID_ADDRESS"
   }
   ```

## 状態確認用エンドポイント

### ヘルスチェック

システムの状態を確認するには：

```bash
curl http://localhost:3000/health
```

**レスポンス**:
```json
{
  "status": "healthy",
  "service": "bookstore-order-management",
  "version": "0.1.0"
}
```

### 注文状態の確認

#### 注文一覧の取得

すべての注文の一覧を取得します。ステータスでフィルタリングも可能です：

```bash
# すべての注文を取得
curl http://localhost:3000/orders

# 確定済みの注文のみを取得
curl "http://localhost:3000/orders?status=Confirmed"
```

**レスポンス例**:
```json
[
  {
    "order_id": "550e8400-e29b-41d4-a716-446655440000",
    "customer_id": "customer-123",
    "status": "Confirmed",
    "total_amount": 3500,
    "total_currency": "JPY",
    "created_at": "2024-01-15T10:30:00Z"
  }
]
```

#### 注文詳細の取得

特定の注文の詳細情報を取得します：

```bash
curl http://localhost:3000/orders/{order_id}
```

**レスポンス例**:
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

### 在庫状態の確認

#### 在庫一覧の取得

すべての在庫情報を取得します。最大在庫数でフィルタリングも可能です：

```bash
# すべての在庫を取得
curl http://localhost:3000/inventory

# 在庫数が5以下の書籍のみを取得
curl "http://localhost:3000/inventory?max_quantity=5"
```

**レスポンス例**:
```json
[
  {
    "book_id": "550e8400-e29b-41d4-a716-446655440001",
    "quantity_on_hand": 10
  },
  {
    "book_id": "550e8400-e29b-41d4-a716-446655440002",
    "quantity_on_hand": 5
  }
]
```

#### 特定書籍の在庫取得

特定の書籍の在庫情報を取得します：

```bash
curl http://localhost:3000/inventory/{book_id}
```

**レスポンス例**:
```json
{
  "book_id": "550e8400-e29b-41d4-a716-446655440001",
  "quantity_on_hand": 10
}
```
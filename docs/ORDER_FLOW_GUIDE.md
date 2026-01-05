# 注文フロー ガイド

このドキュメントでは、書店注文管理システムの注文処理の一連の流れについて説明します。

## 概要

書店注文管理システムは、以下の主要なステップで注文を処理します：

1. **注文作成** - 新しい注文を作成
2. **商品追加** - 注文に書籍を追加
3. **配送先設定** - 配送先住所を設定
4. **注文確定** - 在庫確認と注文の確定
5. **発送処理** - 注文の発送
6. **配達完了** - 配達の完了

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

**注意**: 在庫が不足している場合は `400 Bad Request` が返されます：
```json
{
  "error": "DomainError",
  "message": "Insufficient inventory"
}
```

### ステップ 6: 注文発送

確定した注文を発送状態にします：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/ship
```

**レスポンス**: `200 OK`

### ステップ 7: 配達完了

発送した注文を配達完了状態にします：

```bash
curl -X POST http://localhost:3000/orders/{order_id}/deliver
```

**レスポンス**: `200 OK`

## 注文状態の遷移

注文は以下の状態を遷移します：

```
作成済み → 確定済み → 発送済み → 配達完了
    ↓
  キャンセル済み
```

### 状態の説明

- **作成済み (Created)**: 注文が作成された初期状態
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

- `OrderConfirmed`: 注文が確定された時
- `OrderCancelled`: 注文がキャンセルされた時
- `OrderShipped`: 注文が発送された時
- `OrderDelivered`: 注文が配達完了した時

これらのイベントは `ConsoleEventPublisher` によってコンソールに出力されます。

## エラーハンドリング

### よくあるエラー

1. **在庫不足**
   ```json
   {
     "error": "DomainError",
     "message": "Insufficient inventory"
   }
   ```

2. **無効な注文状態**
   ```json
   {
     "error": "DomainError", 
     "message": "Invalid order state for this operation"
   }
   ```

3. **注文が見つからない**
   ```json
   {
     "error": "NotFound",
     "message": "Order not found"
   }
   ```

4. **無効な住所**
   ```json
   {
     "error": "InvalidAddress",
     "message": "Invalid shipping address: ..."
   }
   ```

## ヘルスチェック

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
# テストガイド

## 現在のテスト実装方針

このプロジェクトでは、ドメイン駆動設計の原則に基づいて、以下のテスト戦略を採用しています。

## 実装済みテストの種類

### 単体テスト（Unit Tests）

現在実装されているドメインオブジェクトの個別動作を検証します。

**実装済み対象**:
- 値オブジェクト（Money, OrderLine, ShippingAddress等）
- エンティティ（Order, Inventory）
- アダプター（ResponseDTO）
- 設定（DatabaseConfig）

**特徴**:
- 高速実行
- 外部依存なし

**実装箇所**
- 各モジュール内に `#[cfg(test)]` で実装

### 2. プロパティベーステスト（Property-Based Tests）

[proptest](https://github.com/proptest-rs/proptest)クレートを使用して、ランダムな入力での不変条件を検証します。

**実装済み対象**:
- 値オブジェクト（Money, OrderLine, ShippingAddress）
- エンティティ（Order, Inventory）
- 数学的性質（交換法則、結合法則、分配法則）
- ビジネスルール（小計計算、在庫制約、注文確定条件）

**特徴**:
- 256回のランダムテストケース実行（デフォルト）
- 不変条件の自動検証
- エッジケースの自動発見

**実装箇所**:
- `tests/property_tests.rs`

## 現在のテスト実行方法

### 基本的なテスト実行

```bash
# すべてのテストを実行
cargo test

# 詳細な出力付きでテスト実行
cargo test -- --nocapture

# 特定のテスト関数を実行
cargo test test_money_addition

# 特定のモジュールのテストを実行
cargo test domain::model::value_objects
```
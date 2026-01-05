use proptest::prelude::*;
use bookstore_order_management::domain::model::{
    Money, OrderLine, BookId, OrderId, CustomerId, Order, Inventory, ShippingAddress
};

// Money のプロパティベーステスト
proptest! {
    /// Money の加算は交換法則を満たす (a + b = b + a)
    #[test]
    fn test_money_addition_is_commutative(
        amount1 in 0i64..1_000_000,
        amount2 in 0i64..1_000_000,
    ) {
        let money1 = Money::jpy(amount1);
        let money2 = Money::jpy(amount2);
        
        let result1 = money1.add(&money2).unwrap();
        let result2 = money2.add(&money1).unwrap();
        
        prop_assert_eq!(result1, result2);
    }

    /// Money の加算は結合法則を満たす ((a + b) + c = a + (b + c))
    #[test]
    fn test_money_addition_is_associative(
        amount1 in 0i64..100_000,
        amount2 in 0i64..100_000,
        amount3 in 0i64..100_000,
    ) {
        let money1 = Money::jpy(amount1);
        let money2 = Money::jpy(amount2);
        let money3 = Money::jpy(amount3);
        
        let result1 = money1.add(&money2).unwrap().add(&money3).unwrap();
        let result2 = money1.add(&money2.add(&money3).unwrap()).unwrap();
        
        prop_assert_eq!(result1, result2);
    }

    /// Money の乗算は分配法則を満たす (a * (b + c) = a * b + a * c)
    #[test]
    fn test_money_multiplication_distributive(
        base_amount in 1i64..10_000,
        factor1 in 1u32..100,
        factor2 in 1u32..100,
    ) {
        let money = Money::jpy(base_amount);
        
        let left_side = money.multiply(factor1 + factor2);
        let right_side = money.multiply(factor1).add(&money.multiply(factor2)).unwrap();
        
        prop_assert_eq!(left_side, right_side);
    }

    /// Money の乗算で0を掛けると0になる
    #[test]
    fn test_money_multiply_by_zero(
        amount in 1i64..1_000_000,
    ) {
        let money = Money::jpy(amount);
        let result = money.multiply(0);
        
        prop_assert_eq!(result, Money::jpy(0));
    }

    /// Money の乗算で1を掛けると元の値と同じ
    #[test]
    fn test_money_multiply_by_one(
        amount in 0i64..1_000_000,
    ) {
        let money = Money::jpy(amount);
        let result = money.multiply(1);
        
        prop_assert_eq!(result, money);
    }
}

// OrderLine のプロパティベーステスト
proptest! {
    /// OrderLine の小計は常に単価 × 数量と等しい
    #[test]
    fn test_order_line_subtotal_calculation(
        quantity in 1u32..1000,
        unit_price in 1i64..100_000,
    ) {
        let book_id = BookId::new();
        let price = Money::jpy(unit_price);
        let line = OrderLine::new(book_id, quantity, price).unwrap();
        
        let expected_subtotal = price.multiply(quantity);
        prop_assert_eq!(line.subtotal(), expected_subtotal);
    }

    /// OrderLine の数量増加は常に正しく動作する
    #[test]
    fn test_order_line_quantity_increase(
        initial_quantity in 1u32..500,
        additional_quantity in 1u32..500,
        unit_price in 1i64..100_000,
    ) {
        let book_id = BookId::new();
        let price = Money::jpy(unit_price);
        let mut line = OrderLine::new(book_id, initial_quantity, price).unwrap();
        
        let result = line.increase_quantity(additional_quantity);
        prop_assert!(result.is_ok());
        prop_assert_eq!(line.quantity(), initial_quantity + additional_quantity);
    }

    /// OrderLine で0の数量増加は失敗する
    #[test]
    fn test_order_line_zero_quantity_increase_fails(
        initial_quantity in 1u32..1000,
        unit_price in 1i64..100_000,
    ) {
        let book_id = BookId::new();
        let price = Money::jpy(unit_price);
        let mut line = OrderLine::new(book_id, initial_quantity, price).unwrap();
        
        let result = line.increase_quantity(0);
        prop_assert!(result.is_err());
        prop_assert_eq!(line.quantity(), initial_quantity); // 数量は変わらない
    }
}

// Order のプロパティベーステスト
proptest! {
    /// Order の合計金額は常に非負である
    #[test]
    fn test_order_total_is_non_negative(
        book_data in prop::collection::vec((1u32..100, 1i64..10_000), 1..10),
    ) {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);
        
        // 複数の書籍を追加
        for (quantity, unit_price) in book_data {
            let book_id = BookId::new();
            let price = Money::jpy(unit_price);
            order.add_book(book_id, quantity, price).unwrap();
        }
        
        let total = order.calculate_total();
        prop_assert!(total.amount() >= 0);
    }

    /// Order の合計金額は小計 + 配送料と等しい
    #[test]
    fn test_order_total_calculation_correctness(
        book_data in prop::collection::vec((1u32..50, 1i64..5_000), 1..5),
    ) {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);
        
        let mut expected_subtotal = 0i64;
        
        // 複数の書籍を追加して期待される小計を計算
        for (quantity, unit_price) in book_data {
            let book_id = BookId::new();
            let price = Money::jpy(unit_price);
            order.add_book(book_id, quantity, price).unwrap();
            expected_subtotal += unit_price * (quantity as i64);
        }
        
        // 配送料の計算
        let expected_shipping_fee = if expected_subtotal >= 10_000 { 0 } else { 500 };
        let expected_total = expected_subtotal + expected_shipping_fee;
        
        let actual_total = order.calculate_total();
        prop_assert_eq!(actual_total.amount(), expected_total);
    }

    /// Order に同じ書籍を複数回追加すると数量が累積される
    #[test]
    fn test_order_same_book_quantity_accumulation(
        quantities in prop::collection::vec(1u32..100, 2..10),
        unit_price in 1i64..10_000,
    ) {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);
        
        let book_id = BookId::new();
        let price = Money::jpy(unit_price);
        let expected_total_quantity: u32 = quantities.iter().sum();
        
        // 同じ書籍を複数回追加
        for quantity in quantities {
            order.add_book(book_id, quantity, price).unwrap();
        }
        
        // 注文明細は1つだけで、数量が累積されている
        prop_assert_eq!(order.order_lines().len(), 1);
        prop_assert_eq!(order.order_lines()[0].quantity(), expected_total_quantity);
    }

    /// Order の確定には注文明細と配送先住所が必要
    #[test]
    fn test_order_confirmation_requirements(
        has_order_lines in any::<bool>(),
        has_shipping_address in any::<bool>(),
    ) {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();
        let mut order = Order::new(order_id, customer_id);
        
        // 条件に応じて注文明細を追加
        if has_order_lines {
            let book_id = BookId::new();
            let price = Money::jpy(1000);
            order.add_book(book_id, 1, price).unwrap();
        }
        
        // 条件に応じて配送先住所を設定
        if has_shipping_address {
            let address = ShippingAddress::new(
                "1234567".to_string(),
                "東京都".to_string(),
                "渋谷区".to_string(),
                "道玄坂1-1-1".to_string(),
                None,
            ).unwrap();
            order.set_shipping_address(address);
        }
        
        let result = order.confirm();
        
        // 両方の条件が満たされている場合のみ成功
        if has_order_lines && has_shipping_address {
            prop_assert!(result.is_ok());
        } else {
            prop_assert!(result.is_err());
        }
    }
}

// Inventory のプロパティベーステスト
proptest! {
    /// Inventory の予約と解放は可逆的である
    #[test]
    fn test_inventory_reserve_release_reversible(
        initial_quantity in 10u32..1000,
        reserve_quantity in 1u32..9,
    ) {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, initial_quantity);
        
        // 予約
        let reserve_result = inventory.reserve(reserve_quantity);
        prop_assert!(reserve_result.is_ok());
        prop_assert_eq!(inventory.quantity_on_hand(), initial_quantity - reserve_quantity);
        
        // 解放
        let release_result = inventory.release(reserve_quantity);
        prop_assert!(release_result.is_ok());
        prop_assert_eq!(inventory.quantity_on_hand(), initial_quantity);
    }

    /// Inventory の予約は在庫数を超えない場合のみ成功する
    #[test]
    fn test_inventory_reserve_within_limits(
        initial_quantity in 0u32..1000,
        reserve_quantity in 0u32..2000,
    ) {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, initial_quantity);
        let original_quantity = inventory.quantity_on_hand();
        
        let result = inventory.reserve(reserve_quantity);
        
        if reserve_quantity <= initial_quantity {
            prop_assert!(result.is_ok());
            prop_assert_eq!(inventory.quantity_on_hand(), initial_quantity - reserve_quantity);
        } else {
            prop_assert!(result.is_err());
            prop_assert_eq!(inventory.quantity_on_hand(), original_quantity); // 在庫数は変わらない
        }
    }

    /// Inventory の has_available_stock は正確である
    #[test]
    fn test_inventory_has_available_stock_accuracy(
        initial_quantity in 0u32..1000,
        check_quantity in 0u32..2000,
    ) {
        let book_id = BookId::new();
        let inventory = Inventory::new(book_id, initial_quantity);
        
        let has_stock = inventory.has_available_stock(check_quantity);
        let expected = check_quantity <= initial_quantity;
        
        prop_assert_eq!(has_stock, expected);
    }

    /// Inventory の解放は常に成功し、在庫数を増加させる
    #[test]
    fn test_inventory_release_always_succeeds(
        initial_quantity in 0u32..1000,
        release_quantity in 1u32..1000,
    ) {
        let book_id = BookId::new();
        let mut inventory = Inventory::new(book_id, initial_quantity);
        
        let result = inventory.release(release_quantity);
        prop_assert!(result.is_ok());
        prop_assert_eq!(inventory.quantity_on_hand(), initial_quantity + release_quantity);
    }
}

// ShippingAddress のプロパティベーステスト
proptest! {
    /// ShippingAddress の郵便番号バリデーション
    #[test]
    fn test_shipping_address_postal_code_validation(
        postal_code in "[0-9]{7}",
        prefecture in "[\\p{Hiragana}\\p{Katakana}\\p{Han}]{2,4}",
        city in "[\\p{Hiragana}\\p{Katakana}\\p{Han}]{2,10}",
        street in "[\\p{Hiragana}\\p{Katakana}\\p{Han}0-9\\-]{5,20}",
    ) {
        let result = ShippingAddress::new(
            postal_code.clone(),
            prefecture.clone(),
            city.clone(),
            street.clone(),
            None,
        );
        
        // 7桁の数字の郵便番号は常に有効
        prop_assert!(result.is_ok());
        
        let address = result.unwrap();
        prop_assert_eq!(address.postal_code(), postal_code);
        prop_assert_eq!(address.prefecture(), prefecture);
        prop_assert_eq!(address.city(), city);
        prop_assert_eq!(address.street(), street);
    }

    /// ShippingAddress の無効な郵便番号は拒否される
    #[test]
    fn test_shipping_address_invalid_postal_code_rejected(
        // 7桁でない、または数字でない郵便番号
        postal_code in "([0-9]{1,6}|[0-9]{8,}|[a-zA-Z]{7}|[0-9]{3}-[0-9]{4})",
        prefecture in "[\\p{Hiragana}\\p{Katakana}\\p{Han}]{2,4}",
        city in "[\\p{Hiragana}\\p{Katakana}\\p{Han}]{2,10}",
        street in "[\\p{Hiragana}\\p{Katakana}\\p{Han}0-9\\-]{5,20}",
    ) {
        let result = ShippingAddress::new(
            postal_code,
            prefecture,
            city,
            street,
            None,
        );
        
        // 無効な郵便番号は拒否される
        prop_assert!(result.is_err());
    }

    /// ShippingAddress の空の必須フィールドは拒否される
    #[test]
    fn test_shipping_address_empty_required_fields_rejected(
        postal_code in "[0-9]{7}",
        prefecture_empty in any::<bool>(),
        city_empty in any::<bool>(),
        street_empty in any::<bool>(),
    ) {
        let prefecture = if prefecture_empty { "".to_string() } else { "東京都".to_string() };
        let city = if city_empty { "".to_string() } else { "渋谷区".to_string() };
        let street = if street_empty { "".to_string() } else { "道玄坂1-1-1".to_string() };
        
        let result = ShippingAddress::new(
            postal_code,
            prefecture,
            city,
            street,
            None,
        );
        
        // いずれかのフィールドが空の場合は拒否される
        if prefecture_empty || city_empty || street_empty {
            prop_assert!(result.is_err());
        } else {
            prop_assert!(result.is_ok());
        }
    }
}
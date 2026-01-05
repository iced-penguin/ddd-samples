CREATE TABLE IF NOT EXISTS order_lines (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    order_id CHAR(36) NOT NULL,
    book_id CHAR(36) NOT NULL,
    quantity INT UNSIGNED NOT NULL,
    unit_price_amount BIGINT NOT NULL,
    unit_price_currency VARCHAR(3) NOT NULL DEFAULT 'JPY',
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE,
    UNIQUE KEY uk_order_book (order_id, book_id),
    INDEX idx_order_id (order_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

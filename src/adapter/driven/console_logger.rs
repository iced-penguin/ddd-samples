use crate::domain::port::{LogLevel, Logger};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// ログエントリ
/// 構造化ログの基本構造を定義
/// アダプター層の実装詳細として配置
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub correlation_id: Option<Uuid>,
    pub component: String,
    pub execution_time: Option<Duration>,
    pub additional_context: HashMap<String, String>,
}

impl LogEntry {
    /// 新しいログエントリを作成
    pub fn new(level: LogLevel, message: String, component: String) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            message,
            correlation_id: None,
            component,
            execution_time: None,
            additional_context: HashMap::new(),
        }
    }

    /// 相関IDを設定
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// 実行時間を設定
    pub fn with_execution_time(mut self, execution_time: Duration) -> Self {
        self.execution_time = Some(execution_time);
        self
    }

    /// 追加コンテキストを設定
    pub fn with_context(mut self, key: String, value: String) -> Self {
        self.additional_context.insert(key, value);
        self
    }

    /// ログエントリを文字列として出力
    pub fn format(&self) -> String {
        let level_str = match self.level {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        };

        let mut parts = vec![
            format!("[{}]", self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
            format!("[{}]", level_str),
            format!("[{}]", self.component),
        ];

        if let Some(correlation_id) = self.correlation_id {
            parts.push(format!("[correlation_id: {}]", correlation_id));
        }

        if let Some(execution_time) = self.execution_time {
            parts.push(format!("[execution_time: {:?}]", execution_time));
        }

        parts.push(self.message.clone());

        // 追加コンテキストがある場合は追加
        if !self.additional_context.is_empty() {
            let context_str = self
                .additional_context
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("[{}]", context_str));
        }

        parts.join(" ")
    }
}

/// コンソールログ実装
/// 標準出力・標準エラー出力にログを出力する
pub struct ConsoleLogger;

impl ConsoleLogger {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger for ConsoleLogger {
    fn debug(
        &self,
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        context: Option<HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Debug, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(ctx) = context {
            for (key, value) in ctx {
                entry = entry.with_context(key, value);
            }
        }

        println!("{}", entry.format());
    }

    fn info(
        &self,
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        context: Option<HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Info, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(ctx) = context {
            for (key, value) in ctx {
                entry = entry.with_context(key, value);
            }
        }

        println!("{}", entry.format());
    }

    fn warn(
        &self,
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        context: Option<HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Warning, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(ctx) = context {
            for (key, value) in ctx {
                entry = entry.with_context(key, value);
            }
        }

        println!("{}", entry.format());
    }

    fn error(
        &self,
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        context: Option<HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Error, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(ctx) = context {
            for (key, value) in ctx {
                entry = entry.with_context(key, value);
            }
        }

        eprintln!("{}", entry.format());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        );

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.component, "TestComponent");
        assert!(entry.correlation_id.is_none());
        assert!(entry.execution_time.is_none());
    }

    #[test]
    fn test_log_entry_with_correlation_id() {
        let correlation_id = Uuid::new_v4();
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        )
        .with_correlation_id(correlation_id);

        assert_eq!(entry.correlation_id, Some(correlation_id));
    }

    #[test]
    fn test_log_entry_format() {
        let correlation_id = Uuid::new_v4();
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context("key1".to_string(), "value1".to_string());

        let formatted = entry.format();

        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("[TestComponent]"));
        assert!(formatted.contains(&format!("[correlation_id: {}]", correlation_id)));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("key1=value1"));
    }

    #[test]
    fn test_console_logger_creation() {
        let logger = ConsoleLogger::new();
        // ログ出力のテストは実際の出力を確認するのが困難なため、
        // 作成できることのみをテスト
        logger.info("TestComponent", "Test message", None, None);
    }

    #[test]
    fn test_console_logger_with_context() {
        let logger = ConsoleLogger::new();
        let correlation_id = Uuid::new_v4();
        let mut context = HashMap::new();
        context.insert("key1".to_string(), "value1".to_string());
        
        logger.debug(
            "TestComponent",
            "Test debug message",
            Some(correlation_id),
            Some(context),
        );
    }
}
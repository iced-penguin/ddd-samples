use chrono::{DateTime, Utc};
use std::time::Duration;
use uuid::Uuid;

/// ログレベル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

/// ログエントリ
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub correlation_id: Option<Uuid>,
    pub component: String,
    pub execution_time: Option<Duration>,
    pub additional_context: std::collections::HashMap<String, String>,
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
            additional_context: std::collections::HashMap::new(),
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
        let mut parts = vec![
            format!("[{}]", self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
            format!("[{}]", self.level.as_str()),
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

/// イベントロガー
/// ドメインイベントとハンドラーの処理に特化したロガー
pub struct EventLogger;

impl EventLogger {
    /// イベント発行ログ
    pub fn log_event_published(event_type: &str, correlation_id: Uuid, event_id: Uuid) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("Event published: {}", event_type),
            "EventBus".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context("event_id".to_string(), event_id.to_string())
        .with_context("event_type".to_string(), event_type.to_string());

        println!("{}", entry.format());
    }

    /// ハンドラー処理開始ログ
    pub fn log_handler_started(handler_name: &str, event_type: &str, correlation_id: Uuid) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("Processing {} event", event_type),
            handler_name.to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context("event_type".to_string(), event_type.to_string());

        println!("{}", entry.format());
    }

    /// ハンドラー処理成功ログ
    pub fn log_handler_success(
        handler_name: &str,
        event_type: &str,
        correlation_id: Uuid,
        execution_time: Duration,
    ) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("{} event processed successfully", event_type),
            handler_name.to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_execution_time(execution_time)
        .with_context("event_type".to_string(), event_type.to_string());

        println!("{}", entry.format());
    }

    /// ハンドラー処理エラーログ
    pub fn log_handler_error(
        handler_name: &str,
        event_type: &str,
        correlation_id: Uuid,
        error_message: &str,
        execution_time: Option<Duration>,
    ) {
        let mut entry = LogEntry::new(
            LogLevel::Error,
            format!("{} event processing failed: {}", event_type, error_message),
            handler_name.to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context("event_type".to_string(), event_type.to_string())
        .with_context("error".to_string(), error_message.to_string());

        if let Some(exec_time) = execution_time {
            entry = entry.with_execution_time(exec_time);
        }

        eprintln!("{}", entry.format());
    }

    /// サーガ補償開始ログ
    pub fn log_saga_compensation_started(
        saga_id: Uuid,
        failed_step: &str,
        failure_reason: &str,
        compensation_steps: &[String],
    ) {
        let entry = LogEntry::new(
            LogLevel::Warning,
            format!("Saga compensation started for failed step: {}", failed_step),
            "SagaCompensation".to_string(),
        )
        .with_correlation_id(saga_id)
        .with_context("failed_step".to_string(), failed_step.to_string())
        .with_context("failure_reason".to_string(), failure_reason.to_string())
        .with_context(
            "compensation_steps".to_string(),
            compensation_steps.join(","),
        );

        println!("{}", entry.format());
    }

    /// サーガ補償完了ログ
    pub fn log_saga_compensation_completed(
        saga_id: Uuid,
        compensated_steps: &[String],
        result: &str,
        execution_time: Duration,
    ) {
        let level = if result == "Success" {
            LogLevel::Info
        } else {
            LogLevel::Warning
        };

        let entry = LogEntry::new(
            level,
            format!("Saga compensation completed with result: {}", result),
            "SagaCompensation".to_string(),
        )
        .with_correlation_id(saga_id)
        .with_execution_time(execution_time)
        .with_context("compensated_steps".to_string(), compensated_steps.join(","))
        .with_context("result".to_string(), result.to_string());

        println!("{}", entry.format());
    }

    /// 補償アクション実行ログ
    pub fn log_compensation_action(
        action_type: &str,
        correlation_id: Uuid,
        target_id: &str,
        execution_time: Duration,
    ) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("Compensation action executed: {}", action_type),
            "CompensationHandler".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_execution_time(execution_time)
        .with_context("action_type".to_string(), action_type.to_string())
        .with_context("target_id".to_string(), target_id.to_string());

        println!("{}", entry.format());
    }

    /// 通知送信ログ
    pub fn log_notification_sent(notification_type: &str, correlation_id: Uuid, recipient: &str) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("Notification sent: {}", notification_type),
            "NotificationHandler".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context(
            "notification_type".to_string(),
            notification_type.to_string(),
        )
        .with_context("recipient".to_string(), recipient.to_string());

        println!("{}", entry.format());
    }

    /// デッドレターキュー追加ログ
    pub fn log_dead_letter_queue_entry(
        event_type: &str,
        handler_name: &str,
        correlation_id: Uuid,
        error_message: &str,
        attempt_count: u32,
    ) {
        let entry = LogEntry::new(
            LogLevel::Error,
            format!(
                "Event added to dead letter queue after {} attempts",
                attempt_count
            ),
            "EventBus".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_context("event_type".to_string(), event_type.to_string())
        .with_context("handler_name".to_string(), handler_name.to_string())
        .with_context("error".to_string(), error_message.to_string())
        .with_context("attempt_count".to_string(), attempt_count.to_string());

        eprintln!("{}", entry.format());
    }

    /// 一般的なエラーログ
    pub fn log_error(
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        additional_context: Option<std::collections::HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Error, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(context) = additional_context {
            for (key, value) in context {
                entry = entry.with_context(key, value);
            }
        }

        eprintln!("{}", entry.format());
    }

    /// 一般的な情報ログ
    pub fn log_info(
        component: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        additional_context: Option<std::collections::HashMap<String, String>>,
    ) {
        let mut entry = LogEntry::new(LogLevel::Info, message.to_string(), component.to_string());

        if let Some(corr_id) = correlation_id {
            entry = entry.with_correlation_id(corr_id);
        }

        if let Some(context) = additional_context {
            for (key, value) in context {
                entry = entry.with_context(key, value);
            }
        }

        println!("{}", entry.format());
    }

    /// サーガステップ完了ログ
    pub fn log_saga_step_completed(
        correlation_id: Uuid,
        step_name: &str,
        target_id: &str,
        execution_time: Duration,
    ) {
        let entry = LogEntry::new(
            LogLevel::Info,
            format!("Saga step completed: {}", step_name),
            "SagaStep".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_execution_time(execution_time)
        .with_context("step_name".to_string(), step_name.to_string())
        .with_context("target_id".to_string(), target_id.to_string());

        println!("{}", entry.format());
    }

    /// デバッグログ（開発時のみ使用）
    pub fn log_debug(component: &str, message: &str, correlation_id: Option<Uuid>) {
        #[cfg(debug_assertions)]
        {
            let mut entry =
                LogEntry::new(LogLevel::Debug, message.to_string(), component.to_string());

            if let Some(corr_id) = correlation_id {
                entry = entry.with_correlation_id(corr_id);
            }

            println!("{}", entry.format());
        }

        // リリースビルドでは何もしない
        #[cfg(not(debug_assertions))]
        {
            let _ = (component, message, correlation_id);
        }
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
    fn test_log_entry_with_execution_time() {
        let execution_time = Duration::from_millis(100);
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        )
        .with_execution_time(execution_time);

        assert_eq!(entry.execution_time, Some(execution_time));
    }

    #[test]
    fn test_log_entry_with_context() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        )
        .with_context("key1".to_string(), "value1".to_string())
        .with_context("key2".to_string(), "value2".to_string());

        assert_eq!(entry.additional_context.len(), 2);
        assert_eq!(
            entry.additional_context.get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            entry.additional_context.get("key2"),
            Some(&"value2".to_string())
        );
    }

    #[test]
    fn test_log_entry_format() {
        let correlation_id = Uuid::new_v4();
        let execution_time = Duration::from_millis(150);

        let entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "TestComponent".to_string(),
        )
        .with_correlation_id(correlation_id)
        .with_execution_time(execution_time)
        .with_context("key1".to_string(), "value1".to_string());

        let formatted = entry.format();

        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("[TestComponent]"));
        assert!(formatted.contains(&format!("[correlation_id: {}]", correlation_id)));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("key1=value1"));
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warning.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    }
}

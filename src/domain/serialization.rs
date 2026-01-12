use crate::domain::event::DomainEvent;
use serde_json;
use thiserror::Error;

/// シリアライゼーションエラー
#[derive(Debug, Error, Clone)]
pub enum SerializationError {
    #[error("JSON serialization failed: {message}. Event type: {event_type}, Field: {field:?}")]
    JsonSerializationFailed {
        message: String,
        event_type: String,
        field: Option<String>,
    },

    #[error("JSON deserialization failed: {message}. Expected type: {expected_type}, Input: {input_preview}")]
    JsonDeserializationFailed {
        message: String,
        expected_type: String,
        input_preview: String,
    },

    #[error("Schema version incompatibility: Expected version {expected}, found {actual}. Event type: {event_type}")]
    SchemaVersionIncompatible {
        expected: u32,
        actual: u32,
        event_type: String,
    },

    #[error("Missing required field: {field_name} in event type {event_type}")]
    MissingRequiredField {
        field_name: String,
        event_type: String,
    },

    #[error("Invalid field value: {field_name} = {field_value} in event type {event_type}. Reason: {reason}")]
    InvalidFieldValue {
        field_name: String,
        field_value: String,
        event_type: String,
        reason: String,
    },

    #[error("Complex object serialization failed: {object_type} in event {event_type}. Details: {details}")]
    ComplexObjectSerializationFailed {
        object_type: String,
        event_type: String,
        details: String,
    },

    #[error("Event schema validation failed: {validation_error} for event type {event_type}")]
    SchemaValidationFailed {
        validation_error: String,
        event_type: String,
    },

    #[error("Unsupported event format: {format} for event type {event_type}")]
    UnsupportedEventFormat { format: String, event_type: String },
}

impl SerializationError {
    /// 入力データのプレビューを生成（デバッグ用、最大100文字）
    fn create_input_preview(input: &str) -> String {
        if input.len() <= 100 {
            input.to_string()
        } else {
            format!("{}...", &input[..97])
        }
    }

    /// JSONシリアライゼーションエラーを作成
    pub fn json_serialization_failed(
        message: String,
        event_type: String,
        field: Option<String>,
    ) -> Self {
        Self::JsonSerializationFailed {
            message,
            event_type,
            field,
        }
    }

    /// JSONデシリアライゼーションエラーを作成
    pub fn json_deserialization_failed(
        message: String,
        expected_type: String,
        input: &str,
    ) -> Self {
        Self::JsonDeserializationFailed {
            message,
            expected_type,
            input_preview: Self::create_input_preview(input),
        }
    }
}

/// イベントシリアライザー
/// ドメインイベントの安全なシリアライゼーション/デシリアライゼーションを提供
pub struct EventSerializer {
    /// サポートするスキーマバージョンの範囲
    supported_versions: std::ops::RangeInclusive<u32>,
}

impl EventSerializer {
    /// 新しいイベントシリアライザーを作成
    pub fn new() -> Self {
        Self {
            supported_versions: 1..=1, // 現在はバージョン1のみサポート
        }
    }



    /// ドメインイベントをJSONにシリアライズ
    pub fn serialize_event(&self, event: &DomainEvent) -> Result<String, SerializationError> {
        // スキーマバージョンの検証
        let event_version = event.metadata().event_version;
        if !self.supported_versions.contains(&event_version) {
            return Err(SerializationError::SchemaVersionIncompatible {
                expected: *self.supported_versions.end(),
                actual: event_version,
                event_type: event.event_type().to_string(),
            });
        }

        // 事前検証：必須フィールドの存在確認
        self.validate_event_before_serialization(event)?;

        // JSONシリアライゼーション実行
        match serde_json::to_string(event) {
            Ok(json) => {
                // シリアライゼーション後の検証
                self.validate_serialized_json(&json, event)?;
                Ok(json)
            }
            Err(serde_error) => {
                // serdeエラーを詳細なエラーメッセージに変換
                let detailed_error = self.analyze_serialization_error(&serde_error, event);
                Err(detailed_error)
            }
        }
    }

    /// JSONからドメインイベントにデシリアライズ
    pub fn deserialize_event(&self, json: &str) -> Result<DomainEvent, SerializationError> {
        // 入力の基本検証
        if json.trim().is_empty() {
            return Err(SerializationError::JsonDeserializationFailed {
                message: "Empty JSON input".to_string(),
                expected_type: "DomainEvent".to_string(),
                input_preview: "".to_string(),
            });
        }

        // JSONの構文検証
        let _: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            SerializationError::json_deserialization_failed(
                format!("Invalid JSON syntax: {}", e),
                "DomainEvent".to_string(),
                json,
            )
        })?;

        // スキーマバージョンの事前チェック
        self.validate_schema_compatibility(json)?;

        // デシリアライゼーション実行
        match serde_json::from_str::<DomainEvent>(json) {
            Ok(event) => {
                // デシリアライゼーション後の検証
                self.validate_deserialized_event(&event)?;
                Ok(event)
            }
            Err(serde_error) => {
                // serdeエラーを詳細なエラーメッセージに変換
                let detailed_error = self.analyze_deserialization_error(&serde_error, json);
                Err(detailed_error)
            }
        }
    }

    /// スキーマ互換性の検証
    fn validate_schema_compatibility(&self, json: &str) -> Result<(), SerializationError> {
        // JSONからメタデータのバージョン情報を抽出
        let parsed: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            SerializationError::json_deserialization_failed(
                format!("Failed to parse JSON for schema validation: {}", e),
                "JSON Value".to_string(),
                json,
            )
        })?;

        // イベントタイプの取得
        let event_type = parsed
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // メタデータからバージョン情報を取得
        let version = parsed
            .get("event_data")
            .and_then(|data| data.get("metadata"))
            .and_then(|metadata| metadata.get("event_version"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1); // デフォルトはバージョン1

        if !self.supported_versions.contains(&version) {
            return Err(SerializationError::SchemaVersionIncompatible {
                expected: *self.supported_versions.end(),
                actual: version,
                event_type: event_type.to_string(),
            });
        }

        Ok(())
    }

    /// シリアライゼーション前のイベント検証
    fn validate_event_before_serialization(
        &self,
        event: &DomainEvent,
    ) -> Result<(), SerializationError> {
        let event_type = event.event_type();
        let metadata = event.metadata();

        // メタデータの必須フィールド検証
        if metadata.event_id.is_nil() {
            return Err(SerializationError::MissingRequiredField {
                field_name: "event_id".to_string(),
                event_type: event_type.to_string(),
            });
        }

        if metadata.correlation_id.is_nil() {
            return Err(SerializationError::MissingRequiredField {
                field_name: "correlation_id".to_string(),
                event_type: event_type.to_string(),
            });
        }

        // イベント固有の検証
        match event {
            DomainEvent::OrderConfirmed(order_confirmed) => {
                if order_confirmed.order_id.to_string().is_empty() {
                    return Err(SerializationError::MissingRequiredField {
                        field_name: "order_id".to_string(),
                        event_type: event_type.to_string(),
                    });
                }

                if order_confirmed.customer_id.to_string().is_empty() {
                    return Err(SerializationError::MissingRequiredField {
                        field_name: "customer_id".to_string(),
                        event_type: event_type.to_string(),
                    });
                }
            }
            DomainEvent::InventoryReserved(inventory_reserved) => {
                if inventory_reserved.order_lines.is_empty() {
                    return Err(SerializationError::InvalidFieldValue {
                        field_name: "order_lines".to_string(),
                        field_value: "empty array".to_string(),
                        event_type: event_type.to_string(),
                        reason: "Order lines cannot be empty for inventory reservation".to_string(),
                    });
                }
            }
            // 他のイベントタイプの検証も必要に応じて追加
            _ => {}
        }

        Ok(())
    }

    /// シリアライゼーション後のJSON検証
    fn validate_serialized_json(
        &self,
        json: &str,
        original_event: &DomainEvent,
    ) -> Result<(), SerializationError> {
        // JSONが有効な構造を持つことを確認
        let parsed: serde_json::Value = serde_json::from_str(json).map_err(|e| {
            SerializationError::json_serialization_failed(
                format!("Generated invalid JSON: {}", e),
                original_event.event_type().to_string(),
                None,
            )
        })?;

        // 必須フィールドの存在確認
        if parsed.get("event_type").is_none() {
            return Err(SerializationError::json_serialization_failed(
                "Missing event_type field in serialized JSON".to_string(),
                original_event.event_type().to_string(),
                Some("event_type".to_string()),
            ));
        }

        if parsed.get("event_data").is_none() {
            return Err(SerializationError::json_serialization_failed(
                "Missing event_data field in serialized JSON".to_string(),
                original_event.event_type().to_string(),
                Some("event_data".to_string()),
            ));
        }

        Ok(())
    }

    /// デシリアライゼーション後のイベント検証
    fn validate_deserialized_event(&self, event: &DomainEvent) -> Result<(), SerializationError> {
        // 基本的な整合性チェック
        let metadata = event.metadata();

        if metadata.event_id.is_nil() {
            return Err(SerializationError::SchemaValidationFailed {
                validation_error: "Deserialized event has nil event_id".to_string(),
                event_type: event.event_type().to_string(),
            });
        }

        if metadata.correlation_id.is_nil() {
            return Err(SerializationError::SchemaValidationFailed {
                validation_error: "Deserialized event has nil correlation_id".to_string(),
                event_type: event.event_type().to_string(),
            });
        }

        Ok(())
    }

    /// シリアライゼーションエラーの詳細分析
    fn analyze_serialization_error(
        &self,
        serde_error: &serde_json::Error,
        event: &DomainEvent,
    ) -> SerializationError {
        let error_msg = serde_error.to_string();
        let event_type = event.event_type().to_string();

        // エラーメッセージから具体的な問題を特定
        if error_msg.contains("invalid type") {
            SerializationError::ComplexObjectSerializationFailed {
                object_type: "Unknown".to_string(),
                event_type,
                details: format!("Type mismatch during serialization: {}", error_msg),
            }
        } else if error_msg.contains("missing field") {
            // フィールド名を抽出しようと試みる
            let field_name = error_msg
                .split("missing field `")
                .nth(1)
                .and_then(|s| s.split('`').next())
                .unwrap_or("unknown");

            SerializationError::MissingRequiredField {
                field_name: field_name.to_string(),
                event_type,
            }
        } else {
            SerializationError::json_serialization_failed(error_msg, event_type, None)
        }
    }

    /// デシリアライゼーションエラーの詳細分析
    fn analyze_deserialization_error(
        &self,
        serde_error: &serde_json::Error,
        json: &str,
    ) -> SerializationError {
        let error_msg = serde_error.to_string();

        // エラーメッセージから具体的な問題を特定
        if error_msg.contains("missing field") {
            let field_name = error_msg
                .split("missing field `")
                .nth(1)
                .and_then(|s| s.split('`').next())
                .unwrap_or("unknown");

            SerializationError::MissingRequiredField {
                field_name: field_name.to_string(),
                event_type: "Unknown".to_string(),
            }
        } else if error_msg.contains("invalid type") {
            SerializationError::json_deserialization_failed(
                format!("Type mismatch: {}", error_msg),
                "DomainEvent".to_string(),
                json,
            )
        } else if error_msg.contains("unknown variant") {
            SerializationError::UnsupportedEventFormat {
                format: "Unknown event variant".to_string(),
                event_type: "Unknown".to_string(),
            }
        } else {
            SerializationError::json_deserialization_failed(
                error_msg,
                "DomainEvent".to_string(),
                json,
            )
        }
    }


}

impl Default for EventSerializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::OrderConfirmed;
    use crate::domain::model::{CustomerId, Money, OrderId};

    #[test]
    fn test_successful_serialization() {
        let serializer = EventSerializer::new();
        let event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
            OrderId::new(),
            CustomerId::new(),
            vec![],
            Money::jpy(1000),
        ));

        let result = serializer.serialize_event(&event);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.contains("OrderConfirmed"));
        assert!(json.contains("event_type"));
        assert!(json.contains("event_data"));
    }

    #[test]
    fn test_successful_deserialization() {
        let serializer = EventSerializer::new();
        let original_event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
            OrderId::new(),
            CustomerId::new(),
            vec![],
            Money::jpy(1000),
        ));

        let json = serializer.serialize_event(&original_event).unwrap();
        let deserialized = serializer.deserialize_event(&json);

        assert!(deserialized.is_ok());
        let deserialized_event = deserialized.unwrap();
        assert_eq!(original_event.event_type(), deserialized_event.event_type());
    }



    #[test]
    fn test_empty_json_deserialization_error() {
        let serializer = EventSerializer::new();
        let result = serializer.deserialize_event("");

        assert!(result.is_err());
        match result.unwrap_err() {
            SerializationError::JsonDeserializationFailed { message, .. } => {
                assert!(message.contains("Empty JSON input"));
            }
            _ => panic!("Expected JsonDeserializationFailed error"),
        }
    }

    #[test]
    fn test_invalid_json_deserialization_error() {
        let serializer = EventSerializer::new();
        let invalid_json = "{ invalid json }";
        let result = serializer.deserialize_event(invalid_json);

        assert!(result.is_err());
        match result.unwrap_err() {
            SerializationError::JsonDeserializationFailed {
                message,
                input_preview,
                ..
            } => {
                assert!(message.contains("Invalid JSON syntax"));
                assert_eq!(input_preview, invalid_json);
            }
            _ => panic!("Expected JsonDeserializationFailed error"),
        }
    }



    #[test]
    fn test_utils_functions() {
        let event = DomainEvent::OrderConfirmed(OrderConfirmed::new(
            OrderId::new(),
            CustomerId::new(),
            vec![],
            Money::jpy(1000),
        ));

        // 直接シリアライザーを使用したテスト
        let serializer = EventSerializer::new();
        let serialized = serializer.serialize_event(&event);
        assert!(serialized.is_ok());

        let json = serialized.unwrap();
        let deserialized = serializer.deserialize_event(&json);
        assert!(deserialized.is_ok());
    }
}

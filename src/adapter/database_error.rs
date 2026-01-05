/// データベースエラー型
/// データベース操作で発生するエラーを表現する
/// 要件: 8.1, 8.2, 8.5
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseError {
    /// データベース接続エラー
    ConnectionError(String),
    /// SQLクエリエラー
    QueryError(String),
    /// マイグレーションエラー
    MigrationError(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionError(msg) => write!(f, "Database connection error: {}", msg),
            DatabaseError::QueryError(msg) => write!(f, "Database query error: {}", msg),
            DatabaseError::MigrationError(msg) => write!(f, "Migration error: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

/// DatabaseErrorからRepositoryErrorへの変換
/// 要件: 8.1, 8.2, 8.5
impl From<DatabaseError> for crate::domain::port::RepositoryError {
    fn from(err: DatabaseError) -> Self {
        match err {
            DatabaseError::ConnectionError(msg) => crate::domain::port::RepositoryError::ConnectionFailed(msg),
            DatabaseError::QueryError(msg) => crate::domain::port::RepositoryError::OperationFailed(msg),
            DatabaseError::MigrationError(msg) => crate::domain::port::RepositoryError::OperationFailed(msg),
        }
    }
}
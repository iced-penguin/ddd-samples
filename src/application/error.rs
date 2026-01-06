use crate::domain::error::DomainError;
use crate::domain::port::RepositoryError;

/// アプリケーション層のエラー型
/// ドメインエラー、リポジトリエラー、イベント発行エラーをラップする
#[derive(Debug)]
pub enum ApplicationError {
    /// ドメインエラー（ビジネスルール違反）
    DomainError(DomainError),
    /// リポジトリエラー（永続化の失敗）
    RepositoryError(RepositoryError),
    /// イベントバス発行エラー
    EventPublishingFailed(String),
    /// エンティティが見つからない
    NotFound(String),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::DomainError(err) => write!(f, "Domain error: {}", err),
            ApplicationError::RepositoryError(err) => write!(f, "Repository error: {}", err),
            ApplicationError::EventPublishingFailed(msg) => {
                write!(f, "Event publishing failed: {}", msg)
            }
            ApplicationError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for ApplicationError {}

// From実装でエラー変換を簡潔に
impl From<DomainError> for ApplicationError {
    fn from(err: DomainError) -> Self {
        ApplicationError::DomainError(err)
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(err: RepositoryError) -> Self {
        ApplicationError::RepositoryError(err)
    }
}

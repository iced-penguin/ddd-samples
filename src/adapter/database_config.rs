use std::env;

/// データベース接続設定を管理する構造体
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: u32,
}

/// 設定エラー
#[derive(Debug)]
pub enum ConfigError {
    InvalidValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidValue(msg) => write!(f, "Invalid configuration value: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

impl DatabaseConfig {
    /// 環境変数から設定を読み取る
    /// 環境変数が設定されていない場合はデフォルト値を使用
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("DATABASE_HOST").unwrap_or_else(|_| "localhost".to_string());

        let port = env::var("DATABASE_PORT")
            .unwrap_or_else(|_| "3306".to_string())
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidValue(format!("Invalid DATABASE_PORT: {}", e)))?;

        let database = env::var("DATABASE_NAME").unwrap_or_else(|_| "bookstore_db".to_string());

        let username = env::var("DATABASE_USER").unwrap_or_else(|_| "bookstore_user".to_string());

        let password =
            env::var("DATABASE_PASSWORD").unwrap_or_else(|_| "bookstore_password".to_string());

        let max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .map_err(|e| {
                ConfigError::InvalidValue(format!("Invalid DATABASE_MAX_CONNECTIONS: {}", e))
            })?;

        Ok(Self {
            host,
            port,
            database,
            username,
            password,
            max_connections,
        })
    }

    /// MySQL接続文字列を生成
    pub fn connection_string(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // テスト間の環境変数の競合を防ぐためのロック
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_from_env_with_all_variables() {
        let _lock = ENV_LOCK.lock().unwrap();

        // 環境変数を設定
        env::set_var("DATABASE_HOST", "testhost");
        env::set_var("DATABASE_PORT", "3307");
        env::set_var("DATABASE_NAME", "testdb");
        env::set_var("DATABASE_USER", "testuser");
        env::set_var("DATABASE_PASSWORD", "testpass");
        env::set_var("DATABASE_MAX_CONNECTIONS", "20");

        let config = DatabaseConfig::from_env().unwrap();

        assert_eq!(config.host, "testhost");
        assert_eq!(config.port, 3307);
        assert_eq!(config.database, "testdb");
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
        assert_eq!(config.max_connections, 20);

        // クリーンアップ
        env::remove_var("DATABASE_HOST");
        env::remove_var("DATABASE_PORT");
        env::remove_var("DATABASE_NAME");
        env::remove_var("DATABASE_USER");
        env::remove_var("DATABASE_PASSWORD");
        env::remove_var("DATABASE_MAX_CONNECTIONS");
    }

    #[test]
    fn test_from_env_with_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();

        // 環境変数をクリア
        env::remove_var("DATABASE_HOST");
        env::remove_var("DATABASE_PORT");
        env::remove_var("DATABASE_NAME");
        env::remove_var("DATABASE_USER");
        env::remove_var("DATABASE_PASSWORD");
        env::remove_var("DATABASE_MAX_CONNECTIONS");

        let config = DatabaseConfig::from_env().unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3306);
        assert_eq!(config.database, "bookstore_db");
        assert_eq!(config.username, "bookstore_user");
        assert_eq!(config.password, "bookstore_password");
        assert_eq!(config.max_connections, 10);
    }

    #[test]
    fn test_connection_string() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            max_connections: 10,
        };

        let conn_str = config.connection_string();
        assert_eq!(conn_str, "mysql://user:pass@localhost:3306/testdb");
    }

    #[test]
    fn test_invalid_port() {
        let _lock = ENV_LOCK.lock().unwrap();

        env::set_var("DATABASE_PORT", "invalid");

        let result = DatabaseConfig::from_env();
        assert!(result.is_err());

        env::remove_var("DATABASE_PORT");
    }

    #[test]
    fn test_invalid_max_connections() {
        let _lock = ENV_LOCK.lock().unwrap();

        env::set_var("DATABASE_MAX_CONNECTIONS", "invalid");

        let result = DatabaseConfig::from_env();
        assert!(result.is_err());

        env::remove_var("DATABASE_MAX_CONNECTIONS");
    }
}

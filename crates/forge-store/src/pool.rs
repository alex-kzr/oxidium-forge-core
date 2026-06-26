use forge_model::{Config, StoreError};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;

pub struct Store {
    pub pool: SqlitePool,
}

impl Store {
    pub async fn connect(config: &Config) -> Result<Self, StoreError> {
        if let Some(parent) = config.db_path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let db_url = format!("sqlite:{}", config.db_path.display());

        let opts = SqliteConnectOptions::from_str(&db_url)
            .map_err(|e| StoreError::Sqlx(e.to_string()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_millis(config.busy_timeout_ms));

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(opts)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        Ok(Store { pool })
    }

    pub async fn close(self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_model::Config;
    use tempfile::tempdir;

    #[tokio::test]
    async fn connects_and_creates_db() {
        let dir = tempdir().unwrap();
        let mut config = Config::default();
        config.db_path = dir.path().join("test.db");

        let store = Store::connect(&config).await.unwrap();

        // Verify WAL mode
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(mode, "wal");

        store.close().await;
    }
}

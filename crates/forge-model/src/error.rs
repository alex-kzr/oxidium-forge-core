use thiserror::Error;

#[derive(Debug, Error)]
pub enum ForgeError {
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("Not found: {resource} {id}")]
    NotFound { resource: String, id: i64 },
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("SQLx error: {0}")]
    Sqlx(String),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Checksum mismatch for migration version {version}")]
    ChecksumMismatch { version: i64 },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

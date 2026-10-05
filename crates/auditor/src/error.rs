use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditorError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] storage::rusqlite::Error),

    #[error("Calculation error: {0}")]
    Calculation(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, AuditorError>;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditorError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("Calculation error: {0}")]
    Calculation(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, AuditorError>;

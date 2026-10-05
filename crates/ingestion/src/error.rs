use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv_async::Error),

    #[error("Zip error: {0}")]
    Zip(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, IngestionError>;

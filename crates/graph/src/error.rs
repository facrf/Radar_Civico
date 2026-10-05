use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] storage::rusqlite::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Graph error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, GraphError>;

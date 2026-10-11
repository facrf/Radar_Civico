use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Tarefa conflitante em execução: {namespace}/{id}")]
    Conflict { namespace: String, id: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Money error: {0}")]
    Money(String),

    #[error("Pool error: {0}")]
    Pool(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

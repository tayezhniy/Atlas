use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("rusqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("node not found: {0}")]
    NodeNotFound(String),
    #[error("edge not found: {0} -> {1}")]
    EdgeNotFound(String, String),
    #[error("migration error: {0}")]
    Migration(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

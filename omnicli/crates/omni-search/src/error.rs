use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Index not found. Run `omni search index <path>` first.")]
    IndexNotFound,

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

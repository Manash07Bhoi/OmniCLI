use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("Not found: {item}")]
    NotFound { item: String },

    #[error("Already exists: {item}")]
    AlreadyExists { item: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<rusqlite::Error> for WorkspaceError {
    fn from(e: rusqlite::Error) -> Self {
        WorkspaceError::Database {
            message: e.to_string(),
        }
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Source not found: {path}")]
    SourceNotFound { path: String },

    #[error("Backup job not found: {name}")]
    JobNotFound { name: String },

    #[error("Snapshot not found: {id}")]
    SnapshotNotFound { id: String },

    #[error("Destination is not a directory: {path}")]
    NotADirectory { path: String },

    #[error("Manifest corrupt: {message}")]
    ManifestCorrupt { message: String },

    #[error("Verify failed: {changed} file(s) differ from backup")]
    VerifyFailed { changed: usize },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

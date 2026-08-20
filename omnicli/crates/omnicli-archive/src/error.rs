use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Unsupported archive format: {0}")]
    UnsupportedFormat(String),

    #[error("Archive not found: {0}")]
    NotFound(String),

    #[error("Corrupt archive: {0}")]
    Corrupt(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

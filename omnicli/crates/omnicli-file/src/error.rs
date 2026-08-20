use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Hash mismatch after copy: src={src} dest={dest}")]
    HashMismatch { src: String, dest: String },

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Invalid duration: {0}")]
    InvalidDuration(String),

    #[error("Invalid size filter: {0}")]
    InvalidSize(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

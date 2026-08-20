use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevError {
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Parse error: {message}")]
    Parse { message: String },

    #[error("Unsupported algorithm: {algo}")]
    UnsupportedAlgo { algo: String },

    #[error("JWT decode error: {message}")]
    JwtDecode { message: String },

    #[error("Regex error: {message}")]
    Regex { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

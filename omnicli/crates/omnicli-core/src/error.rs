use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Hash error: {0}")]
    Hash(String),

    #[error("Unsupported algorithm: {algo}")]
    UnsupportedAlgo { algo: String },
}

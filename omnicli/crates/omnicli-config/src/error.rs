use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("File not found: {path}")]
    NotFound { path: String },

    #[error("Unsupported format: {fmt}. Supported: json, yaml, toml, xml, ini")]
    UnsupportedFormat { fmt: String },

    #[error("Parse error in {path}: {message}")]
    Parse { path: String, message: String },

    #[error("Key not found: {key}")]
    KeyNotFound { key: String },

    #[error("Serialisation error: {message}")]
    Serialise { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

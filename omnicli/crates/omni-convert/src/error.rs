use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported conversion: {from} → {to}. Run `omni convert --list` to see supported pairs.")]
    UnsupportedPair { from: String, to: String },

    #[error("Cannot infer format from path: {0}")]
    UnknownExtension(String),

    #[error("Parse error ({format}): {detail}")]
    ParseError { format: String, detail: String },

    #[error("Encode error ({format}): {detail}")]
    EncodeError { format: String, detail: String },

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

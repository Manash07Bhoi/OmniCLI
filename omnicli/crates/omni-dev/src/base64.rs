use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct Base64Result {
    pub input: String,
    pub output: String,
    pub decoded: bool,
    pub input_len: usize,
    pub output_len: usize,
}

pub fn process_base64(input: &str, decode: bool) -> Result<Base64Result, DevError> {
    if decode {
        let decoded_bytes = STANDARD.decode(input.trim()).map_err(|e| DevError::Parse {
            message: format!("Invalid base64: {e}"),
        })?;
        let output = String::from_utf8(decoded_bytes.clone()).unwrap_or_else(|_| {
            // Fall back to hex representation for binary data
            hex::encode(&decoded_bytes)
        });
        Ok(Base64Result {
            input: if input.len() > 60 {
                format!("{}…", &input[..60])
            } else {
                input.to_owned()
            },
            input_len: input.len(),
            output_len: output.len(),
            output,
            decoded: true,
        })
    } else {
        let output = STANDARD.encode(input.as_bytes());
        Ok(Base64Result {
            input: if input.len() > 60 {
                format!("{}…", &input[..60])
            } else {
                input.to_owned()
            },
            input_len: input.len(),
            output_len: output.len(),
            output,
            decoded: false,
        })
    }
}

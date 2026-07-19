use md5::Md5;
use serde::Serialize;
use sha1::{Digest as _, Sha1};
use sha2::{Digest as _, Sha256};

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct HashResult {
    pub input: String,
    pub algo: String,
    pub digest: String,
    pub input_len: usize,
}

pub fn compute_hash(input: &str, algo: &str) -> Result<HashResult, DevError> {
    let bytes = input.as_bytes();
    let digest = match algo.to_lowercase().as_str() {
        "sha256" | "sha-256" => {
            let mut h = Sha256::new();
            h.update(bytes);
            hex::encode(h.finalize())
        }
        "sha1" | "sha-1" => {
            let mut h = Sha1::new();
            h.update(bytes);
            hex::encode(h.finalize())
        }
        "md5" => {
            use md5::Digest as _;
            let mut h = Md5::new();
            h.update(bytes);
            hex::encode(h.finalize())
        }
        "blake3" => blake3::hash(bytes).to_hex().to_string(),
        other => {
            return Err(DevError::UnsupportedAlgo {
                algo: other.to_owned(),
            })
        }
    };

    Ok(HashResult {
        input: if input.len() > 80 {
            format!("{}…", &input[..80])
        } else {
            input.to_owned()
        },
        algo: algo.to_lowercase(),
        digest,
        input_len: bytes.len(),
    })
}

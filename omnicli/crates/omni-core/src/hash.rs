use std::{
    fmt,
    io::{self, Read},
    path::Path,
    str::FromStr,
};

use anyhow::{Context, Result};
use md5::Md5;
use sha2::{Digest as _, Sha256};

/// Supported hashing algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashAlgo {
    #[default]
    Blake3,
    Sha256,
    Md5,
}

impl fmt::Display for HashAlgo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashAlgo::Blake3 => write!(f, "blake3"),
            HashAlgo::Sha256 => write!(f, "sha256"),
            HashAlgo::Md5 => write!(f, "md5"),
        }
    }
}

impl FromStr for HashAlgo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "blake3" => Ok(HashAlgo::Blake3),
            "sha256" => Ok(HashAlgo::Sha256),
            "md5" => Ok(HashAlgo::Md5),
            other => anyhow::bail!("Unknown hash algorithm: {other}. Use blake3, sha256, or md5."),
        }
    }
}

/// Hash raw bytes using the given algorithm. Returns a lowercase hex string.
pub fn hash_bytes(data: &[u8], algo: HashAlgo) -> String {
    match algo {
        HashAlgo::Blake3 => blake3::hash(data).to_hex().to_string(),
        HashAlgo::Sha256 => {
            let mut h = Sha256::new();
            h.update(data);
            hex::encode(h.finalize())
        }
        HashAlgo::Md5 => {
            use md5::Digest as _;
            let mut h = Md5::new();
            h.update(data);
            hex::encode(h.finalize())
        }
    }
}

/// Stream-hash a file using the given algorithm. Returns a lowercase hex string.
pub fn hash_file(path: &Path, algo: HashAlgo) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open file for hashing: {}", path.display()))?;

    match algo {
        HashAlgo::Blake3 => {
            let mut hasher = blake3::Hasher::new();
            let mut buf = vec![0u8; 65536];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok(hasher.finalize().to_hex().to_string())
        }
        HashAlgo::Sha256 => {
            let mut hasher = Sha256::new();
            io::copy(&mut file, &mut hasher)?;
            Ok(hex::encode(hasher.finalize()))
        }
        HashAlgo::Md5 => {
            use md5::Digest as _;
            let mut hasher = Md5::new();
            io::copy(&mut file, &mut hasher)?;
            Ok(hex::encode(hasher.finalize()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_bytes_blake3() {
        let data = b"hello world";
        let result = hash_bytes(data, HashAlgo::Blake3);
        // BLAKE3 of "hello world" is deterministic
        assert_eq!(result.len(), 64);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_bytes_sha256() {
        let data = b"hello world";
        let result = hash_bytes(data, HashAlgo::Sha256);
        // SHA256 always produces a 64-character hex string
        assert_eq!(result.len(), 64);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
        // Verify it is deterministic
        assert_eq!(result, hash_bytes(data, HashAlgo::Sha256));
    }

    #[test]
    fn test_hash_sha256_known_value() {
        // SHA256 of the empty string is a well-known test vector
        let data = b"";
        let result = hash_bytes(data, HashAlgo::Sha256);
        assert_eq!(
            result,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"test content").unwrap();
        let result = hash_file(tmp.path(), HashAlgo::Blake3).unwrap();
        assert_eq!(result.len(), 64);

        let result2 = hash_file(tmp.path(), HashAlgo::Blake3).unwrap();
        assert_eq!(result, result2, "hashing same file twice gives same result");
    }

    #[test]
    fn test_hash_algo_roundtrip() {
        for (s, expected) in [
            ("blake3", HashAlgo::Blake3),
            ("sha256", HashAlgo::Sha256),
            ("md5", HashAlgo::Md5),
        ] {
            assert_eq!(s.parse::<HashAlgo>().unwrap(), expected);
        }
    }
}

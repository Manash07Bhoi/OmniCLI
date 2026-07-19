use std::path::Path;

use anyhow::Result;
use omni_core::hash::{hash_file, HashAlgo};
use serde::Serialize;

use crate::error::FileError;

/// Result of `omni file hash`.
#[derive(Debug, Serialize)]
pub struct HashResult {
    pub path: String,
    pub algorithm: String,
    pub digest: String,
    pub size_bytes: u64,
}

/// Hash a file and return structured result.
pub fn hash_file_cmd(path: &Path, algo: HashAlgo) -> Result<HashResult, FileError> {
    let meta = std::fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FileError::NotFound(path.display().to_string())
        } else {
            FileError::Io(e)
        }
    })?;

    if !meta.is_file() {
        return Err(FileError::Other(anyhow::anyhow!(
            "{} is not a regular file",
            path.display()
        )));
    }

    let digest = hash_file(path, algo).map_err(FileError::Other)?;
    Ok(HashResult {
        path: path.display().to_string(),
        algorithm: algo.to_string(),
        digest,
        size_bytes: meta.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_file_cmd() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"test data").unwrap();
        let result = hash_file_cmd(f.path(), HashAlgo::Blake3).unwrap();
        assert_eq!(result.algorithm, "blake3");
        assert_eq!(result.digest.len(), 64);
        assert_eq!(result.size_bytes, 9);
    }

    #[test]
    fn test_hash_file_not_found() {
        let result = hash_file_cmd(Path::new("/nonexistent/file.txt"), HashAlgo::Sha256);
        assert!(matches!(result, Err(FileError::NotFound(_))));
    }
}

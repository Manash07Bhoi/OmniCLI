use std::{fs, io::Read, path::Path};

use anyhow::Result;
use omni_core::hash::{hash_file, HashAlgo};
use serde::Serialize;

use crate::error::FileError;

#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub file_a: String,
    pub file_b: String,
    pub identical: bool,
    /// Hash of each file (when `--hash-only` or files differ).
    pub hash_a: Option<String>,
    pub hash_b: Option<String>,
    /// First byte offset that differs (None when identical or hash-only mode).
    pub first_diff_offset: Option<u64>,
    pub size_a: u64,
    pub size_b: u64,
}

/// Compare two files.  `hash_only` skips byte-diff and compares BLAKE3 digests only.
pub fn compare_files(a: &Path, b: &Path, hash_only: bool) -> Result<CompareResult, FileError> {
    let meta_a = fs::metadata(a).map_err(|_| FileError::NotFound(a.display().to_string()))?;
    let meta_b = fs::metadata(b).map_err(|_| FileError::NotFound(b.display().to_string()))?;

    let size_a = meta_a.len();
    let size_b = meta_b.len();

    if hash_only || size_a > 512 * 1024 * 1024 {
        // Hash-only comparison (always used for large files)
        let hash_a = hash_file(a, HashAlgo::Blake3).map_err(FileError::Other)?;
        let hash_b = hash_file(b, HashAlgo::Blake3).map_err(FileError::Other)?;
        let identical = hash_a == hash_b;
        return Ok(CompareResult {
            file_a: a.display().to_string(),
            file_b: b.display().to_string(),
            identical,
            hash_a: Some(hash_a),
            hash_b: Some(hash_b),
            first_diff_offset: None,
            size_a,
            size_b,
        });
    }

    // Fast shortcut: different sizes → different files
    if size_a != size_b {
        let hash_a = hash_file(a, HashAlgo::Blake3).map_err(FileError::Other)?;
        let hash_b = hash_file(b, HashAlgo::Blake3).map_err(FileError::Other)?;
        return Ok(CompareResult {
            file_a: a.display().to_string(),
            file_b: b.display().to_string(),
            identical: false,
            hash_a: Some(hash_a),
            hash_b: Some(hash_b),
            first_diff_offset: Some(size_a.min(size_b)),
            size_a,
            size_b,
        });
    }

    // Byte-level comparison
    let mut fa = fs::File::open(a)?;
    let mut fb = fs::File::open(b)?;
    let mut buf_a = vec![0u8; 65536];
    let mut buf_b = vec![0u8; 65536];
    let mut offset: u64 = 0;
    let mut first_diff: Option<u64> = None;

    loop {
        let n_a = fa.read(&mut buf_a)?;
        let n_b = fb.read(&mut buf_b)?;
        let n = n_a.max(n_b);
        if n == 0 {
            break;
        }
        if first_diff.is_none() {
            for i in 0..n {
                let ba = buf_a.get(i).copied().unwrap_or(0);
                let bb = buf_b.get(i).copied().unwrap_or(0);
                if ba != bb {
                    first_diff = Some(offset + i as u64);
                    break;
                }
            }
        }
        offset += n as u64;
    }

    Ok(CompareResult {
        file_a: a.display().to_string(),
        file_b: b.display().to_string(),
        identical: first_diff.is_none(),
        hash_a: None,
        hash_b: None,
        first_diff_offset: first_diff,
        size_a,
        size_b,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compare_identical() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"hello world").unwrap();
        fs::write(&b, b"hello world").unwrap();
        let result = compare_files(&a, &b, false).unwrap();
        assert!(result.identical);
        assert!(result.first_diff_offset.is_none());
    }

    #[test]
    fn test_compare_different() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"hello WORLD").unwrap();
        fs::write(&b, b"hello world").unwrap();
        let result = compare_files(&a, &b, false).unwrap();
        assert!(!result.identical);
        assert_eq!(result.first_diff_offset, Some(6));
    }

    #[test]
    fn test_compare_hash_only() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"data").unwrap();
        fs::write(&b, b"data").unwrap();
        let result = compare_files(&a, &b, true).unwrap();
        assert!(result.identical);
        assert!(result.hash_a.is_some());
        assert_eq!(result.hash_a, result.hash_b);
    }
}

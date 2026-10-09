use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use omnicli_core::hash::{hash_file, HashAlgo};
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// A group of files that share identical content.
#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub content_hash: String,
    pub size_bytes: u64,
    pub files: Vec<String>,
}

/// Result of `omnicli file duplicate`.
#[derive(Debug, Serialize)]
pub struct DuplicateScanResult {
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicate_files: usize,
    pub wasted_bytes: u64,
}

impl DuplicateScanResult {
    /// True when the scan found nothing worth reporting.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// Scan `dir` for duplicate files using BLAKE3 content hashing.
///
/// Zero-byte files are always skipped: a directory containing many empty files
/// would otherwise report every one of them as a duplicate of every other, which
/// is technically true but never actionable. `min_size` adds a further
/// inclusive lower bound.
pub fn scan_duplicates(dir: &Path, min_size: u64) -> Result<DuplicateScanResult, FileError> {
    if !dir.exists() {
        return Err(FileError::NotFound(dir.display().to_string()));
    }

    // Pass 1: group candidate files by size (cheap pre-filter).
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = match entry {
            Ok(e) if e.file_type().is_file() => e,
            Ok(_) => continue,
            Err(e) => {
                // Surface the problem instead of silently ignoring it: an
                // unreadable subtree would otherwise produce a misleading
                // "no duplicates" answer.
                return Err(FileError::Other(anyhow::anyhow!(
                    "Failed to traverse {}: {e}",
                    dir.display()
                )));
            }
        };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        // Zero-byte files are excluded unconditionally: a directory full of
        // placeholders would otherwise report every empty file as a duplicate.
        if size == 0 || size < min_size {
            continue;
        }
        by_size
            .entry(size)
            .or_default()
            .push(entry.path().to_owned());
    }

    // Pass 2: only hash files that share a size with at least one other file.
    let mut by_hash: HashMap<String, (u64, Vec<PathBuf>)> = HashMap::new();
    for (size, files) in by_size.into_iter().filter(|(_, f)| f.len() > 1) {
        for path in files {
            match hash_file(&path, HashAlgo::Blake3) {
                Ok(h) => by_hash
                    .entry(h)
                    .or_insert_with(|| (size, Vec::new()))
                    .1
                    .push(path),
                Err(e) => {
                    return Err(FileError::Other(anyhow::anyhow!(
                        "Failed to hash {}: {e}",
                        path.display()
                    )))
                }
            }
        }
    }

    let mut groups: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, (_, files))| files.len() > 1)
        .map(|(hash, (size_bytes, mut files))| {
            // Deterministic ordering keeps output stable across runs.
            files.sort();
            DuplicateGroup {
                content_hash: hash,
                size_bytes,
                files: files.iter().map(|p| p.display().to_string()).collect(),
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });

    let total_duplicate_files: usize = groups.iter().map(|g| g.files.len() - 1).sum();
    let wasted_bytes: u64 = groups
        .iter()
        .map(|g| g.size_bytes.saturating_mul(g.files.len() as u64 - 1))
        .sum();

    Ok(DuplicateScanResult {
        groups,
        total_duplicate_files,
        wasted_bytes,
    })
}

/// Delete every file except the first (lexicographically smallest) in each group.
///
/// `dry_run` reports what would be deleted without touching the filesystem.
/// Returns the number of files deleted.
pub fn delete_duplicates(result: &DuplicateScanResult, dry_run: bool) -> Result<u64, FileError> {
    let mut deleted = 0u64;
    for group in &result.groups {
        // Keep the first file; delete the rest.
        for path_str in group.files.iter().skip(1) {
            if !dry_run {
                fs::remove_file(path_str)?;
            }
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scan_duplicates_finds_dupes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"same content").unwrap();
        fs::write(dir.path().join("b.txt"), b"same content").unwrap();
        fs::write(dir.path().join("c.txt"), b"different").unwrap();

        let result = scan_duplicates(dir.path(), 0).unwrap();
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].files.len(), 2);
        assert_eq!(result.total_duplicate_files, 1);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_scan_no_duplicates() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"aaa").unwrap();
        fs::write(dir.path().join("b.txt"), b"bbb").unwrap();

        let result = scan_duplicates(dir.path(), 0).unwrap();
        assert_eq!(result.groups.len(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_skips_empty_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"").unwrap();
        fs::write(dir.path().join("b.txt"), b"").unwrap();

        let result = scan_duplicates(dir.path(), 0).unwrap();
        assert!(result.is_empty(), "empty files must not form a group");
    }

    #[test]
    fn test_min_size_filter_excludes_small_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"tiny").unwrap();
        fs::write(dir.path().join("b.txt"), b"tiny").unwrap();
        fs::write(dir.path().join("c.bin"), vec![7u8; 4096]).unwrap();
        fs::write(dir.path().join("d.bin"), vec![7u8; 4096]).unwrap();

        let result = scan_duplicates(dir.path(), 1024).unwrap();
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].size_bytes, 4096);
    }

    #[test]
    fn test_scan_missing_path_is_an_error() {
        let err = scan_duplicates(Path::new("/definitely/not/here"), 0).unwrap_err();
        assert!(matches!(err, FileError::NotFound(_)));
    }

    #[test]
    fn test_group_ordering_is_deterministic_and_stable() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("z.txt"), b"first payload").unwrap();
        fs::write(dir.path().join("a.txt"), b"first payload").unwrap();

        let result = scan_duplicates(dir.path(), 0).unwrap();
        let files = &result.groups[0].files;
        assert!(
            files[0].ends_with("a.txt"),
            "first entry must be deterministic (got {files:?})"
        );
    }

    #[test]
    fn test_delete_duplicates_dry_run() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"same").unwrap();
        fs::write(dir.path().join("b.txt"), b"same").unwrap();

        let scan = scan_duplicates(dir.path(), 0).unwrap();
        let deleted = delete_duplicates(&scan, true).unwrap();
        assert_eq!(deleted, 1);
        assert!(dir.path().join("a.txt").exists());
        assert!(dir.path().join("b.txt").exists());
    }

    #[test]
    fn test_delete_duplicates_removes_all_but_first() {
        let dir = tempdir().unwrap();
        for name in ["a.txt", "b.txt", "c.txt"] {
            fs::write(dir.path().join(name), b"same").unwrap();
        }

        let scan = scan_duplicates(dir.path(), 0).unwrap();
        let deleted = delete_duplicates(&scan, false).unwrap();
        assert_eq!(deleted, 2);
        assert!(dir.path().join("a.txt").exists(), "first file is kept");
        assert!(!dir.path().join("b.txt").exists());
        assert!(!dir.path().join("c.txt").exists());
    }
}

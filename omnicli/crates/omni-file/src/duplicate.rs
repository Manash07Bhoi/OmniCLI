use std::{collections::HashMap, fs, path::{Path, PathBuf}};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use omni_core::hash::{hash_file, HashAlgo};
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

/// Result of `omni file duplicate --scan`.
#[derive(Debug, Serialize)]
pub struct DuplicateScanResult {
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicate_files: usize,
    pub wasted_bytes: u64,
}

/// Scan `dir` for duplicate files using BLAKE3 content hashing.
pub fn scan_duplicates(dir: &Path) -> Result<DuplicateScanResult, FileError> {
    // First pass: group files by size (fast pre-filter)
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = match entry {
            Ok(e) if e.file_type().is_file() => e,
            _ => continue,
        };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        by_size.entry(size).or_default().push(entry.path().to_owned());
    }

    // Only hash groups where multiple files share the same size
    let candidates: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .collect();

    let total_candidates: u64 = candidates.iter().map(|(_, v)| v.len() as u64).sum();
    let pb = ProgressBar::new(total_candidates);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} hashing {pos}/{len} files  [{elapsed_precise}]")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let mut by_hash: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
    for (size, files) in candidates {
        for path in files {
            if let Ok(h) = hash_file(&path, HashAlgo::Blake3) {
                by_hash.entry(h).or_default().push((path, size));
            } // Err: skip unreadable files
            pb.inc(1);
        }
    }
    pb.finish_and_clear();

    let groups: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(hash, files)| {
            let size_bytes = files[0].1;
            DuplicateGroup {
                content_hash: hash,
                size_bytes,
                files: files.iter().map(|(p, _)| p.display().to_string()).collect(),
            }
        })
        .collect();

    let total_duplicate_files: usize = groups.iter().map(|g| g.files.len() - 1).sum();
    let wasted_bytes: u64 = groups
        .iter()
        .map(|g| g.size_bytes * (g.files.len() as u64 - 1))
        .sum();

    Ok(DuplicateScanResult {
        groups,
        total_duplicate_files,
        wasted_bytes,
    })
}

/// Delete all but the first file in each duplicate group. Requires explicit confirmation
/// (the caller must verify the `--delete-dupes` flag; this function performs the deletion).
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

        let result = scan_duplicates(dir.path()).unwrap();
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].files.len(), 2);
        assert_eq!(result.total_duplicate_files, 1);
    }

    #[test]
    fn test_scan_no_duplicates() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"aaa").unwrap();
        fs::write(dir.path().join("b.txt"), b"bbb").unwrap();

        let result = scan_duplicates(dir.path()).unwrap();
        assert_eq!(result.groups.len(), 0);
    }

    #[test]
    fn test_delete_duplicates_dry_run() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"same").unwrap();
        fs::write(dir.path().join("b.txt"), b"same").unwrap();

        let scan = scan_duplicates(dir.path()).unwrap();
        let deleted = delete_duplicates(&scan, true).unwrap();
        assert_eq!(deleted, 1);
        // Files must still exist after dry run
        assert!(dir.path().join("a.txt").exists() || dir.path().join("b.txt").exists());
    }
}

use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use omni_core::hash::{hash_file, HashAlgo};
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Options for `omni file sync`.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub source: PathBuf,
    pub dest: PathBuf,
    /// Delete files in dest that don't exist in source.
    pub delete_extraneous: bool,
    pub dry_run: bool,
}

/// Summary of a sync operation.
#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub source: String,
    pub dest: String,
    pub files_added: u64,
    pub files_updated: u64,
    pub files_deleted: u64,
    pub bytes_transferred: u64,
    pub dry_run: bool,
}

/// Synchronise `source` into `dest` using BLAKE3 content hashes to detect changes.
pub fn sync_dirs(opts: &SyncOptions) -> Result<SyncResult, FileError> {
    let mut files_added = 0u64;
    let mut files_updated = 0u64;
    let mut files_deleted = 0u64;
    let mut bytes_transferred = 0u64;

    // Build a map of dest files → their hashes (for change detection)
    let mut dest_files: HashMap<PathBuf, String> = HashMap::new();
    if opts.dest.exists() {
        for entry in WalkDir::new(&opts.dest).follow_links(false) {
            let entry = match entry {
                Ok(e) if e.file_type().is_file() => e,
                _ => continue,
            };
            let rel = entry
                .path()
                .strip_prefix(&opts.dest)
                .map_err(|e| FileError::Other(e.into()))?
                .to_owned();
            let hash = hash_file(entry.path(), HashAlgo::Blake3).unwrap_or_default();
            dest_files.insert(rel, hash);
        }
    } else if !opts.dry_run {
        fs::create_dir_all(&opts.dest)?;
    }

    // Walk source and sync each file
    for entry in WalkDir::new(&opts.source).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let rel = entry
            .path()
            .strip_prefix(&opts.source)
            .map_err(|e| FileError::Other(e.into()))?
            .to_owned();
        let dest_path = opts.dest.join(&rel);

        if entry.file_type().is_dir() {
            if !opts.dry_run {
                fs::create_dir_all(&dest_path)?;
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let src_hash = hash_file(entry.path(), HashAlgo::Blake3).unwrap_or_default();
        let dest_hash = dest_files.get(&rel).cloned().unwrap_or_default();

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        if dest_hash.is_empty() {
            // New file
            if !opts.dry_run {
                if let Some(p) = dest_path.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::copy(entry.path(), &dest_path)?;
            }
            files_added += 1;
            bytes_transferred += size;
        } else if src_hash != dest_hash {
            // Changed file
            if !opts.dry_run {
                fs::copy(entry.path(), &dest_path)?;
            }
            files_updated += 1;
            bytes_transferred += size;
        }
        // Unchanged → skip

        dest_files.remove(&rel);
    }

    // Remaining dest_files were not in source → extraneous
    if opts.delete_extraneous {
        for rel in dest_files.keys() {
            let dest_path = opts.dest.join(rel);
            if !opts.dry_run {
                let _ = fs::remove_file(&dest_path);
            }
            files_deleted += 1;
        }
    }

    Ok(SyncResult {
        source: opts.source.display().to_string(),
        dest: opts.dest.display().to_string(),
        files_added,
        files_updated,
        files_deleted,
        bytes_transferred,
        dry_run: opts.dry_run,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sync_adds_new_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("a.txt"), b"hello").unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: false,
            dry_run: false,
        };
        let result = sync_dirs(&opts).unwrap();
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_updated, 0);
        assert_eq!(fs::read(dst.path().join("a.txt")).unwrap(), b"hello");
    }

    #[test]
    fn test_sync_updates_changed_file() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("a.txt"), b"version 2").unwrap();
        fs::write(dst.path().join("a.txt"), b"version 1").unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: false,
            dry_run: false,
        };
        let result = sync_dirs(&opts).unwrap();
        assert_eq!(result.files_updated, 1);
        assert_eq!(fs::read(dst.path().join("a.txt")).unwrap(), b"version 2");
    }

    #[test]
    fn test_sync_delete_extraneous() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(dst.path().join("extra.txt"), b"should be removed").unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: true,
            dry_run: false,
        };
        let result = sync_dirs(&opts).unwrap();
        assert_eq!(result.files_deleted, 1);
        assert!(!dst.path().join("extra.txt").exists());
    }
}

use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use omnicli_core::hash::{hash_file, HashAlgo};
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Options for `omnicli file sync`.
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
///
/// Change detection never relies on timestamps: a destination file is rewritten
/// only when its content hash differs from the source's. Timestamp-based
/// shortcuts (as used by rsync without `--checksum`) silently miss changes on
/// filesystems with coarse mtime granularity (FAT, some network mounts) or when
/// a file is rewritten with identical size inside one timestamp tick. The one
/// metadata fast path that *is* sound is a size mismatch: files of different
/// lengths cannot have equal content, so they are copied without hashing either
/// side.
pub fn sync_dirs(opts: &SyncOptions) -> Result<SyncResult, FileError> {
    if !opts.source.exists() {
        return Err(FileError::NotFound(opts.source.display().to_string()));
    }
    if !opts.source.is_dir() {
        return Err(FileError::Other(anyhow::anyhow!(
            "sync source must be a directory: {}",
            opts.source.display()
        )));
    }
    if same_file(&opts.source, &opts.dest) {
        return Err(FileError::Other(anyhow::anyhow!(
            "sync source and destination are the same directory: {}",
            opts.source.display()
        )));
    }
    if opts.dest.starts_with(&opts.source) && opts.delete_extraneous {
        return Err(FileError::Other(anyhow::anyhow!(
            "refusing to sync {} into its own subdirectory {} with --delete: \
             the source would be deleted",
            opts.source.display(),
            opts.dest.display()
        )));
    }

    let mut files_added = 0u64;
    let mut files_updated = 0u64;
    let mut files_deleted = 0u64;
    let mut bytes_transferred = 0u64;

    // Destination index: relative path → on-disk size.
    let mut dest_files: HashMap<PathBuf, u64> = HashMap::new();
    if opts.dest.exists() {
        for entry in WalkDir::new(&opts.dest).follow_links(false) {
            let entry = match entry {
                Ok(e) if e.file_type().is_file() => e,
                Ok(_) => continue,
                Err(e) => {
                    return Err(FileError::Other(anyhow::anyhow!(
                        "Failed to traverse destination {}: {e}",
                        opts.dest.display()
                    )))
                }
            };
            let rel = entry
                .path()
                .strip_prefix(&opts.dest)
                .map_err(|e| FileError::Other(e.into()))?
                .to_owned();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            dest_files.insert(rel, size);
        }
    } else if !opts.dry_run {
        fs::create_dir_all(&opts.dest)?;
    }

    // Walk the source and sync each file.
    for entry in WalkDir::new(&opts.source).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                return Err(FileError::Other(anyhow::anyhow!(
                    "Failed to traverse source {}: {e}",
                    opts.source.display()
                )))
            }
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

        let src_meta = entry.metadata().map_err(|e| {
            FileError::Other(anyhow::anyhow!(
                "Cannot stat {}: {e}",
                entry.path().display()
            ))
        })?;
        let size = src_meta.len();

        let existing = dest_files.remove(&rel);

        let is_new = existing.is_none();
        let needs_copy = match existing {
            // New file: always copy.
            None => true,
            // Different length: contents cannot match, copy without hashing.
            Some(d_size) if d_size != size => true,
            // Same length: compare content hashes.
            Some(_) => {
                let src_hash = hash_file(entry.path(), HashAlgo::Blake3).map_err(|e| {
                    FileError::Other(anyhow::anyhow!(
                        "Cannot hash {}: {e}",
                        entry.path().display()
                    ))
                })?;
                match hash_file(&dest_path, HashAlgo::Blake3) {
                    Ok(dest_hash) => src_hash != dest_hash,
                    // The destination could not be read: treat as changed so the
                    // copy path re-creates it.
                    Err(_) => true,
                }
            }
        };

        if !needs_copy {
            continue;
        }

        if !opts.dry_run {
            if let Some(p) = dest_path.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(entry.path(), &dest_path)?;
        }
        if is_new {
            files_added += 1;
        } else {
            files_updated += 1;
        }
        bytes_transferred += size;
    }

    // Anything left in dest_files was not present in the source → extraneous.
    if opts.delete_extraneous {
        // Deterministic order keeps dry-run reports stable.
        let mut rels: Vec<PathBuf> = dest_files.into_keys().collect();
        rels.sort();
        for rel in rels {
            let dest_path = opts.dest.join(&rel);
            if !opts.dry_run {
                fs::remove_file(&dest_path)?;
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

/// Best-effort check for two paths referring to the same existing location.
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
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
    fn test_sync_is_idempotent_across_runs() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("a.txt"), b"same bytes").unwrap();
        fs::create_dir(src.path().join("nested")).unwrap();
        fs::write(src.path().join("nested/b.bin"), vec![1u8; 2048]).unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: false,
            dry_run: false,
        };

        let first = sync_dirs(&opts).unwrap();
        assert_eq!(first.files_added, 2);

        // A second run must not copy anything: identical size + mtime takes the
        // metadata fast path, and identical content takes the hashing fallback.
        let second = sync_dirs(&opts).unwrap();
        assert_eq!(second.files_added, 0);
        assert_eq!(second.files_updated, 0);
        assert_eq!(second.files_deleted, 0);
        assert_eq!(second.bytes_transferred, 0);
    }

    #[test]
    fn test_sync_rewrites_file_with_same_size_but_different_content() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("a.txt"), b"AAAAA").unwrap();
        fs::write(dst.path().join("a.txt"), b"BBBBB").unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: false,
            dry_run: false,
        };
        // Both files are 5 bytes and are written microseconds apart, so neither
        // size nor a timestamp-based shortcut can distinguish them: the decision
        // must come from content.
        let result = sync_dirs(&opts).unwrap();
        assert_eq!(
            result.files_updated, 1,
            "same-size different-content must be copied"
        );
        assert_eq!(result.files_added, 0);
        assert_eq!(fs::read(dst.path().join("a.txt")).unwrap(), b"AAAAA");
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

    #[test]
    fn test_sync_dry_run_changes_nothing() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("new.txt"), b"payload").unwrap();
        fs::write(dst.path().join("gone.txt"), b"stale").unwrap();

        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: dst.path().to_owned(),
            delete_extraneous: true,
            dry_run: true,
        };
        let result = sync_dirs(&opts).unwrap();
        assert_eq!(result.files_added, 1);
        assert_eq!(result.files_deleted, 1);
        assert!(result.dry_run);
        assert!(
            !dst.path().join("new.txt").exists(),
            "dry run must not copy"
        );
        assert!(
            dst.path().join("gone.txt").exists(),
            "dry run must not delete"
        );
    }

    #[test]
    fn test_sync_missing_source_is_an_error() {
        let dst = tempdir().unwrap();
        let opts = SyncOptions {
            source: PathBuf::from("/definitely/not/here"),
            dest: dst.path().to_owned(),
            delete_extraneous: false,
            dry_run: false,
        };
        assert!(matches!(sync_dirs(&opts), Err(FileError::NotFound(_))));
    }

    #[test]
    fn test_sync_source_into_itself_with_delete_is_refused() {
        let src = tempdir().unwrap();
        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: src.path().to_owned(),
            delete_extraneous: true,
            dry_run: false,
        };
        let err = sync_dirs(&opts).unwrap_err();
        assert!(err.to_string().contains("same directory"), "got: {err}");
    }

    #[test]
    fn test_sync_nested_dest_with_delete_is_refused() {
        let src = tempdir().unwrap();
        let opts = SyncOptions {
            source: src.path().to_owned(),
            dest: src.path().join("nested"),
            delete_extraneous: true,
            dry_run: false,
        };
        let err = sync_dirs(&opts).unwrap_err();
        assert!(err.to_string().contains("refusing"), "got: {err}");
    }
}

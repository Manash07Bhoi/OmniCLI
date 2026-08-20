use std::{
    fs,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Options for `omni file clean`.
#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub path: std::path::PathBuf,
    /// Remove files older than this duration.
    pub older_than: Option<Duration>,
    /// Also remove empty directories.
    pub empty_dirs: bool,
    pub dry_run: bool,
}

/// Result of `omni file clean`.
#[derive(Debug, Serialize)]
pub struct CleanResult {
    pub files_removed: u64,
    pub dirs_removed: u64,
    pub bytes_freed: u64,
    pub dry_run: bool,
}

/// Run `omni file clean` on the given path with the provided options.
pub fn clean_path(opts: &CleanOptions) -> Result<CleanResult, FileError> {
    let now = SystemTime::now();
    let mut files_removed = 0u64;
    let mut dirs_removed = 0u64;
    let mut bytes_freed = 0u64;

    // Collect candidates — walk in depth-first order so we see file children before dirs.
    let entries: Vec<_> = WalkDir::new(&opts.path)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let ft = entry.file_type();

        if ft.is_file() {
            if let Some(max_age) = opts.older_than {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let age = now.duration_since(mtime).unwrap_or(Duration::MAX);
                if age <= max_age {
                    continue;
                }
                let size = meta.len();
                if !opts.dry_run {
                    fs::remove_file(entry.path())?;
                }
                files_removed += 1;
                bytes_freed += size;
            }
        } else if ft.is_dir() && opts.empty_dirs {
            // Only remove if directory is empty.
            let is_empty = fs::read_dir(entry.path())
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if is_empty && entry.path() != opts.path {
                if !opts.dry_run {
                    let _ = fs::remove_dir(entry.path());
                }
                dirs_removed += 1;
            }
        }
    }

    Ok(CleanResult {
        files_removed,
        dirs_removed,
        bytes_freed,
        dry_run: opts.dry_run,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_clean_empty_dirs() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("empty_sub");
        fs::create_dir(&sub).unwrap();

        let opts = CleanOptions {
            path: dir.path().to_owned(),
            older_than: None,
            empty_dirs: true,
            dry_run: false,
        };
        let result = clean_path(&opts).unwrap();
        assert_eq!(result.dirs_removed, 1);
        assert!(!sub.exists());
    }

    #[test]
    fn test_clean_dry_run_no_deletion() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("empty_sub");
        fs::create_dir(&sub).unwrap();

        let opts = CleanOptions {
            path: dir.path().to_owned(),
            older_than: None,
            empty_dirs: true,
            dry_run: true,
        };
        clean_path(&opts).unwrap();
        assert!(sub.exists(), "dry run must not delete anything");
    }
}

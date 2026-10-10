use std::{
    fs,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Options for `omnicli file clean`.
#[derive(Debug, Clone, Default)]
pub struct CleanOptions {
    pub path: std::path::PathBuf,
    /// Remove files older than this duration.
    pub older_than: Option<Duration>,
    /// Remove files whose *file name* matches this glob (e.g. `*.tmp`).
    pub pattern: Option<String>,
    /// Also remove empty directories.
    pub empty_dirs: bool,
    pub dry_run: bool,
}

/// Result of `omnicli file clean`.
#[derive(Debug, Serialize)]
pub struct CleanResult {
    pub files_removed: u64,
    pub dirs_removed: u64,
    pub bytes_freed: u64,
    pub dry_run: bool,
    /// Absolute paths of the entries that were (or would be) removed.
    pub removed: Vec<String>,
}

/// Run `omnicli file clean` on the given path with the provided options.
///
/// A file is removed when **either** filter matches:
/// * `older_than` — the file's modification time is older than the given age;
/// * `pattern` — the file name matches the glob.
///
/// When neither filter is set and `empty_dirs` is false there is nothing to do;
/// that is reported as an error rather than a silent no-op so the caller notices
/// that their invocation had no effect.
pub fn clean_path(opts: &CleanOptions) -> Result<CleanResult, FileError> {
    if opts.older_than.is_none() && opts.pattern.is_none() && !opts.empty_dirs {
        return Err(FileError::Other(anyhow::anyhow!(
            "Nothing to clean: pass --pattern <GLOB>, --older-than <DURATION>, or --empty-dirs."
        )));
    }

    let glob =
        match &opts.pattern {
            Some(p) => Some(glob::Pattern::new(p).map_err(|e| {
                FileError::Other(anyhow::anyhow!("Invalid glob pattern '{p}': {e}"))
            })?),
            None => None,
        };

    let now = SystemTime::now();
    let mut files_removed = 0u64;
    let mut dirs_removed = 0u64;
    let mut bytes_freed = 0u64;
    let mut removed: Vec<String> = Vec::new();

    // Walk depth-first (children before parents) so that a directory which only
    // becomes empty after its children are removed is still detected as empty.
    let entries: Vec<_> = WalkDir::new(&opts.path)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let ft = entry.file_type();

        if ft.is_file() {
            let is_older = match opts.older_than {
                Some(max_age) => {
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    now.duration_since(mtime).unwrap_or(Duration::MAX) > max_age
                }
                None => false,
            };

            let matches_glob = match &glob {
                Some(g) => g.matches(&entry.file_name().to_string_lossy()),
                None => false,
            };

            if !(is_older || matches_glob) {
                continue;
            }

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if !opts.dry_run {
                fs::remove_file(entry.path())?;
            }
            files_removed += 1;
            bytes_freed += size;
            removed.push(entry.path().display().to_string());
        } else if ft.is_dir() && opts.empty_dirs {
            // The root itself is never removed, only directories strictly inside it.
            if entry.path() == opts.path {
                continue;
            }
            let is_empty = fs::read_dir(entry.path())
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if !is_empty {
                continue;
            }
            if !opts.dry_run {
                fs::remove_dir(entry.path())?;
            }
            dirs_removed += 1;
            removed.push(entry.path().display().to_string());
        }
    }

    Ok(CleanResult {
        files_removed,
        dirs_removed,
        bytes_freed,
        dry_run: opts.dry_run,
        removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn opts(path: &std::path::Path) -> CleanOptions {
        CleanOptions {
            path: path.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn test_clean_without_filters_is_an_error() {
        let dir = tempdir().unwrap();
        let err = clean_path(&opts(dir.path())).unwrap_err();
        assert!(err.to_string().contains("Nothing to clean"), "got: {err}");
    }

    #[test]
    fn test_clean_pattern_removes_only_matching_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.tmp"), b"x").unwrap();
        fs::write(dir.path().join("b.tmp"), b"yy").unwrap();
        fs::write(dir.path().join("keep.txt"), b"zzz").unwrap();

        let result = clean_path(&CleanOptions {
            pattern: Some("*.tmp".to_owned()),
            ..opts(dir.path())
        })
        .unwrap();

        assert_eq!(result.files_removed, 2);
        assert_eq!(result.bytes_freed, 3);
        assert!(!dir.path().join("a.tmp").exists());
        assert!(!dir.path().join("b.tmp").exists());
        assert!(dir.path().join("keep.txt").exists());
        assert_eq!(result.removed.len(), 2);
    }

    #[test]
    fn test_clean_pattern_dry_run_keeps_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.tmp"), b"x").unwrap();

        let result = clean_path(&CleanOptions {
            pattern: Some("*.tmp".to_owned()),
            dry_run: true,
            ..opts(dir.path())
        })
        .unwrap();

        assert_eq!(result.files_removed, 1);
        assert!(result.dry_run);
        assert!(dir.path().join("a.tmp").exists(), "dry run must not delete");
    }

    #[test]
    fn test_clean_invalid_glob_is_an_error() {
        let dir = tempdir().unwrap();
        let err = clean_path(&CleanOptions {
            pattern: Some("[unclosed".to_owned()),
            ..opts(dir.path())
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("Invalid glob pattern"),
            "got: {err}"
        );
    }

    #[test]
    fn test_clean_older_than_removes_only_stale_files() {
        let dir = tempdir().unwrap();
        let fresh = dir.path().join("fresh.txt");
        fs::write(&fresh, b"new").unwrap();

        // A zero-duration age removes everything (every mtime is "older than now").
        let result = clean_path(&CleanOptions {
            older_than: Some(Duration::from_secs(0)),
            ..opts(dir.path())
        })
        .unwrap();
        assert_eq!(result.files_removed, 1);
        assert!(!fresh.exists());
    }

    #[test]
    fn test_clean_older_than_keeps_recent_files() {
        let dir = tempdir().unwrap();
        let fresh = dir.path().join("fresh.txt");
        fs::write(&fresh, b"new").unwrap();

        let result = clean_path(&CleanOptions {
            older_than: Some(Duration::from_secs(86_400)),
            ..opts(dir.path())
        })
        .unwrap();
        assert_eq!(result.files_removed, 0);
        assert!(fresh.exists());
    }

    #[test]
    fn test_clean_empty_dirs() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("empty_sub");
        fs::create_dir(&sub).unwrap();

        let result = clean_path(&CleanOptions {
            empty_dirs: true,
            ..opts(dir.path())
        })
        .unwrap();
        assert_eq!(result.dirs_removed, 1);
        assert!(!sub.exists());
        // The root directory itself is never removed.
        assert!(dir.path().exists());
    }

    #[test]
    fn test_clean_removes_dirs_emptied_by_pattern_pass() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("junk.tmp"), b"junk").unwrap();

        let result = clean_path(&CleanOptions {
            pattern: Some("*.tmp".to_owned()),
            empty_dirs: true,
            ..opts(dir.path())
        })
        .unwrap();

        assert_eq!(result.files_removed, 1);
        assert_eq!(result.dirs_removed, 1);
        assert!(
            !nested.exists(),
            "directory emptied by the pattern pass is removed"
        );
    }

    #[test]
    fn test_clean_dry_run_no_deletion() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("empty_sub");
        fs::create_dir(&sub).unwrap();

        clean_path(&CleanOptions {
            empty_dirs: true,
            dry_run: true,
            ..opts(dir.path())
        })
        .unwrap();
        assert!(sub.exists(), "dry run must not delete anything");
    }
}

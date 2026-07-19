use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Filter by entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryType {
    #[default]
    Any,
    File,
    Dir,
    Symlink,
}

impl EntryType {
    pub fn parse(s: &str) -> Result<Self, FileError> {
        match s {
            "f" | "file" => Ok(Self::File),
            "d" | "dir" => Ok(Self::Dir),
            "l" | "symlink" => Ok(Self::Symlink),
            "a" | "any" | "*" | "" => Ok(Self::Any),
            other => Err(FileError::Other(anyhow::anyhow!(
                "Unknown type filter: {other}. Use: f (file), d (dir), l (symlink), any."
            ))),
        }
    }
}

/// Direction for size filters.
#[derive(Debug, Clone, Copy)]
pub enum SizeOp {
    /// Greater than N bytes.
    GreaterThan(u64),
    /// Less than N bytes.
    LessThan(u64),
}

/// Parse a size expression like `+50M`, `-100K`, `1G`.
pub fn parse_size(s: &str) -> Result<SizeOp, FileError> {
    let s = s.trim();
    let (op, rest) = if let Some(r) = s.strip_prefix('+') {
        (true, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (false, r)
    } else {
        (true, s)
    };

    let (digits, suffix) = rest.split_at(
        rest.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len()),
    );

    if digits.is_empty() {
        return Err(FileError::InvalidSize(s.to_owned()));
    }

    let n: u64 = digits
        .parse()
        .map_err(|_| FileError::InvalidSize(s.to_owned()))?;

    let multiplier: u64 = match suffix.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1_024,
        "M" | "MB" => 1_024 * 1_024,
        "G" | "GB" => 1_024 * 1_024 * 1_024,
        other => {
            return Err(FileError::InvalidSize(format!(
                "Unknown size suffix: {other}"
            )))
        }
    };

    let bytes = n.saturating_mul(multiplier);
    if op {
        Ok(SizeOp::GreaterThan(bytes))
    } else {
        Ok(SizeOp::LessThan(bytes))
    }
}

/// Parse a duration like `7d`, `2h`, `30m`, `1w`.
pub fn parse_duration(s: &str) -> Result<Duration, FileError> {
    let s = s.trim();
    let (digits, suffix) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(s.len()),
    );

    let n: u64 = digits
        .parse()
        .map_err(|_| FileError::InvalidDuration(s.to_owned()))?;

    let secs: u64 = match suffix {
        "s" => n,
        "m" | "min" => n * 60,
        "h" | "hr" => n * 3600,
        "d" | "day" | "days" => n * 86_400,
        "w" | "week" | "weeks" => n * 7 * 86_400,
        other => {
            return Err(FileError::InvalidDuration(format!(
                "Unknown duration suffix: {other}. Use s, m, h, d, or w."
            )))
        }
    };

    Ok(Duration::from_secs(secs))
}

/// Options for `omni file find`.
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    /// Glob / regex pattern to match against the file name. If `None`, matches all.
    pub pattern: Option<String>,
    /// Use the pattern as a regex (otherwise treated as a glob-like substring).
    pub regex: bool,
    /// Filter by entry type.
    pub entry_type: EntryType,
    /// Filter by size (e.g. "+50M").
    pub size: Option<SizeOp>,
    /// Only return entries modified within the past `modified` duration.
    pub modified_within: Option<Duration>,
    /// Root search path.
    pub path: PathBuf,
    /// Maximum depth (None = unlimited).
    pub max_depth: Option<usize>,
    /// Follow symbolic links. When true, inode-based cycle detection prevents
    /// infinite loops from circular symlinks. Default: false.
    pub follow_symlinks: bool,
}

/// A matched filesystem entry.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub file_type: String,
    pub size_bytes: u64,
    /// Unix timestamp of last modification.
    pub modified: i64,
    /// Set to true when a circular symlink was detected and the entry was skipped.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cycle_detected: bool,
}

/// Run `omni file find` and return matched entries.
///
/// When `opts.follow_symlinks` is true, the traversal follows symbolic links but
/// detects cycles by tracking visited directory inodes. Circular symlinks cause a
/// `FileEntry` with `cycle_detected: true` to be emitted (not traversed) so the
/// caller can log or skip them without an infinite loop.
pub fn find_files(opts: &FindOptions) -> Result<Vec<FileEntry>, FileError> {
    let root = if opts.path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &opts.path
    };

    // Compile the optional regex pattern.
    let re: Option<Regex> = if opts.regex {
        match &opts.pattern {
            Some(p) => Some(Regex::new(p)?),
            None => None,
        }
    } else {
        None
    };

    let now = SystemTime::now();
    let mut results = Vec::new();

    // Inode-based cycle detection for --follow-symlinks.
    // Key: (device_id, inode_number) — guaranteed unique per file/directory on Unix.
    // On non-Unix targets we skip cycle detection (Windows doesn't expose real inodes via std).
    #[cfg(unix)]
    let mut visited_dirs: HashSet<(u64, u64)> = HashSet::new();

    let mut walker = WalkDir::new(root).follow_links(opts.follow_symlinks);
    if let Some(d) = opts.max_depth {
        walker = walker.max_depth(d);
    }

    for entry_res in walker {
        let entry = match entry_res {
            Ok(e) => e,
            Err(err) => {
                // walkdir reports loop errors when follow_links=true; surface as cycle_detected.
                if opts.follow_symlinks && err.loop_ancestor().is_some() {
                    if let Some(p) = err.path() {
                        results.push(FileEntry {
                            path: p.display().to_string(),
                            file_type: "symlink".to_owned(),
                            size_bytes: 0,
                            modified: 0,
                            cycle_detected: true,
                        });
                    }
                }
                continue; // skip permission-denied and other errors
            }
        };

        let ft = entry.file_type();

        // On Unix, perform our own inode-based cycle guard for directories.
        #[cfg(unix)]
        if opts.follow_symlinks && ft.is_dir() {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = entry.metadata() {
                let key = (meta.dev(), meta.ino());
                if !visited_dirs.insert(key) {
                    // Already visited this directory via a different path — circular symlink.
                    results.push(FileEntry {
                        path: entry.path().display().to_string(),
                        file_type: "dir".to_owned(),
                        size_bytes: 0,
                        modified: 0,
                        cycle_detected: true,
                    });
                    continue;
                }
            }
        }

        // Type filter
        let matches_type = match opts.entry_type {
            EntryType::Any => true,
            EntryType::File => ft.is_file(),
            EntryType::Dir => ft.is_dir(),
            EntryType::Symlink => ft.is_symlink(),
        };
        if !matches_type {
            continue;
        }

        // Pattern filter
        let file_name = entry.file_name().to_string_lossy();
        if let Some(pat) = &opts.pattern {
            let matches_pattern = if let Some(r) = &re {
                r.is_match(&file_name)
            } else {
                file_name.contains(pat.as_str())
            };
            if !matches_pattern {
                continue;
            }
        }

        // Metadata (size + mtime)
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size_bytes = if ft.is_file() { meta.len() } else { 0 };

        // Size filter
        if let Some(sz) = &opts.size {
            let passes = match sz {
                SizeOp::GreaterThan(n) => size_bytes > *n,
                SizeOp::LessThan(n) => size_bytes < *n,
            };
            if !passes {
                continue;
            }
        }

        // Modified-within filter
        let modified_ts: i64 = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Some(max_age) = opts.modified_within {
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let age = now.duration_since(mtime).unwrap_or(Duration::MAX);
            if age > max_age {
                continue;
            }
        }

        let type_str = if ft.is_file() {
            "file"
        } else if ft.is_dir() {
            "dir"
        } else {
            "symlink"
        };

        results.push(FileEntry {
            path: entry.path().display().to_string(),
            file_type: type_str.to_owned(),
            size_bytes,
            modified: modified_ts,
            cycle_detected: false,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_size() {
        assert!(matches!(parse_size("+50M").unwrap(), SizeOp::GreaterThan(n) if n == 50 * 1024 * 1024));
        assert!(matches!(parse_size("-100K").unwrap(), SizeOp::LessThan(n) if n == 100 * 1024));
        assert!(matches!(parse_size("1G").unwrap(), SizeOp::GreaterThan(n) if n == 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("7d").unwrap(), Duration::from_secs(7 * 86_400));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7_200));
        assert_eq!(parse_duration("1w").unwrap(), Duration::from_secs(7 * 86_400));
    }

    #[test]
    fn test_find_files_basic() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, b"hello").unwrap();

        let opts = FindOptions {
            path: dir.path().to_owned(),
            entry_type: EntryType::File,
            ..Default::default()
        };
        let results = find_files(&opts).unwrap();
        assert!(results.iter().any(|e| e.path.contains("hello.txt")));
    }

    #[test]
    fn test_find_files_pattern() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("match.rs"), b"fn main() {}").unwrap();
        std::fs::write(dir.path().join("skip.txt"), b"text").unwrap();

        let opts = FindOptions {
            path: dir.path().to_owned(),
            pattern: Some(".rs".into()),
            ..Default::default()
        };
        let results = find_files(&opts).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].path.ends_with("match.rs"));
    }

    #[test]
    fn test_find_files_size_filter() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![b'x'; 200]).unwrap();
        std::fs::write(dir.path().join("small.txt"), b"tiny").unwrap();

        let opts = FindOptions {
            path: dir.path().to_owned(),
            size: Some(SizeOp::GreaterThan(100)),
            entry_type: EntryType::File,
            ..Default::default()
        };
        let results = find_files(&opts).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].path.ends_with("big.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn test_find_files_symlink_cycle_terminates() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        // Create a symlink loop: sub/loop -> sub
        symlink(&sub, sub.join("loop")).unwrap();

        let opts = FindOptions {
            path: dir.path().to_owned(),
            follow_symlinks: true,
            ..Default::default()
        };
        // Must terminate without panic or infinite loop
        let results = find_files(&opts).unwrap();
        // At least one cycle_detected entry should be present
        assert!(results.iter().any(|e| e.cycle_detected));
    }

    #[test]
    fn test_find_files_no_follow_symlinks_by_default() {
        // Default: follow_symlinks = false means WalkDir won't follow links
        let opts = FindOptions {
            path: std::path::PathBuf::from("."),
            ..Default::default()
        };
        assert!(!opts.follow_symlinks);
    }
}

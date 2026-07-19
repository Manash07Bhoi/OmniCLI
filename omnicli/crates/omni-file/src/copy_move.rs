use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use omni_core::hash::{hash_file, HashAlgo};
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::FileError;

/// Result summary for copy / move operations.
#[derive(Debug, Serialize)]
pub struct CopyResult {
    pub source: String,
    pub dest: String,
    pub files_copied: u64,
    pub bytes_transferred: u64,
    pub verified: bool,
}

/// Options for `omni file copy` / `omni file move`.
#[derive(Debug, Clone)]
pub struct CopyOptions {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub recursive: bool,
    /// Re-hash after copy to confirm byte-identical transfer.
    pub verify: bool,
    pub dry_run: bool,
}

/// Copy `source` to `dest`.
pub fn copy_path(opts: &CopyOptions) -> Result<CopyResult, FileError> {
    let mut files_copied = 0u64;
    let mut bytes_transferred = 0u64;

    if opts.source.is_file() {
        let dest = if opts.dest.is_dir() {
            opts.dest.join(opts.source.file_name().unwrap())
        } else {
            opts.dest.clone()
        };
        copy_single_file(&opts.source, &dest, opts.verify, opts.dry_run)?;
        bytes_transferred += opts.source.metadata()?.len();
        files_copied += 1;
    } else if opts.source.is_dir() {
        if !opts.recursive {
            return Err(FileError::Other(anyhow::anyhow!(
                "{} is a directory. Use --recursive to copy directories.",
                opts.source.display()
            )));
        }
        for entry in WalkDir::new(&opts.source).follow_links(false) {
            let entry = entry.map_err(|e| FileError::Other(e.into()))?;
            let rel = entry
                .path()
                .strip_prefix(&opts.source)
                .map_err(|e| FileError::Other(e.into()))?;
            let dest = opts.dest.join(rel);

            if entry.file_type().is_dir() {
                if !opts.dry_run {
                    fs::create_dir_all(&dest)?;
                }
            } else if entry.file_type().is_file() {
                copy_single_file(entry.path(), &dest, opts.verify, opts.dry_run)?;
                bytes_transferred += entry.metadata().map(|m| m.len()).unwrap_or(0);
                files_copied += 1;
            }
        }
    } else {
        return Err(FileError::NotFound(opts.source.display().to_string()));
    }

    Ok(CopyResult {
        source: opts.source.display().to_string(),
        dest: opts.dest.display().to_string(),
        files_copied,
        bytes_transferred,
        verified: opts.verify && !opts.dry_run,
    })
}

fn copy_single_file(
    src: &Path,
    dest: &Path,
    verify: bool,
    dry_run: bool,
) -> Result<(), FileError> {
    if dry_run {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dest)?;

    if verify {
        let src_hash = hash_file(src, HashAlgo::Blake3).map_err(FileError::Other)?;
        let dst_hash = hash_file(dest, HashAlgo::Blake3).map_err(FileError::Other)?;
        if src_hash != dst_hash {
            return Err(FileError::HashMismatch {
                src: src_hash,
                dest: dst_hash,
            });
        }
    }
    Ok(())
}

/// Move `source` to `dest` (rename if same filesystem, else copy+delete).
pub fn move_path(opts: &CopyOptions) -> Result<CopyResult, FileError> {
    if opts.dry_run {
        return Ok(CopyResult {
            source: opts.source.display().to_string(),
            dest: opts.dest.display().to_string(),
            files_copied: 0,
            bytes_transferred: 0,
            verified: false,
        });
    }

    // Try atomic rename first.
    if fs::rename(&opts.source, &opts.dest).is_ok() {
        let meta = opts.dest.metadata()?;
        return Ok(CopyResult {
            source: opts.source.display().to_string(),
            dest: opts.dest.display().to_string(),
            files_copied: 1,
            bytes_transferred: if meta.is_file() { meta.len() } else { 0 },
            verified: false,
        });
    }

    // Cross-filesystem: copy then delete.
    let copy_result = copy_path(opts)?;
    if opts.source.is_dir() {
        fs::remove_dir_all(&opts.source)?;
    } else {
        fs::remove_file(&opts.source)?;
    }
    Ok(copy_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_copy_single_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, b"hello").unwrap();

        let opts = CopyOptions {
            source: src.clone(),
            dest: dst.clone(),
            recursive: false,
            verify: true,
            dry_run: false,
        };
        let result = copy_path(&opts).unwrap();
        assert_eq!(result.files_copied, 1);
        assert_eq!(result.bytes_transferred, 5);
        assert!(result.verified);
        assert_eq!(fs::read(&dst).unwrap(), b"hello");
    }

    #[test]
    fn test_move_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, b"data").unwrap();

        let opts = CopyOptions {
            source: src.clone(),
            dest: dst.clone(),
            recursive: false,
            verify: false,
            dry_run: false,
        };
        move_path(&opts).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"data");
    }

    #[test]
    fn test_copy_dry_run() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, b"hello").unwrap();

        let opts = CopyOptions {
            source: src.clone(),
            dest: dst.clone(),
            recursive: false,
            verify: false,
            dry_run: true,
        };
        copy_path(&opts).unwrap();
        assert!(!dst.exists(), "dry_run must not create the file");
    }
}

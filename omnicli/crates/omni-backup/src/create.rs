use std::{
    io::Read,
    path::{Path, PathBuf},
};

use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{
    error::BackupError,
    manifest::{manifest_path, store_path, BackupManifest, ManifestEntry},
};

#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub snapshot_id: String,
    pub job_name: String,
    pub source_path: String,
    pub dest_path: String,
    pub files_total: u64,
    pub files_new: u64,
    pub files_unchanged: u64,
    pub bytes_transferred: u64,
    pub duration_ms: u64,
    pub manifest_path: String,
}

/// Hash a file with BLAKE3 — streaming for large files.
fn hash_file_blake3(path: &Path) -> Result<String, BackupError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn backup_create(
    source: &Path,
    dest: &Path,
    job_name: &str,
    quiet: bool,
) -> Result<BackupResult, BackupError> {
    if !source.exists() {
        return Err(BackupError::SourceNotFound {
            path: source.display().to_string(),
        });
    }
    std::fs::create_dir_all(dest)?;

    let snapshot_id = format!(
        "{}-{}",
        job_name,
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let store = store_path(dest);
    std::fs::create_dir_all(&store)?;

    let start = std::time::Instant::now();
    let mut manifest = BackupManifest::new(job_name, source, &snapshot_id);

    // Collect all files under source
    let entries: Vec<PathBuf> = WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(entries.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                " {bar:40.cyan/blue} {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        pb
    };

    let mut files_new = 0u64;
    let mut files_unchanged = 0u64;
    let mut bytes_transferred = 0u64;

    for file_path in &entries {
        let meta = std::fs::metadata(file_path)?;
        let hash = hash_file_blake3(file_path)?;
        let object_path = store.join(&hash);

        let rel = file_path
            .strip_prefix(source)
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        pb.set_message(rel.clone());

        if !object_path.exists() {
            std::fs::copy(file_path, &object_path)?;
            bytes_transferred += meta.len();
            files_new += 1;
        } else {
            files_unchanged += 1;
        }

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH).ok()
            })
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        manifest.entries.insert(
            rel.clone(),
            ManifestEntry {
                rel_path: rel,
                content_hash: hash,
                size_bytes: meta.len(),
                modified_at: mtime,
            },
        );
        pb.inc(1);
    }
    pb.finish_and_clear();

    let mpath = manifest_path(dest, &snapshot_id);
    manifest.save(&mpath)?;

    Ok(BackupResult {
        snapshot_id,
        job_name: job_name.to_owned(),
        source_path: source.display().to_string(),
        dest_path: dest.display().to_string(),
        files_total: entries.len() as u64,
        files_new,
        files_unchanged,
        bytes_transferred,
        duration_ms: start.elapsed().as_millis() as u64,
        manifest_path: mpath.display().to_string(),
    })
}

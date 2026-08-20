use std::path::Path;

use serde::Serialize;

use crate::{
    error::BackupError,
    manifest::{manifest_path, store_path, BackupManifest},
};

#[derive(Debug, Serialize)]
pub struct RestoreResult {
    pub snapshot_id: String,
    pub target_path: String,
    pub files_restored: u64,
    pub bytes_restored: u64,
    pub duration_ms: u64,
}

pub fn backup_restore(
    backup_dir: &Path,
    snapshot_id: &str,
    target: &Path,
) -> Result<RestoreResult, BackupError> {
    let mpath = manifest_path(backup_dir, snapshot_id);
    if !mpath.exists() {
        return Err(BackupError::SnapshotNotFound {
            id: snapshot_id.to_owned(),
        });
    }

    let manifest = BackupManifest::load(&mpath)?;
    let store = store_path(backup_dir);
    std::fs::create_dir_all(target)?;

    let start = std::time::Instant::now();
    let mut files_restored = 0u64;
    let mut bytes_restored = 0u64;

    for (rel_path, entry) in &manifest.entries {
        let object = store.join(&entry.content_hash);
        if !object.exists() {
            return Err(BackupError::Other(anyhow::anyhow!(
                "Object store missing: {} (hash {})",
                rel_path,
                &entry.content_hash[..12]
            )));
        }

        let dest_file = target.join(rel_path);
        if let Some(parent) = dest_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&object, &dest_file)?;
        bytes_restored += entry.size_bytes;
        files_restored += 1;
    }

    Ok(RestoreResult {
        snapshot_id: snapshot_id.to_owned(),
        target_path: target.display().to_string(),
        files_restored,
        bytes_restored,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// List all snapshots in a backup directory.
#[derive(Debug, Serialize)]
pub struct SnapshotInfo {
    pub snapshot_id: String,
    pub job_name: String,
    pub created_at: String,
    pub file_count: usize,
    pub source_path: String,
}

pub fn list_snapshots(backup_dir: &Path) -> Result<Vec<SnapshotInfo>, BackupError> {
    if !backup_dir.exists() {
        return Ok(vec![]);
    }
    let mut snaps = vec![];
    for entry in std::fs::read_dir(backup_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".manifest.json") {
            if let Ok(m) = BackupManifest::load(&entry.path()) {
                snaps.push(SnapshotInfo {
                    snapshot_id: m.snapshot_id.clone(),
                    job_name: m.job_name.clone(),
                    created_at: m.created_at.clone(),
                    file_count: m.entries.len(),
                    source_path: m.source_path.clone(),
                });
            }
        }
    }
    snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snaps)
}

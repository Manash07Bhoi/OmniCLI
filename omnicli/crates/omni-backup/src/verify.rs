use std::{io::Read, path::Path};

use serde::Serialize;

use crate::{
    error::BackupError,
    manifest::{manifest_path, store_path, BackupManifest},
};

#[derive(Debug, Serialize)]
pub struct VerifyEntry {
    pub rel_path: String,
    pub status: String, // "ok" | "missing" | "corrupt"
    pub expected_hash: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResult {
    pub snapshot_id: String,
    pub files_checked: usize,
    pub files_ok: usize,
    pub files_missing: usize,
    pub files_corrupt: usize,
    pub passed: bool,
    pub entries: Vec<VerifyEntry>,
}

pub fn backup_verify(backup_dir: &Path, snapshot_id: &str) -> Result<VerifyResult, BackupError> {
    let mpath = manifest_path(backup_dir, snapshot_id);
    if !mpath.exists() {
        return Err(BackupError::SnapshotNotFound {
            id: snapshot_id.to_owned(),
        });
    }

    let manifest = BackupManifest::load(&mpath)?;
    let store = store_path(backup_dir);

    let mut results = vec![];
    let mut files_ok = 0usize;
    let mut files_missing = 0usize;
    let mut files_corrupt = 0usize;

    for (rel_path, entry) in &manifest.entries {
        let object = store.join(&entry.content_hash);
        if !object.exists() {
            files_missing += 1;
            results.push(VerifyEntry {
                rel_path: rel_path.clone(),
                status: "missing".to_owned(),
                expected_hash: entry.content_hash.clone(),
            });
            continue;
        }

        // Re-hash stored object
        let actual_hash = hash_file_blake3(&object)?;
        if actual_hash == entry.content_hash {
            files_ok += 1;
            results.push(VerifyEntry {
                rel_path: rel_path.clone(),
                status: "ok".to_owned(),
                expected_hash: entry.content_hash.clone(),
            });
        } else {
            files_corrupt += 1;
            results.push(VerifyEntry {
                rel_path: rel_path.clone(),
                status: "corrupt".to_owned(),
                expected_hash: entry.content_hash.clone(),
            });
        }
    }

    let files_checked = manifest.entries.len();
    let passed = files_missing == 0 && files_corrupt == 0;

    Ok(VerifyResult {
        snapshot_id: snapshot_id.to_owned(),
        files_checked,
        files_ok,
        files_missing,
        files_corrupt,
        passed,
        entries: results,
    })
}

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

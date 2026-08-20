use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::error::BackupError;

pub const MANIFEST_FILE: &str = ".omni-manifest.json";
pub const STORE_DIR: &str = ".omni-store";

/// Per-file entry in the backup manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Relative path from the source root.
    pub rel_path: String,
    /// BLAKE3 hex digest — also the filename in the content store.
    pub content_hash: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// mtime as Unix seconds.
    pub modified_at: i64,
}

/// Top-level backup manifest for a single named job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub job_name: String,
    pub source_path: String,
    pub created_at: String,
    pub snapshot_id: String,
    /// Map of relative path → entry.
    pub entries: HashMap<String, ManifestEntry>,
}

impl BackupManifest {
    pub fn new(job_name: &str, source: &Path, snapshot_id: &str) -> Self {
        Self {
            version: 1,
            job_name: job_name.to_owned(),
            source_path: source.display().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            snapshot_id: snapshot_id.to_owned(),
            entries: HashMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, BackupError> {
        let data = std::fs::read(path)?;
        serde_json::from_slice(&data).map_err(|e| BackupError::ManifestCorrupt {
            message: e.to_string(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), BackupError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

/// Path of the manifest file inside a backup destination.
pub fn manifest_path(dest: &Path, snapshot_id: &str) -> PathBuf {
    dest.join(format!("{snapshot_id}.manifest.json"))
}

/// Path of the content-addressed object store inside a backup destination.
pub fn store_path(dest: &Path) -> PathBuf {
    dest.join(STORE_DIR)
}

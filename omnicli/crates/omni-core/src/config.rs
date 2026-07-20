use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Full `~/.config/omni/omni.toml` schema.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OmniConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub file: FileConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub plugin: PluginConfig,
    #[serde(default)]
    pub colors: ColorsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// "auto" | "always" | "never"
    #[serde(default = "default_color")]
    pub color: String,
    /// "pretty" | "plain" | "json"
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default = "default_editor")]
    pub editor: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            color: default_color(),
            output: default_output(),
            editor: default_editor(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    #[serde(default = "default_hash")]
    pub default_hash: String,
    #[serde(default = "default_true")]
    pub trash_instead_of_delete: bool,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            default_hash: default_hash(),
            trash_instead_of_delete: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_index_paths")]
    pub index_paths: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    #[serde(default = "default_true")]
    pub index_on_idle: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            index_paths: default_index_paths(),
            exclude: default_exclude(),
            index_on_idle: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_backup_dest")]
    pub default_dest: String,
    #[serde(default = "default_true")]
    pub verify_after_create: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            default_dest: default_backup_dest(),
            verify_after_create: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default = "default_workspace_db")]
    pub db_path: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            db_path: default_workspace_db(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    pub allow_community: bool,
    #[serde(default = "default_sandbox")]
    pub sandbox: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            allow_community: false,
            sandbox: default_sandbox(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorsConfig {
    #[serde(default = "c_success")]
    pub success: String,
    #[serde(default = "c_error")]
    pub error: String,
    #[serde(default = "c_warning")]
    pub warning: String,
    #[serde(default = "c_info")]
    pub info: String,
    #[serde(default = "c_accent")]
    pub accent: String,
    #[serde(default = "c_muted")]
    pub muted: String,
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            success: c_success(),
            error: c_error(),
            warning: c_warning(),
            info: c_info(),
            accent: c_accent(),
            muted: c_muted(),
        }
    }
}

// ── Default helpers ───────────────────────────────────────────────────────────
fn default_color() -> String {
    "auto".into()
}
fn default_output() -> String {
    "pretty".into()
}
fn default_editor() -> String {
    "$EDITOR".into()
}
fn default_hash() -> String {
    "blake3".into()
}
fn default_true() -> bool {
    true
}
fn default_index_paths() -> Vec<String> {
    vec!["~/".into(), "~/storage/shared".into()]
}
fn default_exclude() -> Vec<String> {
    vec![".git".into(), "node_modules".into(), ".cache".into()]
}
fn default_backup_dest() -> String {
    "~/omni-backups".into()
}
fn default_workspace_db() -> String {
    "~/.local/share/omni/workspace.db".into()
}
fn default_sandbox() -> String {
    "wasm".into()
}
fn c_success() -> String {
    "#2ECC71".into()
}
fn c_error() -> String {
    "#E74C3C".into()
}
fn c_warning() -> String {
    "#F1C40F".into()
}
fn c_info() -> String {
    "#3498DB".into()
}
fn c_accent() -> String {
    "#9B59B6".into()
}
fn c_muted() -> String {
    "#7F8C8D".into()
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// Return the default config file path: `~/.config/omni/omni.toml`.
pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("omni")
        .join("omni.toml")
}

/// Load config from `path` (or the default path).  Missing file returns defaults.
pub fn load_config(path: Option<&Path>) -> Result<OmniConfig> {
    let p = match path {
        Some(p) => p.to_owned(),
        None => default_config_path(),
    };

    if !p.exists() {
        return Ok(OmniConfig::default());
    }

    let raw = fs::read_to_string(&p)
        .with_context(|| format!("Failed to read config at {}", p.display()))?;

    let cfg: OmniConfig = toml::from_str(&raw)
        .with_context(|| format!("Failed to parse config at {}", p.display()))?;

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let cfg = OmniConfig::default();
        assert_eq!(cfg.core.color, "auto");
        assert_eq!(cfg.file.default_hash, "blake3");
        assert!(cfg.file.trash_instead_of_delete);
    }

    #[test]
    fn test_load_config_missing_file() {
        let cfg = load_config(Some(Path::new("/nonexistent/omni.toml"))).unwrap();
        // Missing file → defaults
        assert_eq!(cfg.core.output, "pretty");
    }

    #[test]
    fn test_load_config_partial_toml() {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            r#"
[core]
color = "never"
"#
        )
        .unwrap();
        let cfg = load_config(Some(f.path())).unwrap();
        assert_eq!(cfg.core.color, "never");
        // Unset fields keep their defaults
        assert_eq!(cfg.file.default_hash, "blake3");
    }
}

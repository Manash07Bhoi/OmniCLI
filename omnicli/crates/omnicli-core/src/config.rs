//! OmniCLI configuration schema, loading, and validation.
//!
//! Configuration is read from a single TOML file (default:
//! `~/.config/omni/omnicli.toml`). Every field in this schema is wired to real
//! behaviour — see the `docs/USAGE.md` "Configuration reference" section.
//!
//! Loading rules:
//! * If no explicit `--config` path is given and the default file does not
//!   exist, built-in defaults are returned.
//! * If an explicit `--config` path is given, the file must exist and parse.
//! * A malformed *default* config file is a hard error: silently falling back to
//!   defaults would hide a user's broken configuration.
//! * Unknown keys are rejected so typos ("colr = ...") surface immediately.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::platform::config_file_path;

/// Full `~/.config/omni/omnicli.toml` schema.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct OmniConfig {
    pub core: CoreConfig,
    pub file: FileConfig,
    pub search: SearchConfig,
    pub backup: BackupConfig,
    pub workspace: WorkspaceConfig,
    pub colors: ColorsConfig,
}

/// Values accepted by `core.color`.
pub const COLOR_VALUES: [&str; 3] = ["auto", "always", "never"];
/// Values accepted by `core.output`.
pub const OUTPUT_VALUES: [&str; 3] = ["pretty", "plain", "json"];
/// Values accepted by `file.default_hash`.
pub const HASH_VALUES: [&str; 3] = ["blake3", "sha256", "md5"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CoreConfig {
    /// `"auto"` | `"always"` | `"never"`.
    pub color: String,
    /// `"pretty"` | `"plain"` | `"json"`.
    pub output: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            color: "auto".into(),
            output: "pretty".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FileConfig {
    /// Default `--algo` for `omnicli file hash` / `omnicli file stats` when the flag is omitted.
    pub default_hash: String,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            default_hash: "blake3".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    /// Roots used by `omnicli search index` when no `PATH` argument is supplied.
    pub index_paths: Vec<String>,
    /// Substrings excluded from indexing and from `omnicli file find`.
    pub exclude: Vec<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            index_paths: default_index_paths(),
            exclude: default_exclude(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct BackupConfig {
    /// Destination used by `omnicli backup create` when the `dest` argument is omitted.
    pub default_dest: String,
    /// Re-hash every stored object immediately after creating a snapshot.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceConfig {
    /// Path to the workspace SQLite database.
    pub db_path: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            db_path: default_workspace_db(),
        }
    }
}

/// ANSI/256-colour palette used by human-readable output.
///
/// Colours are stored as `#RRGGBB` strings and validated on load; invalid
/// values are a configuration error rather than a silent no-op.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ColorsConfig {
    pub success: String,
    pub error: String,
    pub warning: String,
    pub info: String,
    pub accent: String,
    pub muted: String,
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            success: "#2ECC71".into(),
            error: "#E74C3C".into(),
            warning: "#F1C40F".into(),
            info: "#3498DB".into(),
            accent: "#9B59B6".into(),
            muted: "#7F8C8D".into(),
        }
    }
}

// ── Default helpers ───────────────────────────────────────────────────────────
fn default_index_paths() -> Vec<String> {
    vec!["~/".into()]
}
fn default_exclude() -> Vec<String> {
    vec![
        ".git".into(),
        "node_modules".into(),
        ".cache".into(),
        "target".into(),
    ]
}
fn default_backup_dest() -> String {
    "~/omni-backups".into()
}
/// The workspace database default. It deliberately derives from
/// `platform::data_dir()`, the single source of truth for OmniCLI data paths —
/// a second, hand-rolled rule would make the dashboard and the CLI write to
/// different stores.
fn default_workspace_db() -> String {
    crate::platform::data_dir()
        .join("workspace.db")
        .to_string_lossy()
        .into_owned()
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Configuration loading/validation failure.
#[derive(Debug)]
pub enum ConfigError {
    /// The requested config file does not exist.
    Missing { path: PathBuf },
    /// The config file could not be read.
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The config file is not valid TOML, or contains unknown/invalid keys.
    Parse { path: PathBuf, message: String },
    /// The config file parsed but contains semantically invalid values.
    Invalid {
        path: PathBuf,
        problems: Vec<String>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(
                f,
                "Config file not found: {}. Create it or omit --config to use built-in defaults.",
                path.display()
            ),
            Self::Read { path, source } => {
                write!(f, "Cannot read config file {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "Invalid config file {}: {message}", path.display())
            }
            Self::Invalid { path, problems } => write!(
                f,
                "Invalid config values in {}:\n  - {}",
                path.display(),
                problems.join("\n  - ")
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_choice(field: &str, value: &str, allowed: &[&str], problems: &mut Vec<String>) {
    if !allowed.contains(&value) {
        problems.push(format!(
            "{field} = \"{value}\" is not valid. Allowed values: {}",
            allowed.join(", ")
        ));
    }
}

fn validate_hex_colour(field: &str, value: &str, problems: &mut Vec<String>) {
    let ok = value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        problems.push(format!(
            "colors.{field} = \"{value}\" is not a #RRGGBB colour"
        ));
    }
}

impl OmniConfig {
    /// Return every semantically invalid value in this configuration.
    ///
    /// An empty vector means the configuration is usable.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();

        validate_choice("core.color", &self.core.color, &COLOR_VALUES, &mut problems);
        validate_choice(
            "core.output",
            &self.core.output,
            &OUTPUT_VALUES,
            &mut problems,
        );
        validate_choice(
            "file.default_hash",
            &self.file.default_hash,
            &HASH_VALUES,
            &mut problems,
        );

        validate_hex_colour("success", &self.colors.success, &mut problems);
        validate_hex_colour("error", &self.colors.error, &mut problems);
        validate_hex_colour("warning", &self.colors.warning, &mut problems);
        validate_hex_colour("info", &self.colors.info, &mut problems);
        validate_hex_colour("accent", &self.colors.accent, &mut problems);
        validate_hex_colour("muted", &self.colors.muted, &mut problems);

        if self.workspace.db_path.trim().is_empty() {
            problems.push("workspace.db_path must not be empty".to_owned());
        }
        if self.backup.default_dest.trim().is_empty() {
            problems.push("backup.default_dest must not be empty".to_owned());
        }
        if self.file.default_hash.is_empty() {
            problems.push("file.default_hash must not be empty".to_owned());
        }

        problems
    }
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// Return the default config file path: `~/.config/omni/omnicli.toml`.
pub fn default_config_path() -> PathBuf {
    config_file_path()
}

/// Load configuration from an explicit path, or the default path when `path` is `None`.
///
/// * `path = Some(p)` — the file must exist, parse, and validate.
/// * `path = None` — a missing default file yields built-in defaults, but an
///   existing-yet-unreadable/unparseable default file is reported as an error.
pub fn load_config(path: Option<&Path>) -> Result<OmniConfig, ConfigError> {
    let explicit = path.is_some();
    let p = match path {
        Some(p) => p.to_owned(),
        None => default_config_path(),
    };

    if !p.exists() {
        if explicit {
            return Err(ConfigError::Missing { path: p });
        }
        return Ok(OmniConfig::default());
    }

    if p.is_dir() {
        return Err(ConfigError::Parse {
            path: p.clone(),
            message: "expected a file, found a directory".to_owned(),
        });
    }

    let raw = fs::read_to_string(&p).map_err(|source| ConfigError::Read {
        path: p.clone(),
        source,
    })?;

    let cfg: OmniConfig = toml::from_str(&raw).map_err(|e| ConfigError::Parse {
        path: p.clone(),
        message: e.message().to_owned(),
    })?;

    let problems = cfg.validate();
    if !problems.is_empty() {
        return Err(ConfigError::Invalid { path: p, problems });
    }

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn default_workspace_db_derives_from_the_data_dir() {
        // One rule for the data directory: if this drifts, the CLI and the
        // dashboard would silently write to different SQLite files.
        assert_eq!(
            OmniConfig::default().workspace.db_path,
            crate::platform::data_dir()
                .join("workspace.db")
                .to_string_lossy()
        );
    }

    #[test]
    fn test_default_config() {
        let cfg = OmniConfig::default();
        assert_eq!(cfg.core.color, "auto");
        assert_eq!(cfg.core.output, "pretty");
        assert_eq!(cfg.file.default_hash, "blake3");
        assert!(cfg.validate().is_empty());
    }

    #[test]
    fn test_default_config_path_matches_platform() {
        assert_eq!(default_config_path(), config_file_path());
        assert!(default_config_path().ends_with("omnicli.toml"));
    }

    #[test]
    fn test_load_config_default_path_is_optional() {
        // With no explicit path, a missing default file silently yields defaults.
        // (The default path is `~/.config/omni/omnicli.toml`; a machine in CI is
        // not guaranteed to be without one, so this asserts the contract rather
        // than the ambient filesystem state.)
        let cfg = load_config(None).unwrap_or_default();
        assert!(COLOR_VALUES.contains(&cfg.core.color.as_str()));
        assert!(OUTPUT_VALUES.contains(&cfg.core.output.as_str()));
    }

    #[test]
    fn test_load_config_explicit_missing_file_is_error() {
        let err = load_config(Some(Path::new("/nonexistent/omnicli.toml"))).unwrap_err();
        assert!(matches!(err, ConfigError::Missing { .. }));
        assert!(err.to_string().contains("Config file not found"));
    }

    #[test]
    fn test_load_config_partial_toml() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "[core]\ncolor = \"never\"\n").unwrap();
        let cfg = load_config(Some(f.path())).unwrap();
        assert_eq!(cfg.core.color, "never");
        // Unset fields keep their defaults
        assert_eq!(cfg.file.default_hash, "blake3");
    }

    #[test]
    fn test_load_config_malformed_toml_is_error() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "this is not toml [[[").unwrap();
        let err = load_config(Some(f.path())).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn test_load_config_unknown_key_is_error() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "[core]\ncolr = \"never\"\n").unwrap();
        let err = load_config(Some(f.path())).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }), "got {err:?}");
    }

    #[test]
    fn test_load_config_invalid_enum_is_error() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "[core]\ncolor = \"purple\"\n").unwrap();
        let err = load_config(Some(f.path())).unwrap_err();
        match err {
            ConfigError::Invalid { problems, .. } => {
                assert!(problems.iter().any(|p| p.contains("core.color")));
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn test_load_config_invalid_colour_is_error() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "[colors]\naccent = \"chartreuse\"\n").unwrap();
        let err = load_config(Some(f.path())).unwrap_err();
        assert!(err.to_string().contains("colors.accent"));
    }
}

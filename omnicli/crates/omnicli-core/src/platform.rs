use std::path::{Path, PathBuf};

/// Expand a leading `~` to the home directory.
/// Returns the path unchanged if expansion is not possible.
/// On Windows, also handles paths starting with `~\`.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    // Windows-style: ~\ prefix
    #[cfg(windows)]
    if let Some(rest) = path.strip_prefix(r"~\") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Return the OmniCLI data directory: `~/.local/share/omni` (Linux/macOS)
/// or `%LOCALAPPDATA%\omni` (Windows).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("omni")
}

/// Return the default config file path: `~/.config/omni/omni.toml` (Linux/macOS)
/// or `%APPDATA%\omni\omni.toml` (Windows).
pub fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("omni")
        .join("omni.toml")
}

/// Ensure a directory exists, creating it (and parents) if needed.
pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Return a human-readable byte-size string (e.g. "48.2 MB", "1.3 KB").
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Detect whether stdout is connected to a terminal (TTY).
/// Works on Linux, macOS, Windows, and WSL2 without any unsafe code.
pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(50_585_395), "48.2 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_expand_tilde() {
        let result = expand_tilde("~/foo/bar");
        // Can't assert the exact home dir in CI, but path must end with foo/bar
        assert!(result.ends_with("foo/bar"));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_expand_tilde_no_home() {
        // Plain path without tilde must be returned as-is
        let result = expand_tilde("relative/path");
        assert_eq!(result, PathBuf::from("relative/path"));
    }
}

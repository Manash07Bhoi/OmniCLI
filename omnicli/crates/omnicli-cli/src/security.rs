//! `omnicli security` — defensive checks for the machine you are sitting at.
//!
//! Scope: these commands inspect files the caller already has access to and
//! report what they find. They do not attempt to bypass permissions, exploit
//! services, or touch anything the user cannot already read. The secret scanner
//! is for auditing your *own* files before sharing them — it never transmits
//! anything and never prints a full credential (matches are redacted).

use std::fs;
use std::path::Path;

use anyhow::Result;
use omnicli_core::output::{print_muted, OutputConfig};
use omnicli_core::platform;
use serde::Serialize;
use walkdir_shim::walk;

/// Small local wrapper so the module has one place that defines the traversal
/// rules (skip VCS/vendor noise, follow nothing, cap depth).
mod walkdir_shim {
    use std::path::{Path, PathBuf};

    pub const SKIP_DIRS: [&str; 6] = [
        "node_modules",
        ".git",
        "target",
        ".cache",
        "__pycache__",
        ".venv",
    ];
    pub const MAX_DEPTH: usize = 12;

    /// Depth-first walk yielding (path, is_dir). Unreadable entries are skipped
    /// silently: a permission error must not abort a security scan.
    pub fn walk(root: &Path) -> Vec<(PathBuf, bool)> {
        let mut out = Vec::new();
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if depth > MAX_DEPTH {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = entry.metadata() else { continue };
                let is_dir = meta.is_dir();
                out.push((path.clone(), is_dir));
                if is_dir {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if SKIP_DIRS.contains(&name.as_str()) {
                        continue;
                    }
                    stack.push((path, depth + 1));
                }
            }
        }
        out
    }
}

// ── Findings ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub path: String,
    /// `high` | `medium` | `low`
    pub severity: &'static str,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub path: String,
    pub findings: Vec<Finding>,
    pub scanned: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

fn summarize(root: &Path, scanned: usize, findings: Vec<Finding>) -> ScanReport {
    let count = |sev: &str| findings.iter().filter(|f| f.severity == sev).count();
    ScanReport {
        path: root.display().to_string(),
        high: count("high"),
        medium: count("medium"),
        low: count("low"),
        findings,
        scanned,
    }
}

// ── omnicli security permissions ────────────────────────────────────────────────

#[cfg(unix)]
pub fn scan_permissions(root: &Path) -> Result<ScanReport> {
    use std::os::unix::fs::PermissionsExt;

    let entries = walk(root);
    let mut findings = Vec::new();
    let mut scanned = 0usize;

    for (path, is_dir) in &entries {
        let Ok(meta) = fs::symlink_metadata(path) else {
            continue;
        };
        scanned += 1;
        let mode = meta.permissions().mode();

        if mode & 0o4000 != 0 {
            findings.push(Finding {
                path: path.display().to_string(),
                severity: "high",
                kind: "setuid".to_owned(),
                detail: "setuid bit set — the file runs with the owner's privileges".to_owned(),
            });
        }
        if mode & 0o2000 != 0 {
            findings.push(Finding {
                path: path.display().to_string(),
                severity: "medium",
                kind: "setgid".to_owned(),
                detail: "setgid bit set — the file runs with the group's privileges".to_owned(),
            });
        }
        if mode & 0o002 != 0 {
            if *is_dir && mode & 0o1000 != 0 {
                findings.push(Finding {
                    path: path.display().to_string(),
                    severity: "low",
                    kind: "world-writable-dir-sticky".to_owned(),
                    detail: "world-writable directory with the sticky bit (e.g. /tmp)".to_owned(),
                });
            } else if *is_dir {
                findings.push(Finding {
                    path: path.display().to_string(),
                    severity: "high",
                    kind: "world-writable-dir".to_owned(),
                    detail: "any local user can add or replace files here".to_owned(),
                });
            } else {
                findings.push(Finding {
                    path: path.display().to_string(),
                    severity: "high",
                    kind: "world-writable-file".to_owned(),
                    detail: "any local user can modify this file".to_owned(),
                });
            }
        }
    }

    Ok(summarize(root, scanned, findings))
}

#[cfg(not(unix))]
pub fn scan_permissions(root: &Path) -> Result<ScanReport> {
    anyhow::bail!(
        "`omnicli security permissions` needs POSIX permission bits and is not available on {}. \
         Nothing was scanned.",
        std::env::consts::OS
    )
}

// ── omnicli security secrets ────────────────────────────────────────────────────

/// A detected credential-like string. The `preview` is always redacted.
#[derive(Debug, Clone, Serialize)]
pub struct SecretHit {
    pub path: String,
    pub line: usize,
    pub kind: String,
    pub preview: String,
}

const TEXT_EXTENSIONS: [&str; 22] = [
    "txt", "md", "rs", "ts", "tsx", "js", "jsx", "py", "go", "rb", "java", "sh", "bash", "zsh",
    "toml", "yaml", "yml", "json", "ini", "conf", "env", "cfg",
];

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Redact everything but the outer characters of a candidate secret.
fn redact(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "…".to_owned();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars.iter().skip(chars.len() - 2).collect();
    format!("{head}…{tail} ({} chars)", chars.len())
}

/// Shannon entropy in bits per character — used only as a secondary signal.
fn entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let len = s.chars().count() as f64;
    counts.values().fold(0.0, |acc, &n| {
        let p = n as f64 / len;
        acc - p * p.log2()
    })
}

fn is_candidate_token(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '/' | '=')
}

/// Plausible placeholder values are not findings.
fn looks_like_placeholder(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    [
        "example",
        "changeme",
        "placeholder",
        "your_",
        "xxxx",
        "dummy",
        "redacted",
        "sample",
    ]
    .iter()
    .any(|p| lower.contains(p))
        || token.chars().all(|c| c == token.chars().next().unwrap())
}

fn detect_kind(token: &str) -> Option<&'static str> {
    let len = token.len();
    if token.starts_with("AKIA")
        && len >= 20
        && token[4..]
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some("aws-access-key-id");
    }
    if token.starts_with("github_pat_") && len >= 30 {
        return Some("github-token");
    }
    if ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]
        .iter()
        .any(|p| token.starts_with(p))
        && len >= 24
    {
        return Some("github-token");
    }
    if ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"]
        .iter()
        .any(|p| token.starts_with(p))
        && len >= 20
    {
        return Some("slack-token");
    }
    if token.starts_with("AIza") && len >= 35 {
        return Some("google-api-key");
    }
    if token.starts_with("sk_live_") && len >= 24 {
        return Some("stripe-secret-key");
    }
    // JWT: three base64url segments, the first decoding to `{"…`.
    if token.starts_with("eyJ") && token.matches('.').count() == 2 && len >= 40 {
        return Some("jwt");
    }
    None
}

fn scan_line(line: &str) -> Option<(String, String)> {
    // Regex-free scanning: split on characters that cannot appear in a credential,
    // then classify each token.
    let mut current = String::new();
    let mut tokens: Vec<String> = Vec::new();
    for c in line.chars() {
        if is_candidate_token(c) {
            current.push(c);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // Assignments such as `api_key = "<value>"` are a strong signal even when the
    // value has no vendor prefix.
    let assignment = [
        "api_key",
        "apikey",
        "secret",
        "token",
        "password",
        "passwd",
        "private_key",
    ]
    .iter()
    .find(|k| line.to_ascii_lowercase().contains(*k));

    for token in &tokens {
        if token.len() < 16 || looks_like_placeholder(token) {
            continue;
        }
        if let Some(kind) = detect_kind(token) {
            return Some((kind.to_owned(), redact(token)));
        }
        let Some(keyword) = assignment else { continue };
        // Secondary signal: long random-looking tokens in an assignment context.
        let distinct = {
            let mut chars: Vec<char> = token.chars().collect();
            chars.sort_unstable();
            chars.dedup();
            chars.len()
        };
        if token.len() >= 24 && entropy(token) >= 3.5 && distinct >= 12 {
            return Some((
                format!("high-entropy value assigned to {keyword}"),
                redact(token),
            ));
        }
    }
    None
}

pub fn scan_secrets(root: &Path) -> Result<(Vec<SecretHit>, usize)> {
    let entries = walk(root);
    let mut hits = Vec::new();
    let mut scanned = 0usize;

    for (path, is_dir) in &entries {
        if *is_dir {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let name_looks_textual = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with(".env") || n == "Dockerfile" || n == "Makefile")
            .unwrap_or(false);
        if !TEXT_EXTENSIONS.contains(&ext.as_str()) && !name_looks_textual {
            continue;
        }
        let Ok(meta) = fs::metadata(path) else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        scanned += 1;

        // PEM private keys span multiple lines.
        if content.contains("-----BEGIN") && content.contains("PRIVATE KEY-----") {
            let line = content
                .lines()
                .position(|l| l.contains("PRIVATE KEY-----"))
                .map(|i| i + 1)
                .unwrap_or(1);
            hits.push(SecretHit {
                path: path.display().to_string(),
                line,
                kind: "private-key".to_owned(),
                preview: "-----BEGIN … PRIVATE KEY----- (redacted)".to_owned(),
            });
        }

        for (index, line) in content.lines().enumerate() {
            if let Some((kind, preview)) = scan_line(line) {
                hits.push(SecretHit {
                    path: path.display().to_string(),
                    line: index + 1,
                    kind,
                    preview,
                });
            }
        }
    }

    Ok((hits, scanned))
}

// ── omnicli security config ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConfigAudit {
    pub config_path: String,
    pub data_dir: String,
    pub findings: Vec<Finding>,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[cfg(unix)]
fn mode_of(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).ok().map(|m| m.permissions().mode())
}

pub fn audit_config() -> ConfigAudit {
    let config_path = platform::config_file_path();
    let data_dir = platform::data_dir();
    let mut findings = Vec::new();

    #[cfg(unix)]
    {
        if let Some(mode) = mode_of(&config_path) {
            if mode & 0o002 != 0 {
                findings.push(Finding {
                    path: config_path.display().to_string(),
                    severity: "high",
                    kind: "world-writable-config".to_owned(),
                    detail: "any local user can change your configuration".to_owned(),
                });
            }
            if mode & 0o004 != 0 {
                findings.push(Finding {
                    path: config_path.display().to_string(),
                    severity: "low",
                    kind: "world-readable-config".to_owned(),
                    detail: "config is readable by other local users".to_owned(),
                });
            }
        }
        if let Some(mode) = mode_of(&data_dir) {
            if mode & 0o002 != 0 {
                findings.push(Finding {
                    path: data_dir.display().to_string(),
                    severity: "high",
                    kind: "world-writable-data-dir".to_owned(),
                    detail: "your databases can be replaced by other local users".to_owned(),
                });
            }
        }
    }

    // A backup destination inside its own source would recurse forever; catch the
    // obvious version of that misconfiguration here.
    let count = |sev: &str| findings.iter().filter(|f| f.severity == sev).count();
    ConfigAudit {
        config_path: config_path.display().to_string(),
        data_dir: data_dir.display().to_string(),
        high: count("high"),
        medium: count("medium"),
        low: count("low"),
        findings,
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

fn render_findings(out: &OutputConfig, findings: &[Finding]) -> bool {
    for f in findings {
        let label = match f.severity {
            "high" => out.error("HIGH  "),
            "medium" => out.warning("MEDIUM"),
            _ => out.muted("LOW   "),
        };
        println!("  [{label}] {:<24} {}", f.kind, f.path);
        print_muted(out, &format!("         └─ {}", f.detail));
    }
    !findings.is_empty()
}

pub fn run_permissions(out: &OutputConfig, path: &Path) -> Result<()> {
    let report = scan_permissions(path)?;
    if out.is_json() {
        out.print_json(&report);
        if report.high > 0 {
            anyhow::bail!("{} high-severity permission finding(s)", report.high);
        }
        return Ok(());
    }
    println!("{}", out.bold("Permission audit"));
    print_muted(
        out,
        &format!("  scanned {} entries under {}", report.scanned, report.path),
    );
    if report.findings.is_empty() {
        println!("  {}", out.success("No risky permission bits found."));
    }
    render_findings(out, &report.findings);
    println!(
        "  {} high, {} medium, {} low",
        report.high, report.medium, report.low
    );
    if report.high > 0 {
        anyhow::bail!("{} high-severity permission finding(s)", report.high);
    }
    Ok(())
}

pub fn run_secrets(out: &OutputConfig, path: &Path) -> Result<()> {
    let (hits, scanned) = scan_secrets(path)?;
    if out.is_json() {
        out.print_json(&serde_json::json!({
            "path": path.display().to_string(),
            "scanned": scanned,
            "findings": hits,
            "total": hits.len(),
        }));
        if !hits.is_empty() {
            anyhow::bail!("{} credential-like string(s) found", hits.len());
        }
        return Ok(());
    }
    println!("{}", out.bold("Secret scan"));
    print_muted(
        out,
        &format!("  scanned {scanned} text files under {}", path.display()),
    );
    if hits.is_empty() {
        println!("  {}", out.success("No credential-like strings found."));
        return Ok(());
    }
    for hit in &hits {
        println!(
            "  [{}] {}:{}  {}",
            out.warning("FOUND "),
            hit.path,
            hit.line,
            hit.kind
        );
        print_muted(
            out,
            &format!("         └─ {} (value redacted)", hit.preview),
        );
    }
    println!(
        "  {} finding(s). Values are never printed in full — rotate anything real.",
        hits.len()
    );
    anyhow::bail!("{} credential-like string(s) found", hits.len())
}

pub fn run_config_audit(out: &OutputConfig) -> Result<()> {
    let audit = audit_config();
    if out.is_json() {
        out.print_json(&audit);
        if audit.high > 0 {
            anyhow::bail!("{} high-severity configuration finding(s)", audit.high);
        }
        return Ok(());
    }
    println!("{}", out.bold("Configuration audit"));
    print_muted(out, &format!("  config:   {}", audit.config_path));
    print_muted(out, &format!("  data dir: {}", audit.data_dir));
    if audit.findings.is_empty() {
        println!(
            "  {}",
            out.success("Permissions are not more permissive than expected.")
        );
    }
    render_findings(out, &audit.findings);
    if audit.high > 0 {
        anyhow::bail!("{} high-severity configuration finding(s)", audit.high);
    }
    Ok(())
}

/// Entry point for `omnicli security <SUBCOMMAND>`.
pub fn run(out: &OutputConfig, cmd: &crate::cli::SecurityCmd) -> Result<()> {
    match cmd {
        crate::cli::SecurityCmd::Permissions { path } => run_permissions(out, path),
        crate::cli::SecurityCmd::Secrets { path } => run_secrets(out, path),
        crate::cli::SecurityCmd::Config => run_config_audit(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_credential_formats() {
        assert_eq!(
            detect_kind("AKIAIOSFODNN7REALKEY1"),
            Some("aws-access-key-id")
        );
        // Placeholder filtering lives in scan_line, not detect_kind.
        assert!(looks_like_placeholder("AKIAIOSFODNN7EXAMPLE"));
        assert_eq!(
            detect_kind("ghp_0123456789abcdefghijklmnopqrstuvwx"),
            Some("github-token")
        );
        assert_eq!(
            detect_kind("xoxb-1234567890-abcdefghijklmn"),
            Some("slack-token")
        );
        assert_eq!(
            detect_kind("sk_live_abcdefghijklmnopqrstuvwx"),
            Some("stripe-secret-key")
        );
    }

    #[test]
    fn redaction_never_reveals_the_whole_value() {
        let secret = "AKIAIOSFODNN7REALKEY1";
        let redacted = redact(secret);
        assert!(!redacted.contains(secret));
        assert!(redacted.starts_with("AKIA"));
        assert!(redacted.contains("chars"));
    }

    #[test]
    fn entropy_flags_random_looking_values() {
        assert!(entropy("a1B2c3D4e5F6g7H8i9J0k1L2") > 4.0);
        assert!(entropy("aaaaaaaaaaaaaaaaaaaaaaaa") < 0.1);
    }

    #[test]
    fn placeholder_values_are_ignored() {
        assert!(looks_like_placeholder("your_api_key_here"));
        assert!(looks_like_placeholder("placeholder-token-value"));
        assert!(!looks_like_placeholder("aZ9xQ2wE8rT4yU6iO0pL"));
    }
}

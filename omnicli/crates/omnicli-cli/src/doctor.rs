//! `omnicli doctor` — environment and data diagnostics.
//!
//! Every check executes real code: the configuration is parsed, the SQLite
//! databases are opened and integrity-checked, the data directory is written to,
//! and external tools are looked up on `PATH`. A check that cannot run is
//! reported as WARN/FAIL with the reason — never as PASS.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use omnicli_core::config::OmniConfig;
use omnicli_core::output::{print_muted, OutputConfig};
use omnicli_core::platform;
use serde::Serialize;

/// Result of a single diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    /// `pass` | `warn` | `fail` | `info`.
    pub status: &'static str,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    fn new(id: &str, status: &'static str, summary: impl Into<String>) -> Self {
        Self {
            id: id.to_owned(),
            status,
            summary: summary.into(),
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: String,
    pub platform: String,
    pub checks: Vec<Check>,
    pub summary: Summary,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub info: usize,
}

pub fn build_report(cfg: &OmniConfig) -> Report {
    let mut checks = Vec::new();

    checks.push(
        Check::new(
            "binary.version",
            "info",
            format!(
                "omnicli {} ({})",
                env!("CARGO_PKG_VERSION"),
                platform::platform_label()
            ),
        )
        .with_detail(format!(
            "executable: {}",
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "<unknown>".to_owned())
        )),
    );

    // ── Data directory ───────────────────────────────────────────────────────
    let data_dir = platform::data_dir();
    checks.push(check_data_dir(&data_dir));

    // ── Workspace database ───────────────────────────────────────────────────
    let workspace_db = platform::expand_tilde(&cfg.workspace.db_path);
    checks.push(check_workspace_db(&workspace_db));

    // ── Search index ─────────────────────────────────────────────────────────
    let index_path = data_dir.join("search.db");
    checks.push(check_search_index(&index_path));

    // ── Configuration ────────────────────────────────────────────────────────
    checks.push(check_config(cfg));

    // ── External tools used by optional features ─────────────────────────────
    checks.extend(check_external_tools());

    // ── Installation ─────────────────────────────────────────────────────────
    checks.push(check_path_installation());

    let summary = Summary {
        pass: checks.iter().filter(|c| c.status == "pass").count(),
        warn: checks.iter().filter(|c| c.status == "warn").count(),
        fail: checks.iter().filter(|c| c.status == "fail").count(),
        info: checks.iter().filter(|c| c.status == "info").count(),
    };

    Report {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: platform::platform_label(),
        checks,
        summary,
    }
}

fn check_data_dir(data_dir: &Path) -> Check {
    if let Err(e) = platform::ensure_dir(data_dir) {
        return Check::new(
            "platform.data_dir",
            "fail",
            format!("cannot create data directory {}", data_dir.display()),
        )
        .with_detail(e.to_string());
    }

    // Prove the directory is writable instead of assuming it.
    let probe = data_dir.join(format!(".doctor-probe-{}", std::process::id()));
    match std::fs::File::create(&probe).and_then(|mut f| f.write_all(b"ok")) {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Check::new(
                "platform.data_dir",
                "pass",
                format!("data directory is writable: {}", data_dir.display()),
            )
        }
        Err(e) => Check::new(
            "platform.data_dir",
            "fail",
            format!("data directory is not writable: {}", data_dir.display()),
        )
        .with_detail(e.to_string()),
    }
}

/// Open a database read-only and run `PRAGMA integrity_check`.
fn integrity_of(path: &Path) -> Result<String, String> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| e.to_string())?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(result)
}

fn check_workspace_db(db_path: &Path) -> Check {
    if !db_path.exists() {
        return Check::new(
            "storage.workspace_db",
            "info",
            format!("not created yet: {}", db_path.display()),
        )
        .with_detail("created automatically on first `omnicli workspace` command".to_owned());
    }

    match omnicli_workspace::open_workspace_db(db_path) {
        Ok(conn) => match omnicli_workspace::workspace_stats(&conn) {
            Ok(stats) => {
                let integrity = integrity_of(db_path).unwrap_or_else(|e| format!("error: {e}"));
                if integrity == "ok" {
                    Check::new(
                        "storage.workspace_db",
                        "pass",
                        format!(
                            "{} notes, {} todos, {} snippets — integrity ok",
                            stats.notes, stats.todos, stats.snippets
                        ),
                    )
                    .with_detail(db_path.display().to_string())
                } else {
                    Check::new(
                        "storage.workspace_db",
                        "fail",
                        format!("integrity check failed: {integrity}"),
                    )
                    .with_detail(db_path.display().to_string())
                }
            }
            Err(e) => Check::new(
                "storage.workspace_db",
                "fail",
                format!("cannot read tables: {e}"),
            )
            .with_detail(db_path.display().to_string()),
        },
        Err(e) => Check::new("storage.workspace_db", "fail", format!("cannot open: {e}"))
            .with_detail(db_path.display().to_string()),
    }
}

fn check_search_index(index_path: &Path) -> Check {
    if !index_path.exists() {
        return Check::new(
            "storage.search_index",
            "info",
            format!("not created yet: {}", index_path.display()),
        )
        .with_detail("build it with `omnicli search index <PATH>`".to_owned());
    }

    match integrity_of(index_path) {
        Ok(v) if v == "ok" => {
            let size = std::fs::metadata(index_path).map(|m| m.len()).unwrap_or(0);
            // Count what the index actually holds: files, content rows, and any
            // duplicate content rows left behind by older versions (which made
            // one file appear several times in search results).
            let counts = rusqlite::Connection::open(index_path).ok().and_then(|c| {
                let files: i64 = c
                    .query_row("SELECT COUNT(*) FROM search_index", [], |r| r.get(0))
                    .ok()?;
                let content: i64 = c
                    .query_row("SELECT COUNT(*) FROM search_content_fts", [], |r| r.get(0))
                    .ok()?;
                let distinct: i64 = c
                    .query_row(
                        "SELECT COUNT(DISTINCT path) FROM search_content_fts",
                        [],
                        |r| r.get(0),
                    )
                    .ok()?;
                Some((files, content, distinct))
            });

            match counts {
                Some((files, content, distinct)) if content > distinct => Check::new(
                    "storage.search_index",
                    "warn",
                    format!(
                        "{files} files; {content} content rows for {distinct} paths — duplicates from an older indexer"
                    ),
                )
                .with_detail(
                    "run `omnicli search index --rebuild <PATH>` to rebuild without duplicates"
                        .to_owned(),
                ),
                Some((files, content, _)) => Check::new(
                    "storage.search_index",
                    "pass",
                    format!(
                        "integrity ok — {files} files, {content} content entries, {}",
                        platform::format_bytes(size)
                    ),
                )
                .with_detail(index_path.display().to_string()),
                None => Check::new(
                    "storage.search_index",
                    "warn",
                    "index opened but its tables could not be read",
                )
                .with_detail("run `omnicli search index --rebuild <PATH>`".to_owned()),
            }
        }
        Ok(v) => Check::new(
            "storage.search_index",
            "fail",
            format!("integrity check failed: {v}"),
        )
        .with_detail(index_path.display().to_string()),
        Err(e) => Check::new("storage.search_index", "fail", format!("cannot open: {e}"))
            .with_detail(index_path.display().to_string()),
    }
}

fn check_config(cfg: &OmniConfig) -> Check {
    let path = platform::config_file_path();
    if !path.exists() {
        return Check::new(
            "config.file",
            "info",
            "no config file — built-in defaults are in use",
        )
        .with_detail(format!("would be {}", path.display()));
    }
    Check::new(
        "config.file",
        "pass",
        format!(
            "{} — output={}, color={}, default_hash={}",
            path.display(),
            cfg.core.output,
            cfg.core.color,
            cfg.file.default_hash
        ),
    )
}

fn which(tool: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(tool);
        candidate.is_file()
    })
}

fn check_external_tools() -> Vec<Check> {
    let required = ["tar"];
    let optional = ["zip", "unzip", "gzip", "age", "rg"];

    let missing_required: Vec<&str> = required.iter().copied().filter(|t| !which(t)).collect();
    let found_optional: Vec<&str> = optional.iter().copied().filter(|t| which(t)).collect();

    let mut checks = vec![if missing_required.is_empty() {
        Check::new(
            "external.required",
            "pass",
            format!("found: {}", required.join(", ")),
        )
    } else {
        Check::new(
            "external.required",
            "warn",
            format!("missing: {}", missing_required.join(", ")),
        )
        .with_detail("tar archives cannot be created or extracted without tar".to_owned())
    }];

    checks.push(Check::new(
        "external.optional",
        "info",
        if found_optional.is_empty() {
            "none of zip, unzip, gzip, age, rg found on PATH".to_owned()
        } else {
            format!("found: {}", found_optional.join(", "))
        },
    ));

    checks
}

fn check_path_installation() -> Check {
    let Ok(exe) = std::env::current_exe() else {
        return Check::new(
            "install.path",
            "info",
            "could not determine the executable path",
        );
    };
    let Some(name) = exe.file_stem().and_then(|s| s.to_str()) else {
        return Check::new("install.path", "info", "unusual executable name");
    };
    if which(name) {
        Check::new("install.path", "pass", format!("`{name}` is on PATH"))
    } else {
        Check::new(
            "install.path",
            "info",
            format!("`{name}` is not on PATH — run it as {}", exe.display()),
        )
    }
}

/// Render the report; a failing check makes the command itself fail (exit 1).
pub fn run(out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    let report = build_report(cfg);

    if out.is_json() {
        out.print_json(&report);
        if report.summary.fail > 0 {
            anyhow::bail!("{} check(s) failed", report.summary.fail);
        }
        return Ok(());
    }

    println!("{}", out.bold("OmniCLI doctor"));
    print_muted(
        out,
        &format!("  omnicli {} on {}", report.version, report.platform),
    );
    println!();

    for check in &report.checks {
        let (label, styled) = match check.status {
            "pass" => ("PASS", out.success("PASS")),
            "warn" => ("WARN", out.warning("WARN")),
            "fail" => ("FAIL", out.error("FAIL")),
            _ => ("INFO", out.muted("INFO")),
        };
        // The styled badge must still contain the plain label: callers strip
        // colours (or pipe output), and the text must survive that unchanged.
        debug_assert!(
            styled.contains(label),
            "styling must not alter the badge text"
        );
        println!("  [{styled}] {:<22} {}", check.id, check.summary);
        if let Some(detail) = &check.detail {
            print_muted(out, &format!("         └─ {detail}"));
        }
    }

    println!();
    println!(
        "  {} passed, {} warning(s), {} failure(s)",
        report.summary.pass, report.summary.warn, report.summary.fail
    );

    if report.summary.fail > 0 {
        anyhow::bail!("{} check(s) failed", report.summary.fail);
    }
    Ok(())
}

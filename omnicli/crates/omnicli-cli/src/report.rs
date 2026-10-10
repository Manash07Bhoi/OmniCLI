//! `omnicli report` — one summary of what the tool knows, in three formats.
//!
//! Every number comes from a live reading: databases are opened, the filesystem
//! is walked, and sizes are measured. Sections that cannot be read are reported
//! as `unavailable` with the reason rather than being silently omitted or
//! zero-filled.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use omnicli_core::config::OmniConfig;
use omnicli_core::output::{print_muted, OutputConfig};
use omnicli_core::platform;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Terminal,
    Json,
    Markdown,
}

impl Format {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "terminal" | "text" | "pretty" => Ok(Format::Terminal),
            "json" => Ok(Format::Json),
            "markdown" | "md" => Ok(Format::Markdown),
            other => {
                anyhow::bail!("Unknown report format '{other}'. Use terminal, json, or markdown.")
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EnvironmentSection {
    pub version: String,
    pub platform: String,
    pub data_dir: String,
    pub config_path: String,
    pub config_file_present: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceSection {
    pub db_path: String,
    pub available: bool,
    pub notes: i64,
    pub todos: i64,
    pub pending_todos: i64,
    pub snippets: i64,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexSection {
    pub db_path: String,
    pub available: bool,
    pub documents: i64,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionCount {
    pub extension: String,
    pub count: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct FilesSection {
    pub path: String,
    pub available: bool,
    pub files: u64,
    pub dirs: u64,
    pub total_size_bytes: u64,
    pub top_extensions: Vec<ExtensionCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub generated_at: String,
    pub environment: EnvironmentSection,
    pub workspace: WorkspaceSection,
    pub search_index: IndexSection,
    pub files: FilesSection,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn workspace_section(cfg: &OmniConfig) -> WorkspaceSection {
    let db_path = platform::expand_tilde(&cfg.workspace.db_path);
    let size_bytes = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    match omnicli_workspace::open_workspace_db(&db_path) {
        Ok(conn) => match omnicli_workspace::workspace_stats(&conn) {
            Ok(stats) => WorkspaceSection {
                db_path: db_path.display().to_string(),
                available: true,
                notes: stats.notes,
                todos: stats.todos,
                pending_todos: stats.todos_pending,
                snippets: stats.snippets,
                size_bytes,
                error: None,
            },
            Err(e) => WorkspaceSection {
                db_path: db_path.display().to_string(),
                available: false,
                notes: 0,
                todos: 0,
                pending_todos: 0,
                snippets: 0,
                size_bytes,
                error: Some(e.to_string()),
            },
        },
        Err(e) => WorkspaceSection {
            db_path: db_path.display().to_string(),
            available: false,
            notes: 0,
            todos: 0,
            pending_todos: 0,
            snippets: 0,
            size_bytes,
            error: Some(e.to_string()),
        },
    }
}

fn index_section() -> IndexSection {
    let db_path = platform::data_dir().join("search.db");
    if !db_path.exists() {
        return IndexSection {
            db_path: db_path.display().to_string(),
            available: false,
            documents: 0,
            size_bytes: 0,
            error: Some("no index yet — build one with `omnicli search index <PATH>`".to_owned()),
        };
    }
    let size_bytes = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    match rusqlite::Connection::open(&db_path).and_then(|c| {
        c.query_row("SELECT COUNT(*) FROM search_index", [], |r| {
            r.get::<_, i64>(0)
        })
    }) {
        Ok(documents) => IndexSection {
            db_path: db_path.display().to_string(),
            available: true,
            documents,
            size_bytes,
            error: None,
        },
        Err(e) => IndexSection {
            db_path: db_path.display().to_string(),
            available: false,
            documents: 0,
            size_bytes,
            error: Some(e.to_string()),
        },
    }
}

fn files_section(root: &Path) -> FilesSection {
    if !root.exists() {
        return FilesSection {
            path: root.display().to_string(),
            available: false,
            files: 0,
            dirs: 0,
            total_size_bytes: 0,
            top_extensions: Vec::new(),
            error: Some("path does not exist".to_owned()),
        };
    }

    let opts = omnicli_file::find::FindOptions {
        path: root.to_path_buf(),
        max_depth: None,
        ..Default::default()
    };
    match omnicli_file::find::find_files(&opts) {
        Ok(entries) => {
            let mut files = 0u64;
            let mut dirs = 0u64;
            let mut total_size_bytes = 0u64;
            let mut per_ext: std::collections::HashMap<String, (u64, u64)> =
                std::collections::HashMap::new();

            for entry in &entries {
                if entry.file_type == "dir" {
                    dirs += 1;
                    continue;
                }
                files += 1;
                total_size_bytes += entry.size_bytes;
                let ext = std::path::Path::new(&entry.path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_else(|| "(none)".to_owned());
                let slot = per_ext.entry(ext).or_insert((0, 0));
                slot.0 += 1;
                slot.1 += entry.size_bytes;
            }

            let mut top_extensions: Vec<ExtensionCount> = per_ext
                .into_iter()
                .map(|(extension, (count, size_bytes))| ExtensionCount {
                    extension,
                    count,
                    size_bytes,
                })
                .collect();
            top_extensions
                .sort_by(|a, b| b.count.cmp(&a.count).then(a.extension.cmp(&b.extension)));
            top_extensions.truncate(5);

            FilesSection {
                path: root.display().to_string(),
                available: true,
                files,
                dirs,
                total_size_bytes,
                top_extensions,
                error: None,
            }
        }
        Err(e) => FilesSection {
            path: root.display().to_string(),
            available: false,
            files: 0,
            dirs: 0,
            total_size_bytes: 0,
            top_extensions: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

pub fn build(cfg: &OmniConfig, path: &Path) -> Report {
    let config_path = platform::config_file_path();
    Report {
        generated_at: now_iso(),
        environment: EnvironmentSection {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: platform::platform_label(),
            data_dir: platform::data_dir().display().to_string(),
            config_file_present: config_path.exists(),
            config_path: config_path.display().to_string(),
        },
        workspace: workspace_section(cfg),
        search_index: index_section(),
        files: files_section(path),
    }
}

fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# OmniCLI report\n\n");
    out.push_str(&format!("Generated: {}\n\n", report.generated_at));

    out.push_str("## Environment\n\n| Item | Value |\n| --- | --- |\n");
    out.push_str(&format!("| Version | {} |\n", report.environment.version));
    out.push_str(&format!("| Platform | {} |\n", report.environment.platform));
    out.push_str(&format!(
        "| Data directory | `{}` |\n",
        report.environment.data_dir
    ));
    out.push_str(&format!(
        "| Config file | `{}` ({}) |\n",
        report.environment.config_path,
        if report.environment.config_file_present {
            "present"
        } else {
            "defaults in use"
        }
    ));

    out.push_str("\n## Workspace\n\n");
    if report.workspace.available {
        out.push_str(&format!(
            "{} notes · {} todos ({} pending) · {} snippets — `{}` ({})\n",
            report.workspace.notes,
            report.workspace.todos,
            report.workspace.pending_todos,
            report.workspace.snippets,
            report.workspace.db_path,
            platform::format_bytes(report.workspace.size_bytes)
        ));
    } else {
        out.push_str(&format!(
            "Unavailable: {}\n",
            report.workspace.error.as_deref().unwrap_or("unknown error")
        ));
    }

    out.push_str("\n## Search index\n\n");
    if report.search_index.available {
        out.push_str(&format!(
            "{} documents — `{}` ({})\n",
            report.search_index.documents,
            report.search_index.db_path,
            platform::format_bytes(report.search_index.size_bytes)
        ));
    } else {
        out.push_str(&format!(
            "Unavailable: {}\n",
            report
                .search_index
                .error
                .as_deref()
                .unwrap_or("unknown error")
        ));
    }

    out.push_str("\n## Files\n\n");
    if report.files.available {
        out.push_str(&format!(
            "`{}`: {} files, {} directories, {}\n\n",
            report.files.path,
            report.files.files,
            report.files.dirs,
            platform::format_bytes(report.files.total_size_bytes)
        ));
        out.push_str("| Extension | Files | Size |\n| --- | --- | --- |\n");
        for ext in &report.files.top_extensions {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                ext.extension,
                ext.count,
                platform::format_bytes(ext.size_bytes)
            ));
        }
    } else {
        out.push_str(&format!(
            "Unavailable: {}\n",
            report.files.error.as_deref().unwrap_or("unknown error")
        ));
    }
    out
}

/// Render the report. It reports state rather than checking it, so it succeeds
/// even when a section is unavailable (the reason is part of the output).
pub fn run(out: &OutputConfig, cfg: &OmniConfig, path: &Path, format: Format) -> Result<()> {
    let report = build(cfg, path);

    match format {
        Format::Json => out.print_json(&report),
        Format::Markdown => println!("{}", markdown(&report)),
        Format::Terminal => {
            println!("{}", out.bold("OmniCLI report"));
            print_muted(out, &format!("  generated {}", report.generated_at));
            println!();
            println!("  {}", out.bold("Environment"));
            print_muted(
                out,
                &format!(
                    "    version    {} ({})",
                    report.environment.version, report.environment.platform
                ),
            );
            print_muted(
                out,
                &format!("    data dir   {}", report.environment.data_dir),
            );
            print_muted(
                out,
                &format!(
                    "    config     {} ({})",
                    report.environment.config_path,
                    if report.environment.config_file_present {
                        "present"
                    } else {
                        "defaults in use"
                    }
                ),
            );

            println!("  {}", out.bold("Workspace"));
            if report.workspace.available {
                print_muted(
                    out,
                    &format!(
                        "    {} notes · {} todos ({} pending) · {} snippets",
                        report.workspace.notes,
                        report.workspace.todos,
                        report.workspace.pending_todos,
                        report.workspace.snippets
                    ),
                );
                print_muted(
                    out,
                    &format!(
                        "    {} ({})",
                        report.workspace.db_path,
                        platform::format_bytes(report.workspace.size_bytes)
                    ),
                );
            } else {
                print_muted(
                    out,
                    &format!(
                        "    unavailable: {}",
                        report.workspace.error.as_deref().unwrap_or("unknown error")
                    ),
                );
            }

            println!("  {}", out.bold("Search index"));
            if report.search_index.available {
                print_muted(
                    out,
                    &format!(
                        "    {} documents ({})",
                        report.search_index.documents,
                        platform::format_bytes(report.search_index.size_bytes)
                    ),
                );
            } else {
                print_muted(
                    out,
                    &format!(
                        "    unavailable: {}",
                        report
                            .search_index
                            .error
                            .as_deref()
                            .unwrap_or("unknown error")
                    ),
                );
            }

            println!("  {}", out.bold("Files"));
            if report.files.available {
                print_muted(
                    out,
                    &format!(
                        "    {} — {} files, {} dirs, {}",
                        report.files.path,
                        report.files.files,
                        report.files.dirs,
                        platform::format_bytes(report.files.total_size_bytes)
                    ),
                );
                for ext in &report.files.top_extensions {
                    print_muted(
                        out,
                        &format!(
                            "      {:<10} {:>6} files  {}",
                            ext.extension,
                            ext.count,
                            platform::format_bytes(ext.size_bytes)
                        ),
                    );
                }
            } else {
                print_muted(
                    out,
                    &format!(
                        "    unavailable: {}",
                        report.files.error.as_deref().unwrap_or("unknown error")
                    ),
                );
            }
        }
    }

    Ok(())
}

/// Default path for the files section when the user does not pass one.
pub fn default_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

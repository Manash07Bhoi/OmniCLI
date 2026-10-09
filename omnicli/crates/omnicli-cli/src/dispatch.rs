//! Command routing: parsed CLI arguments → library calls → rendered output.
//!
//! Rules followed throughout this module:
//! * human-readable payloads go to **stdout**, diagnostics to **stderr**;
//! * `--json` emits exactly one JSON document on stdout, or a single
//!   `{"error": …}` object on stderr when the command fails;
//! * styling is applied through [`OutputConfig`] helpers, never with hardcoded
//!   escape sequences, so `--plain`, `--no-color`, `NO_COLOR`, and `--json` are
//!   always honoured;
//! * every user-supplied path, enum, numeric and duration value is validated
//!   before it reaches the filesystem.

use anyhow::Result;
use omnicli_core::{config::OmniConfig, output::OutputConfig};

use crate::cli::{
    ArchiveCmd, BackupCmd, Commands, ConfigCmd, ConvertCmd, DevCmd, FileCmd, NoteCmd, SearchCmd,
    SnippetCmd, TodoCmd, WorkspaceCmd,
};

/// Errors that map to a specific CLI-level outcome rather than a generic failure.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// `omnicli file compare` found the inputs differ. Exit code 1, like `diff`.
    #[error("files differ")]
    FilesDiffer,
}

/// Route a parsed command to the appropriate module handler.
pub fn dispatch(cmd: Commands, dry_run: bool, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    match cmd {
        Commands::File { cmd } => dispatch_file(cmd, dry_run, out, cfg),
        Commands::Search {
            query,
            limit,
            r#in,
            regex,
            case_sensitive,
            cmd,
        } => {
            // The shorthand `search "<query>" [flags]` becomes the same
            // `SearchCmd::Query` the explicit subcommand parses into.
            let shorthand = query.map(|q| SearchCmd::Query {
                query: q,
                limit: limit.unwrap_or(20),
                r#in,
                regex,
                case_sensitive,
            });
            dispatch_search(shorthand, cmd, out, cfg)
        }
        Commands::Archive { cmd } => dispatch_archive(cmd, out),
        Commands::Convert { cmd } => dispatch_convert(cmd, out),
        Commands::Config { cmd } => dispatch_config(cmd, out, cfg),
        Commands::Dev { cmd } => dispatch_dev(cmd, out),
        Commands::Backup { cmd } => dispatch_backup(cmd, dry_run, out, cfg),
        Commands::Workspace { cmd } => dispatch_workspace(cmd, out, cfg),
        Commands::Doctor => crate::doctor::run(out, cfg),
        Commands::Report { path, format } => {
            // The global `--json` wins over the default, so machine-readable
            // output never depends on remembering two flags.
            let mut format = crate::report::Format::parse(&format)?;
            if out.is_json() {
                format = crate::report::Format::Json;
            }
            let path = path.unwrap_or_else(crate::report::default_path);
            crate::report::run(out, cfg, &path, format)
        }
        Commands::Security { cmd } => crate::security::run(out, &cmd),
        // Completions are handled in main.rs before this function is called.
        Commands::Completions { .. } => Err(anyhow::anyhow!(
            "internal error: completions must be handled before dispatch"
        )),
    }
}

// ── omnicli file ─────────────────────────────────────────────────────────────────

fn dispatch_file(cmd: FileCmd, dry_run: bool, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    use omnicli_core::{
        output::{print_info, print_muted, print_success, print_warning},
        platform::format_bytes,
    };
    use omnicli_file::{
        clean::{clean_path, CleanOptions},
        compare::compare_files,
        copy_move::{copy_path, move_path, CopyOptions},
        duplicate::{delete_duplicates, scan_duplicates},
        find::{find_files, parse_duration, parse_size, EntryType, FindOptions},
        hash::hash_file_cmd,
        sync::{sync_dirs, SyncOptions},
    };

    match cmd {
        FileCmd::Find {
            pattern,
            regex,
            r#type,
            long,
            count,
            size,
            modified,
            path,
            max_depth,
            follow_symlinks,
        } => {
            let entry_type = EntryType::parse(&r#type).map_err(|e| anyhow::anyhow!("{e}"))?;
            let size_filter = size
                .as_deref()
                .map(parse_size)
                .transpose()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let modified_within = modified
                .as_deref()
                .map(parse_duration)
                .transpose()
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let opts = FindOptions {
                pattern,
                regex,
                entry_type,
                size: size_filter,
                modified_within,
                path,
                max_depth,
                follow_symlinks,
                exclude: cfg.search.exclude.clone(),
            };

            let entries = find_files(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;

            if out.is_json() {
                out.print_json(&entries);
            } else if count {
                println!("{}", entries.len());
            } else if long {
                for entry in &entries {
                    let size_str = format_bytes(entry.size_bytes);
                    println!("{:<6}  {:<10}  {}", entry.file_type, size_str, entry.path);
                }
                print_muted(out, &format!("{} entries", entries.len()));
            } else {
                for entry in &entries {
                    println!("{}", entry.path);
                }
                print_muted(out, &format!("{} entries", entries.len()));
            }
        }

        FileCmd::Copy {
            source,
            dest,
            recursive,
            verify,
        } => {
            let opts = CopyOptions {
                source: source.clone(),
                dest: dest.clone(),
                recursive,
                verify,
                dry_run,
            };
            let result = copy_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if dry_run {
                print_success(
                    out,
                    &format!(
                        "Would copy {} → {} ({} file(s), {} bytes)",
                        source.display(),
                        dest.display(),
                        result.files_copied,
                        result.bytes_transferred
                    ),
                );
            } else {
                print_success(
                    out,
                    &format!(
                        "Copied {} → {} ({} file(s), {} bytes)",
                        source.display(),
                        dest.display(),
                        result.files_copied,
                        result.bytes_transferred
                    ),
                );
            }
        }

        FileCmd::Move {
            source,
            dest,
            verify,
        } => {
            let opts = CopyOptions {
                source: source.clone(),
                dest: dest.clone(),
                recursive: true,
                verify,
                dry_run,
            };
            let result = move_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if dry_run {
                print_success(
                    out,
                    &format!(
                        "Would move {} → {} ({} file(s))",
                        source.display(),
                        dest.display(),
                        result.files_copied
                    ),
                );
            } else {
                print_success(
                    out,
                    &format!("Moved {} → {}", source.display(), dest.display()),
                );
            }
        }

        FileCmd::Compare { a, b, hash } => {
            let result = compare_files(&a, &b, hash).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if result.identical {
                print_success(out, "Files are identical.");
            } else {
                print_warning(out, "Files differ.");
                if let Some(off) = result.first_diff_offset {
                    print_info(out, &format!("First difference at byte offset: {off}"));
                }
                print_info(
                    out,
                    &format!(
                        "Size: {} vs {}  Hash-A: {}  Hash-B: {}",
                        result.size_a,
                        result.size_b,
                        result.hash_a.as_deref().unwrap_or("-"),
                        result.hash_b.as_deref().unwrap_or("-"),
                    ),
                );
            }
            // Differing inputs are a failure, mirroring `diff`.
            if !result.identical {
                return Err(CliError::FilesDiffer.into());
            }
        }

        FileCmd::Duplicate {
            path,
            min_size,
            delete_dupes,
            yes,
        } => {
            // `--min-size` is an inclusive lower bound: `1K` and `+1K` both mean
            // "at least 1 KiB". A `-N` (upper-bound) expression is not meaningful
            // here and is rejected rather than silently reinterpreted.
            let min_bytes = match min_size.as_deref() {
                None => 1,
                Some(s) => match parse_size(s).map_err(|e| anyhow::anyhow!("{e}"))? {
                    omnicli_file::find::SizeOp::GreaterThan(n) => n,
                    omnicli_file::find::SizeOp::LessThan(_) => {
                        return Err(anyhow::anyhow!(
                            "--min-size takes a lower bound such as 1K or +50M; \
                             the '-' (smaller than) form is not supported here."
                        ))
                    }
                },
            };
            if min_bytes == 0 {
                return Err(anyhow::anyhow!("--min-size must be at least 1 byte"));
            }
            let result = scan_duplicates(&path, min_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;

            if !delete_dupes {
                if out.is_json() {
                    out.print_json(&result);
                } else if result.is_empty() {
                    print_success(out, "No duplicates found.");
                } else {
                    println!(
                        "{} duplicate group(s), {} redundant file(s), {} wasted:",
                        result.groups.len(),
                        result.total_duplicate_files,
                        format_bytes(result.wasted_bytes)
                    );
                    for group in &result.groups {
                        println!("  {}  {}", out.muted(&group.content_hash), group.size_bytes);
                        for p in &group.files {
                            println!("    {p}");
                        }
                    }
                }
                return Ok(());
            }

            // Deleting duplicates is destructive: require explicit confirmation
            // unless the user asked for a dry run.
            if !yes && !dry_run {
                return Err(anyhow::anyhow!(
                    "--delete-dupes removes files permanently. Re-run with --yes to confirm, \
                     or add --dry-run to preview."
                ));
            }

            let deleted =
                delete_duplicates(&result, dry_run).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({
                    "dry_run": dry_run,
                    "files_deleted": deleted,
                    "bytes_freed": result.wasted_bytes,
                    "groups": result.groups.len(),
                }));
            } else if dry_run {
                print_success(
                    out,
                    &format!(
                        "Would delete {deleted} duplicate file(s), freeing {}.",
                        format_bytes(result.wasted_bytes)
                    ),
                );
            } else {
                print_success(
                    out,
                    &format!(
                        "Deleted {deleted} duplicate file(s), freeing {}.",
                        format_bytes(result.wasted_bytes)
                    ),
                );
            }
        }

        FileCmd::Clean {
            path,
            empty_dirs,
            pattern,
            older_than,
            dry_run: local_dry_run,
        } => {
            let dry_run = dry_run || local_dry_run;
            let older_than = older_than
                .as_deref()
                .map(parse_duration)
                .transpose()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let opts = CleanOptions {
                path: path.clone(),
                older_than,
                pattern,
                empty_dirs,
                dry_run,
            };
            let result = clean_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if out.quiet {
                // quiet mode: report the totals only
                print_success(
                    out,
                    &format!(
                        "{} file(s), {} dir(s) {}, {} freed.",
                        result.files_removed,
                        result.dirs_removed,
                        if dry_run {
                            "would be removed"
                        } else {
                            "removed"
                        },
                        format_bytes(result.bytes_freed),
                    ),
                );
            } else {
                for p in &result.removed {
                    println!("{p}");
                }
                print_success(
                    out,
                    &format!(
                        "{} file(s), {} dir(s) {}, {} freed.",
                        result.files_removed,
                        result.dirs_removed,
                        if dry_run {
                            "would be removed"
                        } else {
                            "removed"
                        },
                        format_bytes(result.bytes_freed),
                    ),
                );
            }
        }

        FileCmd::Hash { path, algo } => {
            let algo_name = algo.unwrap_or_else(|| cfg.file.default_hash.clone());
            let algo_parsed: omnicli_core::hash::HashAlgo =
                algo_name.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
            let result = hash_file_cmd(&path, algo_parsed).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("{}  {}", result.digest, result.path);
                print_muted(
                    out,
                    &format!(
                        "algo: {}  size: {}",
                        result.algorithm,
                        format_bytes(result.size_bytes)
                    ),
                );
            }
        }

        FileCmd::Encrypt {
            path,
            recipient,
            output,
        } => {
            use omnicli_file::encrypt::encrypt_file;
            if !path.is_file() {
                return Err(anyhow::anyhow!(
                    "Not a regular file: {}. Nothing was encrypted.",
                    path.display()
                ));
            }
            let out_path_buf = output.unwrap_or_else(|| {
                let mut s = path.display().to_string();
                s.push_str(".age");
                std::path::PathBuf::from(s)
            });
            let result = encrypt_file(&path, Some(out_path_buf.as_path()), &recipient)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(out, &format!("Encrypted → {}", out_path_buf.display()));
            }
        }

        FileCmd::Decrypt {
            path,
            identity,
            output,
        } => {
            use omnicli_file::decrypt::decrypt_file;
            if !path.is_file() {
                return Err(anyhow::anyhow!(
                    "Not a regular file: {}. Nothing was decrypted.",
                    path.display()
                ));
            }
            // Restrict the default output: stripping ".age" from a filename that
            // does not end in ".age" would collide with the input file.
            let out_path_buf = match output {
                Some(o) => o,
                None => {
                    let s = path.display().to_string();
                    let stripped = s.strip_suffix(".age").ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot infer an output path: {} does not end in .age. \
                             Pass --output <PATH>.",
                            path.display()
                        )
                    })?;
                    std::path::PathBuf::from(stripped)
                }
            };
            if out_path_buf == path {
                return Err(anyhow::anyhow!(
                    "Output path would overwrite the input file {}. Pass --output <PATH>.",
                    path.display()
                ));
            }
            if out_path_buf.exists() {
                return Err(anyhow::anyhow!(
                    "Output path already exists: {}. Pass a different --output <PATH>.",
                    out_path_buf.display()
                ));
            }
            let identity_str = std::fs::read_to_string(&identity).map_err(|e| {
                anyhow::anyhow!("Cannot read identity file {}: {e}", identity.display())
            })?;
            let result = decrypt_file(&path, Some(out_path_buf.as_path()), identity_str.trim())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(out, &format!("Decrypted → {}", out_path_buf.display()));
            }
        }

        FileCmd::Sync {
            source,
            dest,
            delete,
            dry_run: local_dry_run,
        } => {
            let dry_run = dry_run || local_dry_run;
            let opts = SyncOptions {
                source: source.clone(),
                dest: dest.clone(),
                delete_extraneous: delete,
                dry_run,
            };
            let result = sync_dirs(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(
                    out,
                    &format!(
                        "{}: {} added, {} updated, {} deleted — {} {}",
                        if dry_run {
                            "Sync plan"
                        } else {
                            "Sync complete"
                        },
                        result.files_added,
                        result.files_updated,
                        result.files_deleted,
                        format_bytes(result.bytes_transferred),
                        if dry_run {
                            "would transfer"
                        } else {
                            "transferred"
                        },
                    ),
                );
            }
        }

        FileCmd::Stats { path, algo } => {
            let meta = std::fs::metadata(&path)
                .map_err(|e| anyhow::anyhow!("Cannot stat {}: {e}", path.display()))?;
            let size = meta.len();
            let modified: Option<i64> = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);

            let algo_name = algo.unwrap_or_else(|| cfg.file.default_hash.clone());
            let hash_str = if meta.is_file() {
                let algo_parsed: omnicli_core::hash::HashAlgo =
                    algo_name.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
                let r = hash_file_cmd(&path, algo_parsed).map_err(|e| anyhow::anyhow!("{e}"))?;
                Some(r.digest)
            } else {
                // Directories have no content hash; hashing would fail with a
                // confusing I/O error, so report the absence explicitly.
                None
            };

            let stats_val = serde_json::json!({
                "path": path.display().to_string(),
                "file_type": if meta.is_dir() { "dir" } else if meta.is_file() { "file" } else { "other" },
                "size_bytes": size,
                "modified_unix": modified,
                "hash_algorithm": if hash_str.is_some() { Some(algo_name.clone()) } else { None },
                "hash": hash_str,
            });

            if out.is_json() {
                out.print_json(&stats_val);
            } else {
                println!("Path:     {}", path.display());
                print_info(
                    out,
                    &format!(
                        "Type:     {}",
                        stats_val["file_type"].as_str().unwrap_or("other")
                    ),
                );
                print_info(out, &format!("Size:     {}", format_bytes(size)));
                print_info(
                    out,
                    &format!(
                        "Modified: {}",
                        modified
                            .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                            .unwrap_or_else(|| "unknown".to_owned())
                    ),
                );
                match &hash_str {
                    Some(h) => print_info(out, &format!("Hash:     {h}  ({algo_name})")),
                    None => print_info(out, "Hash:     (not applicable for directories)"),
                }
            }
        }
    }
    Ok(())
}

// ── omnicli search ───────────────────────────────────────────────────────────────

fn dispatch_search(
    query: Option<SearchCmd>,
    cmd: Option<SearchCmd>,
    out: &OutputConfig,
    cfg: &OmniConfig,
) -> Result<()> {
    use omnicli_core::{output::print_muted, platform::data_dir};
    use omnicli_search::{
        open_index_db, rebuild_index,
        search::{search_query, ContentFilter, SearchOptions},
    };

    let db_path = data_dir().join("search.db");

    // `omnicli search "<query>" [flags]` is shorthand for
    // `omnicli search query "<query>"` with the same flags; `query` already
    // carries them as a `SearchCmd::Query`.
    let cmd = match (query, cmd) {
        (Some(q), None) => q,
        (None, Some(c)) => c,
        (Some(_), Some(_)) => {
            return Err(anyhow::anyhow!(
                "Pass either a bare query (`omnicli search \"text\"`) or a subcommand, not both."
            ))
        }
        (None, None) => {
            return Err(anyhow::anyhow!(
                "No query given. Use `omnicli search \"text\"` or `omnicli search index <PATH>`."
            ))
        }
    };

    match cmd {
        SearchCmd::Query {
            query,
            limit,
            r#in,
            regex,
            case_sensitive,
        } => {
            if query.trim().is_empty() {
                return Err(anyhow::anyhow!("Search query must not be empty."));
            }
            if limit == 0 {
                return Err(anyhow::anyhow!(
                    "--limit must be at least 1 (got 0). Use `--limit 1` to fetch a single result."
                ));
            }
            let conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let content_filter = match r#in.as_deref() {
                Some(s) => ContentFilter::parse_strict(s).map_err(|e| anyhow::anyhow!("{e}"))?,
                None => ContentFilter::all(),
            };
            let opts = SearchOptions {
                query: query.clone(),
                content_filter,
                use_regex: regex,
                case_sensitive,
                limit,
            };
            let results = search_query(&conn, &opts).map_err(|e| anyhow::anyhow!("{e}"))?;

            if out.is_json() {
                out.print_json(&results);
            } else if results.is_empty() {
                print_muted(out, "No results.");
            } else {
                println!("{} result(s) for '{query}':", results.len());
                for r in &results {
                    println!("  {}", out.accent(&r.path));
                    if let Some(s) = &r.snippet {
                        println!(
                            "    {}",
                            out.muted(&s.chars().take(120).collect::<String>())
                        );
                    }
                }
            }
        }

        SearchCmd::Index {
            paths,
            rebuild,
            exclude,
        } => {
            // Explicit paths win; otherwise fall back to `search.index_paths`
            // from the config file, so that setting is real rather than
            // documentation.
            let paths: Vec<std::path::PathBuf> = if paths.is_empty() {
                cfg.search
                    .index_paths
                    .iter()
                    .map(|p| omnicli_core::platform::expand_tilde(p))
                    .collect()
            } else {
                paths.clone()
            };
            if paths.is_empty() {
                return Err(anyhow::anyhow!(
                    "No paths to index. Pass PATH or set `search.index_paths` in the config file."
                ));
            }
            let mut excludes = cfg.search.exclude.clone();
            excludes.extend(exclude);

            let mut conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let stats = rebuild_index(
                &mut conn,
                &paths,
                &excludes,
                rebuild,
                out.quiet || out.is_json(),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&stats);
            } else {
                println!(
                    "Indexed {} file(s), {} content doc(s), {} skipped.",
                    stats.files_indexed, stats.content_docs_indexed, stats.files_skipped
                );
            }
        }

        SearchCmd::Info => {
            if !db_path.exists() {
                if out.is_json() {
                    out.print_json(&serde_json::json!({
                        "db_path": db_path.display().to_string(),
                        "total_entries": 0,
                        "status": "empty",
                    }));
                } else {
                    print_muted(
                        out,
                        "No index found. Run `omnicli search index <path>` first.",
                    );
                }
                return Ok(());
            }
            let conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM search_index", [], |r| r.get(0))
                .map_err(|e| anyhow::anyhow!("Cannot read the search index: {e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({
                    "db_path": db_path.display().to_string(),
                    "total_entries": count,
                    "status": if count == 0 { "empty" } else { "ready" },
                }));
            } else {
                println!("Index path:    {}", db_path.display());
                println!("Total entries: {count}");
            }
        }
    }
    Ok(())
}

// ── omnicli archive ──────────────────────────────────────────────────────────────

fn dispatch_archive(cmd: ArchiveCmd, out: &OutputConfig) -> Result<()> {
    use omnicli_archive::{create_archive, extract_archive, list_archive};
    use omnicli_core::{
        output::{print_muted, print_warning},
        platform::format_bytes,
    };

    match cmd {
        ArchiveCmd::Create { output, inputs } => {
            for input in &inputs {
                if !input.exists() {
                    return Err(anyhow::anyhow!(
                        "Input does not exist: {}. Nothing was written.",
                        input.display()
                    ));
                }
            }
            let result = create_archive(&output, &inputs).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!(
                    "Created {} ({}, {} file(s))",
                    output.display(),
                    format_bytes(result.archive_size_bytes),
                    result.files_added
                );
            }
        }
        ArchiveCmd::Extract { archive, to } => {
            let result =
                extract_archive(&archive, to.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                for w in &result.warnings {
                    print_warning(out, w);
                }
                println!(
                    "Extracted {} file(s) to {}",
                    result.files_extracted, result.dest
                );
                if result.entries_skipped > 0 {
                    print_warning(
                        out,
                        &format!(
                            "{} entr(ies) skipped — see warnings above",
                            result.entries_skipped
                        ),
                    );
                }
            }
        }
        ArchiveCmd::List { archive } => {
            let entries = list_archive(&archive).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&entries);
            } else {
                for e in &entries {
                    let size = format_bytes(e.size_bytes);
                    println!("{:<12}  {}", size, e.name);
                }
                print_muted(out, &format!("{} entries", entries.len()));
            }
        }
        ArchiveCmd::Convert { input, output } => {
            // Re-archive: extract to a temporary directory, then pack the new format.
            let tmp = tempfile::tempdir()?;
            let _ex =
                extract_archive(&input, Some(tmp.path())).map_err(|e| anyhow::anyhow!("{e}"))?;
            let entries: Vec<std::path::PathBuf> = std::fs::read_dir(tmp.path())?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .collect();
            if entries.is_empty() {
                return Err(anyhow::anyhow!(
                    "{} contains no entries; nothing to convert.",
                    input.display()
                ));
            }
            let result = create_archive(&output, &entries).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({
                    "input": input.display().to_string(),
                    "output": output.display().to_string(),
                    "files": result.files_added
                }));
            } else {
                println!(
                    "{} → {} ({} file(s))",
                    input.display(),
                    output.display(),
                    result.files_added
                );
            }
        }
    }
    Ok(())
}

// ── omnicli convert ──────────────────────────────────────────────────────────────

fn dispatch_convert(cmd: ConvertCmd, out: &OutputConfig) -> Result<()> {
    use omnicli_convert::{convert, list_supported_pairs};

    match cmd {
        ConvertCmd::Run { input, output } => {
            if !input.is_file() {
                return Err(anyhow::anyhow!(
                    "Input is not a regular file: {}. Nothing was converted.",
                    input.display()
                ));
            }
            let result = convert(&input, &output).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!(
                    "{} → {} ({} bytes)",
                    result.from_format, result.to_format, result.bytes_written
                );
            }
        }
        ConvertCmd::List => {
            let pairs = list_supported_pairs();
            if out.is_json() {
                out.print_json(&pairs);
            } else {
                println!("Supported conversion pairs:");
                for p in &pairs {
                    println!("  {:>8} → {:<8}  {}", p.from, p.to, p.description);
                }
            }
        }
    }
    Ok(())
}

// ── omnicli config ───────────────────────────────────────────────────────────────

fn dispatch_config(cmd: ConfigCmd, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    use omnicli_core::output::{print_error, print_success};

    match cmd {
        ConfigCmd::Show => {
            if out.is_json() {
                out.print_json(cfg);
            } else {
                let rendered = toml::to_string_pretty(cfg)
                    .unwrap_or_else(|_| serde_json::to_string_pretty(cfg).unwrap_or_default());
                println!("{rendered}");
            }
        }
        ConfigCmd::Path => {
            let p = omnicli_core::platform::config_file_path();
            if out.is_json() {
                out.print_json(&serde_json::json!({ "path": p.display().to_string() }));
            } else {
                println!("{}", p.display());
            }
        }
        ConfigCmd::Read { path } => {
            let result = omnicli_config::show_config(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                let pretty = serde_json::to_string_pretty(&result.content).unwrap_or_default();
                println!("{pretty}");
            }
        }
        ConfigCmd::Get { path, key } => {
            let val = omnicli_config::get_key(&path, &key).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&val);
            } else {
                let s = match &val {
                    serde_json::Value::String(s) => s.clone(),
                    other => serde_json::to_string_pretty(other).unwrap_or_default(),
                };
                println!("{s}");
            }
        }
        ConfigCmd::Set { path, key, value } => {
            omnicli_config::set_key(&path, &key, &value).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({ "key": key, "value": value, "ok": true }));
            } else {
                print_success(out, &format!("Set {key} = {value}"));
            }
        }
        ConfigCmd::Validate { path } => {
            let result = omnicli_config::validate_config(&path);
            let detail = result.error.clone().unwrap_or_default();
            if out.is_json() {
                out.print_json(&result);
            } else if result.valid {
                println!("{} is valid {} ✓", path.display(), result.format);
            } else {
                print_error(out, &format!("{}: {detail}", path.display()));
            }
            // An invalid file is a failure whether or not JSON was requested.
            if !result.valid {
                return Err(anyhow::anyhow!("{} is not valid: {detail}", path.display()));
            }
        }
    }
    Ok(())
}

// ── omnicli dev ──────────────────────────────────────────────────────────────────

fn dispatch_dev(cmd: DevCmd, out: &OutputConfig) -> Result<()> {
    use omnicli_core::output::print_muted;
    use std::io::Read;

    /// Read the whole of stdin. Fails, rather than exiting with an empty
    /// success payload, when stdin is a TTY (the user forgot to pipe anything).
    fn stdin_input() -> Result<String> {
        if omnicli_core::platform::is_tty() {
            return Err(anyhow::anyhow!(
                "No input given and stdin is a terminal. Pass the value as an argument or pipe it in."
            ));
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| anyhow::anyhow!("Cannot read stdin: {e}"))?;
        Ok(buf.trim_end().to_owned())
    }

    fn stdin_or(arg: Option<String>) -> Result<String> {
        match arg {
            Some(s) => Ok(s),
            None => stdin_input(),
        }
    }

    match cmd {
        DevCmd::Hash { input, algo } => {
            let text = stdin_or(input)?;
            let result =
                omnicli_dev::compute_hash(&text, &algo).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("{}", out.accent(&result.digest));
                print_muted(
                    out,
                    &format!("algo: {}  input: {} bytes", result.algo, result.input_len),
                );
            }
        }

        DevCmd::Json {
            input,
            action,
            query,
        } => {
            let text = stdin_or(input)?;
            let result = omnicli_dev::process_json(&text, &action, query.as_deref())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if result.valid {
                println!("{}", result.output);
            } else {
                let msg = result.error.unwrap_or_default();
                eprintln!("Invalid JSON: {msg}");
                return Err(anyhow::anyhow!("Invalid JSON input"));
            }
        }

        DevCmd::Base64 { input, decode } => {
            let text = stdin_or(input)?;
            let result =
                omnicli_dev::process_base64(&text, decode).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("{}", result.output);
            }
        }

        DevCmd::Uuid { count, ver } => {
            let result =
                omnicli_dev::generate_uuids(count, &ver).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                for u in &result.uuids {
                    println!("{u}");
                }
            }
        }

        DevCmd::Regex {
            pattern,
            text,
            flags,
        } => {
            let input = stdin_or(text)?;
            let result = omnicli_dev::test_regex(&pattern, &input, &flags)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if !result.valid {
                eprintln!("Invalid regex: {}", result.error.unwrap_or_default());
                return Err(anyhow::anyhow!("Invalid regex"));
            } else {
                print_muted(out, &format!("{} match(es)", result.match_count));
                for m in &result.matches {
                    println!("  [{:>5}–{:<5}] {}", m.start, m.end, out.warning(&m.text));
                }
            }
        }

        DevCmd::Jwt { token } => {
            use omnicli_core::output::{print_success, print_warning};
            let text = stdin_or(token)?;
            let result = omnicli_dev::decode_jwt(&text).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!(
                    "{}",
                    out.muted("── Header ─────────────────────────────────────")
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result.header).unwrap_or_default()
                );
                println!(
                    "{}",
                    out.muted("── Payload ────────────────────────────────────")
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result.payload).unwrap_or_default()
                );
                if let Some(expired) = result.is_expired {
                    if expired {
                        print_warning(
                            out,
                            &format!("EXPIRED at {}", result.expires_at.unwrap_or_default()),
                        );
                    } else {
                        print_success(
                            out,
                            &format!("Valid until {}", result.expires_at.unwrap_or_default()),
                        );
                    }
                }
                if let Some(alg) = result.header.get("alg").and_then(|v| v.as_str()) {
                    print_muted(
                        out,
                        &format!("algorithm: {alg} — signature is NOT verified by this command"),
                    );
                }
            }
        }
    }
    Ok(())
}

// ── omnicli backup ───────────────────────────────────────────────────────────────

fn dispatch_backup(
    cmd: BackupCmd,
    dry_run: bool,
    out: &OutputConfig,
    cfg: &OmniConfig,
) -> Result<()> {
    use omnicli_backup::{backup_create, backup_restore, backup_verify, list_snapshots};
    use omnicli_core::{
        output::{print_error, print_muted, print_success, print_warning},
        platform::{expand_tilde, format_bytes},
    };

    match cmd {
        BackupCmd::Create { source, dest, name } => {
            let dest = match dest {
                Some(d) => d,
                None => expand_tilde(&cfg.backup.default_dest),
            };
            if dry_run {
                if !source.exists() {
                    return Err(anyhow::anyhow!(
                        "Source does not exist: {}",
                        source.display()
                    ));
                }
                let payload = serde_json::json!({
                    "dry_run": true,
                    "source": source.display().to_string(),
                    "dest": dest.display().to_string(),
                    "name": name,
                });
                if out.is_json() {
                    out.print_json(&payload);
                } else {
                    print_success(
                        out,
                        &format!(
                            "Would back up {} → {} (job '{name}')",
                            source.display(),
                            dest.display()
                        ),
                    );
                }
                return Ok(());
            }

            let result = backup_create(&source, &dest, &name, out.quiet || out.is_json())
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            // `backup.verify_after_create` re-hashes every object written so a
            // silent storage fault is caught at write time, not at restore time.
            let verification = if cfg.backup.verify_after_create {
                match backup_verify(&dest, &result.snapshot_id) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        print_warning(out, &format!("post-create verification could not run: {e}"));
                        None
                    }
                }
            } else {
                None
            };

            if out.is_json() {
                let mut value = serde_json::to_value(&result).unwrap_or_default();
                if let Some(v) = &verification {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert(
                            "verified".to_owned(),
                            serde_json::json!({
                                "passed": v.passed,
                                "files_checked": v.files_checked,
                                "files_missing": v.files_missing,
                                "files_corrupt": v.files_corrupt,
                            }),
                        );
                    }
                }
                out.print_json(&value);
            } else {
                print_success(out, &format!("Snapshot: {}", result.snapshot_id));
                print_muted(
                    out,
                    &format!(
                        "  {} total — {} new, {} unchanged — {} in {}ms",
                        result.files_total,
                        result.files_new,
                        result.files_unchanged,
                        format_bytes(result.bytes_transferred),
                        result.duration_ms
                    ),
                );
                match verification {
                    Some(v) if v.passed => {
                        print_success(out, &format!("Verified {} object(s).", v.files_checked))
                    }
                    Some(v) => {
                        print_error(
                            out,
                            &format!(
                                "Verification FAILED: {} missing, {} corrupt of {} object(s).",
                                v.files_missing, v.files_corrupt, v.files_checked
                            ),
                        );
                        return Err(anyhow::anyhow!(
                            "backup snapshot {} failed verification",
                            result.snapshot_id
                        ));
                    }
                    None => {}
                }
            }
        }
        BackupCmd::Restore {
            backup_dir,
            snapshot_id,
            to,
        } => {
            let result = backup_restore(&backup_dir, &snapshot_id, &to)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(
                    out,
                    &format!(
                        "Restored {} file(s) ({}) to {}",
                        result.files_restored,
                        format_bytes(result.bytes_restored),
                        result.target_path
                    ),
                );
            }
        }
        BackupCmd::Verify {
            backup_dir,
            snapshot_id,
        } => {
            let result =
                backup_verify(&backup_dir, &snapshot_id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if result.passed {
                print_success(
                    out,
                    &format!("All {} file(s) verified — intact.", result.files_checked),
                );
            } else {
                print_error(
                    out,
                    &format!(
                        "FAILED: {} missing, {} corrupt / {} total.",
                        result.files_missing, result.files_corrupt, result.files_checked
                    ),
                );
                for e in result.entries.iter().filter(|e| e.status != "ok") {
                    eprintln!("  [{:>7}] {}", e.status.to_uppercase(), e.rel_path);
                }
                return Err(anyhow::anyhow!(
                    "backup snapshot {snapshot_id} failed verification"
                ));
            }
        }
        BackupCmd::List { backup_dir } => {
            let snaps = list_snapshots(&backup_dir).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&snaps);
            } else if snaps.is_empty() {
                print_muted(out, "No snapshots found.");
            } else {
                println!("{} snapshot(s):", snaps.len());
                for s in &snaps {
                    println!(
                        "  {}  {}  {} file(s)  ← {}",
                        out.accent(&s.snapshot_id),
                        s.created_at,
                        s.file_count,
                        s.source_path
                    );
                }
            }
        }
    }
    Ok(())
}

// ── omnicli workspace ────────────────────────────────────────────────────────────

fn dispatch_workspace(cmd: WorkspaceCmd, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    use omnicli_core::output::print_muted;
    use omnicli_workspace::open_workspace_db;

    // `workspace.db_path` from the config file is authoritative when set.
    let db_path = omnicli_core::platform::expand_tilde(&cfg.workspace.db_path);
    let conn = open_workspace_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;

    match cmd {
        WorkspaceCmd::Note { cmd } => dispatch_note(cmd, &conn, out)?,
        WorkspaceCmd::Todo { cmd } => dispatch_todo(cmd, &conn, out)?,
        WorkspaceCmd::Snippet { cmd } => dispatch_snippet(cmd, &conn, out)?,
        WorkspaceCmd::Stats => {
            let stats =
                omnicli_workspace::workspace_stats(&conn).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                // `db_path` is the authoritative store location: it honours a
                // configured `workspace.db_path`, so consumers never need to
                // re-derive it.
                out.print_json(&serde_json::json!({
                    "notes": stats.notes,
                    "todos": stats.todos,
                    "todos_pending": stats.todos_pending,
                    "snippets": stats.snippets,
                    "db_path": db_path.display().to_string(),
                }));
            } else {
                println!("Workspace database: {}", db_path.display());
                print_muted(out, &format!("  Notes:    {}", stats.notes));
                print_muted(out, &format!("  Todos:    {}", stats.todos));
                print_muted(out, &format!("  Pending:  {}", stats.todos_pending));
                print_muted(out, &format!("  Snippets: {}", stats.snippets));
            }
        }
    }
    Ok(())
}

fn dispatch_note(cmd: NoteCmd, conn: &rusqlite::Connection, out: &OutputConfig) -> Result<()> {
    use omnicli_core::output::{print_muted, print_success};
    use omnicli_workspace::{create_note, delete_note, get_note, list_notes};

    match cmd {
        NoteCmd::List { search } => {
            let notes = list_notes(conn, search.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&notes);
            } else if notes.is_empty() {
                print_muted(
                    out,
                    "No notes yet. Create one with: omnicli workspace note new \"Title\"",
                );
            } else {
                for n in &notes {
                    println!("  {}  {}", out.accent(&format!("#{:<4}", n.id)), n.title);
                }
            }
        }
        NoteCmd::New { title, body, tags } => {
            if title.trim().is_empty() {
                return Err(anyhow::anyhow!("Note title must not be empty."));
            }
            let n = create_note(conn, &title, body.as_deref().unwrap_or(""), tags.as_deref())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&n);
            } else {
                print_success(out, &format!("Note #{} created: {}", n.id, n.title));
            }
        }
        NoteCmd::Show { id } => {
            let n = get_note(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&n);
            } else {
                println!("{}", out.bold(&n.title));
                if let Some(t) = &n.tags {
                    println!("{}", out.muted(&format!("tags: {t}")));
                }
                println!("{}", n.body);
            }
        }
        NoteCmd::Delete { id } => {
            delete_note(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({ "deleted": id }));
            } else {
                print_success(out, &format!("Note #{id} deleted."));
            }
        }
    }
    Ok(())
}

fn dispatch_todo(cmd: TodoCmd, conn: &rusqlite::Connection, out: &OutputConfig) -> Result<()> {
    use omnicli_core::output::{print_muted, print_success};
    use omnicli_workspace::{create_todo, delete_todo, list_todos, toggle_todo};

    match cmd {
        TodoCmd::List { done } => {
            let todos = list_todos(conn, done).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&todos);
            } else if todos.is_empty() {
                print_muted(
                    out,
                    "No todos yet. Add one with: omnicli workspace todo add \"Description\"",
                );
            } else {
                for t in &todos {
                    let mark = if t.done {
                        out.success("✓")
                    } else {
                        out.muted("○")
                    };
                    println!(
                        "  {mark} {}  {}",
                        out.accent(&format!("#{:<4}", t.id)),
                        t.description
                    );
                }
            }
        }
        TodoCmd::Add { description, due } => {
            if description.trim().is_empty() {
                return Err(anyhow::anyhow!("Todo description must not be empty."));
            }
            let t = create_todo(conn, &description, due).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&t);
            } else {
                print_success(out, &format!("Todo #{} added.", t.id));
            }
        }
        TodoCmd::Toggle { id } => {
            let t = toggle_todo(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&t);
            } else {
                let state = if t.done { "done" } else { "pending" };
                print_success(out, &format!("Todo #{id} marked {state}."));
            }
        }
        TodoCmd::Delete { id } => {
            delete_todo(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({ "deleted": id }));
            } else {
                print_success(out, &format!("Todo #{id} deleted."));
            }
        }
    }
    Ok(())
}

fn dispatch_snippet(
    cmd: SnippetCmd,
    conn: &rusqlite::Connection,
    out: &OutputConfig,
) -> Result<()> {
    use omnicli_core::output::{print_muted, print_success};
    use omnicli_workspace::{create_snippet, delete_snippet, get_snippet, list_snippets};

    match cmd {
        SnippetCmd::List { lang } => {
            let snips = list_snippets(conn, lang.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&snips);
            } else if snips.is_empty() {
                print_muted(
                    out,
                    "No snippets yet. Save one with: omnicli workspace snippet save NAME \"body\"",
                );
            } else {
                for s in &snips {
                    let language = s.language.as_deref().unwrap_or("-");
                    println!(
                        "  {}  {:<8}  {}",
                        out.accent(&format!("#{:<4}", s.id)),
                        language,
                        s.name
                    );
                }
            }
        }
        SnippetCmd::Save { name, lang, body } => {
            if name.trim().is_empty() {
                return Err(anyhow::anyhow!("Snippet name must not be empty."));
            }
            if body.is_empty() {
                return Err(anyhow::anyhow!("Snippet body must not be empty."));
            }
            let s = create_snippet(conn, &name, lang.as_deref(), &body)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&s);
            } else {
                print_success(out, &format!("Snippet #{} saved: {}", s.id, s.name));
            }
        }
        SnippetCmd::Show { id } => {
            let s = get_snippet(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&s);
            } else {
                println!(
                    "{}  {}",
                    out.bold(&s.name),
                    out.muted(s.language.as_deref().unwrap_or(""))
                );
                println!("{}", s.body);
            }
        }
        SnippetCmd::Delete { id } => {
            delete_snippet(conn, id).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({ "deleted": id }));
            } else {
                print_success(out, &format!("Snippet #{id} deleted."));
            }
        }
    }
    Ok(())
}

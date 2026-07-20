use anyhow::Result;
use omni_core::{config::OmniConfig, output::OutputConfig};

use crate::cli::{
    ArchiveCmd, BackupCmd, Commands, ConfigCmd, ConvertCmd, DevCmd, FileCmd, NoteCmd, SearchCmd,
    SnippetCmd, TodoCmd, WorkspaceCmd,
};

// Completions are handled in main.rs before dispatch is called.

/// Route a parsed command to the appropriate module handler.
pub fn dispatch(cmd: Commands, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    match cmd {
        Commands::File { cmd } => dispatch_file(cmd, out, cfg),
        Commands::Search { cmd } => dispatch_search(cmd, out),
        Commands::Archive { cmd } => dispatch_archive(cmd, out),
        Commands::Convert { cmd } => dispatch_convert(cmd, out),
        Commands::Config { cmd } => dispatch_config(cmd, out, cfg),
        Commands::Dev { cmd } => dispatch_dev(cmd, out),
        Commands::Backup { cmd } => dispatch_backup(cmd, out),
        Commands::Workspace { cmd } => dispatch_workspace(cmd, out),
        // Completions are handled in main.rs before this function is called.
        Commands::Completions { .. } => unreachable!("completions handled in main"),
    }
}

// ── omni file ─────────────────────────────────────────────────────────────────

fn dispatch_file(cmd: FileCmd, out: &OutputConfig, _cfg: &OmniConfig) -> Result<()> {
    use omni_core::{
        output::{print_info, print_muted, print_success, print_warning},
        platform::format_bytes,
    };
    use omni_file::{
        clean::{clean_path, CleanOptions},
        compare::compare_files,
        copy_move::{copy_path, move_path, CopyOptions},
        duplicate::scan_duplicates,
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
                path: path.clone(),
                max_depth,
                follow_symlinks,
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
                dry_run: false,
            };
            let result = copy_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(
                    out,
                    &format!(
                        "Copied {} → {} ({} bytes)",
                        source.display(),
                        dest.display(),
                        result.bytes_transferred
                    ),
                );
            }
        }

        FileCmd::Move { source, dest } => {
            let opts = CopyOptions {
                source: source.clone(),
                dest: dest.clone(),
                recursive: false,
                verify: false,
                dry_run: false,
            };
            let result = move_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
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
        }

        FileCmd::Duplicate { path, min_size: _ } => {
            let result = scan_duplicates(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if result.groups.is_empty() {
                print_success(out, "No duplicates found.");
            } else {
                println!(
                    "{} duplicate group(s)  {}  wasted:",
                    result.groups.len(),
                    format_bytes(result.wasted_bytes)
                );
                for group in &result.groups {
                    print_muted(out, &format!("  hash: {}", &group.content_hash[..12]));
                    for p in &group.files {
                        println!("    {p}");
                    }
                }
            }
        }

        FileCmd::Clean {
            path,
            empty_dirs,
            pattern: _,
            dry_run,
        } => {
            let opts = CleanOptions {
                path,
                older_than: None,
                empty_dirs,
                dry_run,
            };
            let result = clean_path(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(
                    out,
                    &format!(
                        "{} file(s), {} dir(s) {}. {} freed.",
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
            let algo_parsed: omni_core::hash::HashAlgo =
                algo.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
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
            use omni_file::encrypt::encrypt_file;
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
            use omni_file::decrypt::decrypt_file;
            let out_path_buf = output.unwrap_or_else(|| {
                let s = path.display().to_string();
                std::path::PathBuf::from(s.trim_end_matches(".age"))
            });
            // Read the identity key from the file
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
        } => {
            let opts = SyncOptions {
                source: source.clone(),
                dest: dest.clone(),
                delete_extraneous: delete,
                dry_run: false,
            };
            let result = sync_dirs(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                print_success(
                    out,
                    &format!(
                        "Sync complete: {} added, {} updated, {} deleted — {} transferred",
                        result.files_added,
                        result.files_updated,
                        result.files_deleted,
                        format_bytes(result.bytes_transferred),
                    ),
                );
            }
        }

        FileCmd::Stats { path, algo } => {
            let meta = std::fs::metadata(&path)
                .map_err(|e| anyhow::anyhow!("Cannot stat {}: {e}", path.display()))?;
            let size = meta.len();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    let dt =
                        chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default();
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                })
                .unwrap_or_default();

            let hash_str = if let Some(algo_str) = algo {
                let algo_parsed: omni_core::hash::HashAlgo =
                    algo_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
                let r = hash_file_cmd(&path, algo_parsed).map_err(|e| anyhow::anyhow!("{e}"))?;
                Some(r.digest)
            } else {
                None
            };

            let stats_val = serde_json::json!({
                "path": path.display().to_string(),
                "size_bytes": size,
                "modified": mtime,
                "hash": hash_str,
            });

            if out.is_json() {
                out.print_json(&stats_val);
            } else {
                println!("Path:     {}", path.display());
                print_info(out, &format!("Size:     {}", format_bytes(size)));
                print_info(out, &format!("Modified: {mtime}"));
                if let Some(h) = &hash_str {
                    print_info(out, &format!("Hash:     {h}"));
                }
            }
        }
    }
    Ok(())
}

// ── omni search ───────────────────────────────────────────────────────────────

fn dispatch_search(cmd: SearchCmd, out: &OutputConfig) -> Result<()> {
    use omni_core::{output::print_muted, platform::data_dir};
    use omni_search::{
        open_index_db, rebuild_index,
        search::{search_query, ContentFilter, SearchOptions},
    };

    let db_path = data_dir().join("search.db");

    match cmd {
        SearchCmd::Query {
            query,
            limit,
            r#in,
            regex,
        } => {
            let conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let content_filter = r#in
                .as_deref()
                .map(ContentFilter::parse)
                .unwrap_or_else(ContentFilter::all);
            let opts = SearchOptions {
                query: query.clone(),
                content_filter,
                use_regex: regex,
                case_sensitive: false,
                limit,
            };
            let results = search_query(&conn, &opts).map_err(|e| anyhow::anyhow!("{e}"))?;

            // For rebuild_index we need &mut — here just query
            drop(conn);

            if out.is_json() {
                out.print_json(&results);
            } else if results.is_empty() {
                print_muted(out, "No results.");
            } else {
                println!("{} result(s) for '{query}':", results.len());
                for r in &results {
                    println!("  \x1b[96m{}\x1b[0m", r.path);
                    if let Some(s) = &r.snippet {
                        println!(
                            "    \x1b[90m{}\x1b[0m",
                            s.chars().take(120).collect::<String>()
                        );
                    }
                }
            }
        }
        SearchCmd::Index { path, rebuild } => {
            let mut conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let stats = rebuild_index(&mut conn, &[path], &[], rebuild)
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
                print_muted(out, "No index found. Run `omni search index <path>` first.");
                return Ok(());
            }
            let conn = open_index_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM search_index", [], |r| r.get(0))
                .unwrap_or(0);
            if out.is_json() {
                out.print_json(&serde_json::json!({
                    "db_path": db_path.display().to_string(),
                    "total_entries": count
                }));
            } else {
                println!("Index path:    {}", db_path.display());
                println!("Total entries: {count}");
            }
        }
    }
    Ok(())
}

// ── omni archive ──────────────────────────────────────────────────────────────

fn dispatch_archive(cmd: ArchiveCmd, out: &OutputConfig) -> Result<()> {
    use omni_archive::{create_archive, extract_archive, list_archive};
    use omni_core::{output::print_muted, platform::format_bytes};

    match cmd {
        ArchiveCmd::Create { output, inputs } => {
            let result = create_archive(&output, &inputs).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!(
                    "Created {} ({} files)",
                    output.display(),
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
                println!(
                    "Extracted {} file(s) to {}",
                    result.files_extracted, result.dest
                );
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
            // Re-archive: extract to temp dir, then pack into new format
            let tmp = tempfile::tempdir()?;
            let _ex =
                extract_archive(&input, Some(tmp.path())).map_err(|e| anyhow::anyhow!("{e}"))?;
            let result = create_archive(&output, &[tmp.path().to_path_buf()])
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({
                    "input": input.display().to_string(),
                    "output": output.display().to_string(),
                    "files": result.files_added
                }));
            } else {
                println!(
                    "{} → {} ({} files)",
                    input.display(),
                    output.display(),
                    result.files_added
                );
            }
        }
    }
    Ok(())
}

// ── omni convert ──────────────────────────────────────────────────────────────

fn dispatch_convert(cmd: ConvertCmd, out: &OutputConfig) -> Result<()> {
    use omni_convert::{convert, list_supported_pairs};

    match cmd {
        ConvertCmd::Run { input, output } => {
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

// ── omni config ───────────────────────────────────────────────────────────────

fn dispatch_config(cmd: ConfigCmd, out: &OutputConfig, cfg: &OmniConfig) -> Result<()> {
    use omni_core::output::{print_error, print_success};

    match cmd {
        ConfigCmd::Show => {
            if out.is_json() {
                out.print_json(cfg);
            } else {
                let rendered = toml::to_string_pretty(cfg).unwrap_or_else(|_| format!("{cfg:?}"));
                println!("{rendered}");
            }
        }
        ConfigCmd::Path => {
            let p = omni_core::platform::config_file_path();
            if out.is_json() {
                out.print_json(&serde_json::json!({ "path": p.display().to_string() }));
            } else {
                println!("{}", p.display());
            }
        }
        ConfigCmd::Read { path } => {
            let result = omni_config::show_config(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                let pretty = serde_json::to_string_pretty(&result.content).unwrap_or_default();
                println!("{pretty}");
            }
        }
        ConfigCmd::Get { path, key } => {
            let val = omni_config::get_key(&path, &key).map_err(|e| anyhow::anyhow!("{e}"))?;
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
            omni_config::set_key(&path, &key, &value).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&serde_json::json!({ "key": key, "value": value, "ok": true }));
            } else {
                print_success(out, &format!("Set {key} = {value}"));
            }
        }
        ConfigCmd::Validate { path } => {
            let result = omni_config::validate_config(&path);
            if out.is_json() {
                out.print_json(&result);
            } else if result.valid {
                println!("{} is valid {} ✓", path.display(), result.format);
            } else {
                print_error(
                    out,
                    &format!("{}: {}", path.display(), result.error.unwrap_or_default()),
                );
            }
        }
    }
    Ok(())
}

// ── omni dev ──────────────────────────────────────────────────────────────────

fn dispatch_dev(cmd: DevCmd, out: &OutputConfig) -> Result<()> {
    use omni_core::output::print_muted;
    use std::io::Read;

    fn stdin_or(arg: Option<String>) -> Result<String> {
        match arg {
            Some(s) => Ok(s),
            None => {
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                Ok(buf.trim_end().to_owned())
            }
        }
    }

    match cmd {
        DevCmd::Hash { input, algo } => {
            let text = stdin_or(input)?;
            let result =
                omni_dev::compute_hash(&text, &algo).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("\x1b[96m{}\x1b[0m", result.digest);
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
            let result = omni_dev::process_json(&text, &action, query.as_deref())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if result.valid {
                println!("{}", result.output);
            } else {
                eprintln!("Invalid JSON: {}", result.error.unwrap_or_default());
            }
        }

        DevCmd::Base64 { input, decode } => {
            let text = stdin_or(input)?;
            let result =
                omni_dev::process_base64(&text, decode).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("{}", result.output);
            }
        }

        DevCmd::Uuid { count, ver } => {
            let result =
                omni_dev::generate_uuids(count, &ver).map_err(|e| anyhow::anyhow!("{e}"))?;
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
            let result = omni_dev::test_regex(&pattern, &input, &flags)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else if !result.valid {
                eprintln!("Invalid regex: {}", result.error.unwrap_or_default());
            } else {
                print_muted(out, &format!("{} match(es)", result.match_count));
                for m in &result.matches {
                    println!("  [{:>5}–{:<5}] \x1b[93m{}\x1b[0m", m.start, m.end, m.text);
                }
            }
        }

        DevCmd::Jwt { token } => {
            use omni_core::output::{print_success, print_warning};
            let text = stdin_or(token)?;
            let result = omni_dev::decode_jwt(&text).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
            } else {
                println!("\x1b[90m── Header ─────────────────────────────────────\x1b[0m");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result.header).unwrap_or_default()
                );
                println!("\x1b[90m── Payload ────────────────────────────────────\x1b[0m");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result.payload).unwrap_or_default()
                );
                if let Some(exp) = result.is_expired {
                    if exp {
                        print_warning(
                            out,
                            &format!("⚠  EXPIRED at {}", result.expires_at.unwrap_or_default()),
                        );
                    } else {
                        print_success(
                            out,
                            &format!("✓  Valid until {}", result.expires_at.unwrap_or_default()),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

// ── omni backup ───────────────────────────────────────────────────────────────

fn dispatch_backup(cmd: BackupCmd, out: &OutputConfig) -> Result<()> {
    use omni_backup::{backup_create, backup_restore, backup_verify, list_snapshots};
    use omni_core::{
        output::{print_error, print_muted, print_success},
        platform::format_bytes,
    };

    match cmd {
        BackupCmd::Create { source, dest, name } => {
            let result = backup_create(&source, &dest, &name, out.quiet)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&result);
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
                        "  \x1b[96m{}\x1b[0m  {}  {} files  ← {}",
                        s.snapshot_id, s.created_at, s.file_count, s.source_path
                    );
                }
            }
        }
    }
    Ok(())
}

// ── omni workspace ────────────────────────────────────────────────────────────

fn dispatch_workspace(cmd: WorkspaceCmd, out: &OutputConfig) -> Result<()> {
    use omni_core::output::print_muted;
    use omni_workspace::{open_workspace_db, workspace_db_path};

    let db_path = workspace_db_path();
    let conn = open_workspace_db(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;

    match cmd {
        WorkspaceCmd::Note { cmd } => dispatch_note(cmd, &conn, out)?,
        WorkspaceCmd::Todo { cmd } => dispatch_todo(cmd, &conn, out)?,
        WorkspaceCmd::Snippet { cmd } => dispatch_snippet(cmd, &conn, out)?,
        WorkspaceCmd::Stats => {
            let notes: i64 = conn
                .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
                .unwrap_or(0);
            let todos: i64 = conn
                .query_row("SELECT COUNT(*) FROM todos", [], |r| r.get(0))
                .unwrap_or(0);
            let snippets: i64 = conn
                .query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0))
                .unwrap_or(0);
            let stats = serde_json::json!({ "notes": notes, "todos": todos, "snippets": snippets });
            if out.is_json() {
                out.print_json(&stats);
            } else {
                println!("Workspace stats:");
                print_muted(out, &format!("  Notes:    {notes}"));
                print_muted(out, &format!("  Todos:    {todos}"));
                print_muted(out, &format!("  Snippets: {snippets}"));
            }
        }
    }
    Ok(())
}

fn dispatch_note(cmd: NoteCmd, conn: &rusqlite::Connection, out: &OutputConfig) -> Result<()> {
    use omni_core::output::{print_muted, print_success};
    use omni_workspace::{create_note, delete_note, get_note, list_notes};

    match cmd {
        NoteCmd::List { search } => {
            let notes = list_notes(conn, search.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&notes);
            } else if notes.is_empty() {
                print_muted(out, "No notes.");
            } else {
                for n in &notes {
                    println!("  \x1b[96m#{:<4}\x1b[0m  {}", n.id, n.title);
                }
            }
        }
        NoteCmd::New { title, body, tags } => {
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
                println!("\x1b[1m{}\x1b[0m", n.title);
                if let Some(t) = &n.tags {
                    println!("\x1b[90mtags: {t}\x1b[0m");
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
    use omni_core::output::{print_muted, print_success};
    use omni_workspace::{create_todo, delete_todo, list_todos, toggle_todo};

    match cmd {
        TodoCmd::List { done } => {
            let todos = list_todos(conn, done).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&todos);
            } else if todos.is_empty() {
                print_muted(out, "No todos.");
            } else {
                for t in &todos {
                    let mark = if t.done {
                        "\x1b[32m✓\x1b[0m"
                    } else {
                        "\x1b[90m○\x1b[0m"
                    };
                    println!("  {mark} \x1b[96m#{:<4}\x1b[0m  {}", t.id, t.description);
                }
            }
        }
        TodoCmd::Add { description, due } => {
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
    use omni_core::output::{print_muted, print_success};
    use omni_workspace::{create_snippet, delete_snippet, get_snippet, list_snippets};

    match cmd {
        SnippetCmd::List { lang } => {
            let snips = list_snippets(conn, lang.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;
            if out.is_json() {
                out.print_json(&snips);
            } else if snips.is_empty() {
                print_muted(out, "No snippets.");
            } else {
                for s in &snips {
                    let lang = s.language.as_deref().unwrap_or("-");
                    println!(
                        "  \x1b[96m#{:<4}\x1b[0m  \x1b[93m{:<8}\x1b[0m  {}",
                        s.id, lang, s.name
                    );
                }
            }
        }
        SnippetCmd::Save { name, lang, body } => {
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
                    "\x1b[1m{}\x1b[0m  \x1b[90m{}\x1b[0m",
                    s.name,
                    s.language.as_deref().unwrap_or("")
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

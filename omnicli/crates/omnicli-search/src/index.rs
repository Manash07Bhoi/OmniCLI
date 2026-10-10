use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::{params, Connection};
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::SearchError;

/// Statistics from an indexing run.
#[derive(Debug, Serialize)]
pub struct IndexStats {
    pub files_indexed: u64,
    pub files_skipped: u64,
    pub content_docs_indexed: u64,
}

/// Open (or create) the search index SQLite database.
pub fn open_index_db(db_path: &Path) -> Result<Connection, SearchError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), SearchError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS search_index (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            file_type TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            content_hash TEXT,
            mtime INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS search_content_fts USING fts5(
            path UNINDEXED,
            content,
            tokenize = 'porter unicode61'
        );
        "#,
    )?;
    Ok(())
}

/// Build (or rebuild) the search index for the given paths.
///
/// When `rebuild` is true, clears existing index data first.
///
/// Progress display:
/// - On a TTY: shows a live spinner with a running file count, updated at most 10×/sec.
/// - With `--json` / piped: no progress output (controlled by the caller via `quiet`).
pub fn rebuild_index(
    conn: &mut Connection,
    paths: &[PathBuf],
    exclude: &[String],
    rebuild: bool,
    quiet: bool,
) -> Result<IndexStats, SearchError> {
    if rebuild {
        conn.execute_batch("DELETE FROM search_index; DELETE FROM search_content_fts;")?;
    }

    let mut files_indexed = 0u64;
    let mut files_skipped = 0u64;
    let mut content_docs_indexed = 0u64;

    // Progress bar: live file count with spinner. Ticks at 100 ms → max 10 updates/sec.
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} Indexing… {pos} files  {elapsed_precise}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    if quiet {
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    } else {
        pb.enable_steady_tick(Duration::from_millis(100));
    }

    let tx = conn.transaction()?;
    tx.execute(
        "CREATE TEMP TABLE IF NOT EXISTS _ft_stage (path TEXT PRIMARY KEY, content TEXT)",
        [],
    )?;
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for root in paths {
        let root = omnicli_core::platform::expand_tilde(&root.to_string_lossy());
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    files_skipped += 1;
                    continue;
                }
            };

            // Check exclude patterns
            let path_str = entry.path().to_string_lossy().to_string();
            if exclude.iter().any(|ex| path_str.contains(ex.as_str())) {
                continue;
            }

            let ft = entry.file_type();
            let type_str = if ft.is_file() {
                "file"
            } else if ft.is_dir() {
                "dir"
            } else {
                "symlink"
            };

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    files_skipped += 1;
                    continue;
                }
            };

            let size_bytes = if ft.is_file() { meta.len() } else { 0 } as i64;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Content hash (files only, ≤ 10 MB to avoid thrashing)
            let content_hash: Option<String> = if ft.is_file() && size_bytes < 10 * 1024 * 1024 {
                hash_file_blake3(entry.path()).ok()
            } else {
                None
            };

            tx.execute(
                r#"INSERT INTO search_index (path, file_type, size_bytes, content_hash, mtime, indexed_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                   ON CONFLICT(path) DO UPDATE SET
                       file_type = excluded.file_type,
                       size_bytes = excluded.size_bytes,
                       content_hash = excluded.content_hash,
                       mtime = excluded.mtime,
                       indexed_at = excluded.indexed_at"#,
                params![path_str, type_str, size_bytes, content_hash, mtime, now],
            )?;

            files_indexed += 1;

            // Advance the spinner counter (indicatif reads `pos` for {pos} in template)
            pb.set_position(files_indexed);

            // Index text content for searchable file types. Extracted text is
            // staged in a temp table and applied after the walk: replacing a
            // file's content per-iteration means an unindexed lookup inside the
            // FTS table for every file, which is quadratic on large trees.
            if ft.is_file() {
                if let Some(text) = extract_text_content(entry.path(), size_bytes as u64) {
                    tx.execute(
                        "INSERT OR REPLACE INTO _ft_stage (path, content) VALUES (?1, ?2)",
                        params![path_str, text],
                    )?;
                    content_docs_indexed += 1;
                }
            }
        }
    }

    if content_docs_indexed > 0 {
        // One scan of the FTS table instead of one per file, and no duplicate
        // rows afterwards: the same file is indexed exactly once, so a re-index
        // never makes it appear several times in results.
        tx.execute(
            "DELETE FROM search_content_fts WHERE path IN (SELECT path FROM _ft_stage)",
            [],
        )?;
        tx.execute(
            "INSERT INTO search_content_fts (path, content) SELECT path, content FROM _ft_stage",
            [],
        )?;
        tx.execute("DELETE FROM _ft_stage", [])?;
    }

    tx.commit()?;
    pb.finish_and_clear();

    Ok(IndexStats {
        files_indexed,
        files_skipped,
        content_docs_indexed,
    })
}

/// Read and return text content for indexing, for supported file types.
/// Returns None for binary files or unsupported types.
fn extract_text_content(path: &Path, size_bytes: u64) -> Option<String> {
    // Limit to 1 MB of content per file
    const MAX_CONTENT: u64 = 1024 * 1024;
    if size_bytes > MAX_CONTENT {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let text_extensions = [
        "txt",
        "md",
        "rs",
        "py",
        "js",
        "ts",
        "go",
        "c",
        "cpp",
        "h",
        "hpp",
        "java",
        "rb",
        "sh",
        "bash",
        "zsh",
        "fish",
        "toml",
        "yaml",
        "yml",
        "json",
        "xml",
        "html",
        "htm",
        "css",
        "sql",
        "log",
        "conf",
        "cfg",
        "ini",
        "env",
        "gitignore",
        "makefile",
        "cmake",
        "dockerfile",
        "lock",
        "mod",
        "sum",
        "tf",
        "hcl",
        "cs",
        "swift",
        "kt",
        "scala",
        "hs",
    ];

    if !text_extensions.contains(&ext.as_str()) && !ext.is_empty() {
        return None;
    }

    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;

    // Reject binary content (contains NUL bytes)
    if buf.contains(&0u8) {
        return None;
    }

    String::from_utf8(buf).ok()
}

fn hash_file_blake3(path: &Path) -> Result<String> {
    let data = std::fs::read(path)?;
    Ok(blake3::hash(&data).to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_index_db() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_index_db(&db_path).unwrap();
        // Schema must exist
        conn.execute_batch("SELECT * FROM search_index LIMIT 1;")
            .unwrap();
    }

    #[test]
    fn test_rebuild_index() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");

        std::fs::write(dir.path().join("hello.txt"), b"hello world rust").unwrap();
        std::fs::write(
            dir.path().join("code.rs"),
            b"fn main() { println!(\"hi\"); }",
        )
        .unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        let stats = rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

        assert!(stats.files_indexed >= 2);
        assert!(stats.content_docs_indexed >= 2);
    }

    #[test]
    fn reindexing_does_not_duplicate_content_rows() {
        // Regression: every `search index` run used to append another content
        // row for the same file, so one file showed up several times in results
        // and the index grew on each run.
        let dir = tempdir().unwrap();
        let root = dir.path().join("corpus");
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..3 {
            std::fs::write(
                root.join(format!("file{i}.txt")),
                format!("needle content {i}"),
            )
            .unwrap();
        }

        let db_path = dir.path().join("index.db");
        let mut conn = open_index_db(&db_path).unwrap();
        for _ in 0..3 {
            rebuild_index(&mut conn, std::slice::from_ref(&root), &[], false, true).unwrap();
        }

        let content_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM search_content_fts", [], |r| r.get(0))
            .unwrap();
        let distinct: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT path) FROM search_content_fts",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content_rows, distinct, "one content row per file");
        assert_eq!(distinct, 3);

        // Directories are indexed too, so count the file rows explicitly.
        let stats: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM search_index WHERE file_type = 'file'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stats, 3, "the file table is upserted, not duplicated");
    }

    #[test]
    fn rebuild_flag_clears_previous_rows() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("corpus");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.txt"), "alpha").unwrap();

        let db_path = dir.path().join("index.db");
        let mut conn = open_index_db(&db_path).unwrap();
        rebuild_index(&mut conn, std::slice::from_ref(&root), &[], false, true).unwrap();

        // A file disappears, and a rebuild must drop its row.
        std::fs::remove_file(root.join("a.txt")).unwrap();
        std::fs::write(root.join("b.txt"), "beta").unwrap();
        rebuild_index(&mut conn, std::slice::from_ref(&root), &[], true, true).unwrap();

        let paths: Vec<String> = conn
            .prepare("SELECT path FROM search_index WHERE file_type = 'file'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(paths.len(), 1, "only the new file remains: {paths:?}");
        assert!(paths[0].ends_with("b.txt"), "paths: {paths:?}");
    }

    #[test]
    fn test_rebuild_index_excludes() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");
        let node_modules = dir.path().join("node_modules");
        std::fs::create_dir(&node_modules).unwrap();
        std::fs::write(node_modules.join("pkg.json"), b"{}").unwrap();
        std::fs::write(dir.path().join("main.rs"), b"fn main() {}").unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        let stats = rebuild_index(
            &mut conn,
            &[dir.path().to_owned()],
            &["node_modules".to_string()],
            true,
            true,
        )
        .unwrap();

        // node_modules/pkg.json must be excluded
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM search_index WHERE path LIKE '%node_modules%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        assert!(stats.files_indexed >= 1);
    }
}

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::{Connection, params};
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
/// When `rebuild` is true, clears existing index data first.
pub fn rebuild_index(
    conn: &mut Connection,
    paths: &[PathBuf],
    exclude: &[String],
    rebuild: bool,
) -> Result<IndexStats, SearchError> {
    if rebuild {
        conn.execute_batch(
            "DELETE FROM search_index; DELETE FROM search_content_fts;",
        )?;
    }

    let mut files_indexed = 0u64;
    let mut files_skipped = 0u64;
    let mut content_docs_indexed = 0u64;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );

    let tx = conn.transaction()?;
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for root in paths {
        let root = omni_core::platform::expand_tilde(&root.to_string_lossy());
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

            pb.set_message(format!("indexing {path_str}"));

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

            // Index text content for searchable file types
            if ft.is_file() {
                let content = extract_text_content(entry.path(), size_bytes as u64);
                if let Some(text) = content {
                    tx.execute(
                        "INSERT INTO search_content_fts (path, content) VALUES (?1, ?2)",
                        params![path_str, text],
                    )?;
                    content_docs_indexed += 1;
                }
            }
        }
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
        "txt", "md", "rs", "py", "js", "ts", "go", "c", "cpp", "h", "hpp",
        "java", "rb", "sh", "bash", "zsh", "fish", "toml", "yaml", "yml",
        "json", "xml", "html", "htm", "css", "sql", "log", "conf", "cfg",
        "ini", "env", "gitignore", "makefile", "cmake", "dockerfile",
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
        conn.execute_batch("SELECT * FROM search_index LIMIT 1;").unwrap();
    }

    #[test]
    fn test_rebuild_index() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");

        // Create some test files
        std::fs::write(dir.path().join("hello.txt"), b"hello world rust").unwrap();
        std::fs::write(dir.path().join("code.rs"), b"fn main() { println!(\"hi\"); }").unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        let stats = rebuild_index(
            &mut conn,
            &[dir.path().to_owned()],
            &[],
            true,
        )
        .unwrap();

        assert!(stats.files_indexed >= 2);
        assert!(stats.content_docs_indexed >= 2);
    }
}

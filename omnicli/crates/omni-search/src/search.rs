use anyhow::Result;
use regex::Regex;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::SearchError;

/// Content types to include in the search.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter {
    pub files: bool,
    pub pdf: bool,
    pub code: bool,
    pub sqlite: bool,
    pub json: bool,
    pub logs: bool,
    pub zip: bool,
}

impl ContentFilter {
    /// Parse a comma-separated list like "code,logs,pdf".
    pub fn parse(s: &str) -> Self {
        let mut f = Self::default();
        for part in s.split(',') {
            match part.trim() {
                "files" => f.files = true,
                "pdf" => f.pdf = true,
                "code" => f.code = true,
                "sqlite" => f.sqlite = true,
                "json" => f.json = true,
                "logs" => f.logs = true,
                "zip" => f.zip = true,
                _ => {}
            }
        }
        // If nothing specified, search all
        if !f.files && !f.pdf && !f.code && !f.sqlite && !f.json && !f.logs && !f.zip {
            f.files = true;
            f.code = true;
            f.logs = true;
        }
        f
    }

    pub fn all() -> Self {
        Self {
            files: true,
            pdf: true,
            code: true,
            sqlite: true,
            json: true,
            logs: true,
            zip: true,
        }
    }
}

/// Options for `omni search <query>`.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub query: String,
    pub content_filter: ContentFilter,
    pub use_regex: bool,
    pub case_sensitive: bool,
    pub limit: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            content_filter: ContentFilter::all(),
            use_regex: false,
            case_sensitive: false,
            limit: 100,
        }
    }
}

/// A single search result.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub file_type: String,
    pub size_bytes: i64,
    /// Snippet of matching content (if available).
    pub snippet: Option<String>,
    /// Line number within the file where the match was found (if available).
    pub line_number: Option<u64>,
}

/// Run a search query against the persistent index.
pub fn search_query(
    conn: &Connection,
    opts: &SearchOptions,
) -> Result<Vec<SearchResult>, SearchError> {
    // Verify the index exists
    let index_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM search_index", [], |r| r.get(0))
        .unwrap_or(0);

    if index_count == 0 {
        return Err(SearchError::IndexNotFound);
    }

    let mut results: Vec<SearchResult> = Vec::new();

    // 1. FTS5 content search
    let fts_query = if opts.use_regex {
        // For regex, do a broad FTS5 search and filter afterward
        "*".to_owned()
    } else {
        // Escape FTS5 special characters
        fts5_escape(&opts.query)
    };

    if fts_query != "*" {
        let sql = r#"
            SELECT si.path, si.file_type, si.size_bytes, fts.content
            FROM search_content_fts fts
            JOIN search_index si ON fts.path = si.path
            WHERE search_content_fts MATCH ?1
            ORDER BY rank
            LIMIT ?2
            "#;

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![fts_query, opts.limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        for row in rows {
            let (path, file_type, size_bytes, content) = row?;
            let (snippet, line_number) =
                extract_snippet(&content, &opts.query, opts.case_sensitive);
            results.push(SearchResult {
                path,
                file_type,
                size_bytes,
                snippet,
                line_number,
            });
        }
    }

    // 2. Path/name search (for files not in FTS index)
    let path_pattern = format!("%{}%", opts.query);
    let path_sql = r#"
        SELECT path, file_type, size_bytes
        FROM search_index
        WHERE path LIKE ?1 COLLATE NOCASE
        LIMIT ?2
    "#;

    let mut stmt = conn.prepare(path_sql)?;
    let rows = stmt.query_map(params![path_pattern, opts.limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let existing_paths: std::collections::HashSet<String> =
        results.iter().map(|r| r.path.clone()).collect();

    for row in rows {
        let (path, file_type, size_bytes) = row?;
        if !existing_paths.contains(&path) {
            results.push(SearchResult {
                path,
                file_type,
                size_bytes,
                snippet: None,
                line_number: None,
            });
        }
    }

    // Apply regex filter if requested
    if opts.use_regex {
        let re = if opts.case_sensitive {
            Regex::new(&opts.query)?
        } else {
            Regex::new(&format!("(?i){}", opts.query))?
        };
        results.retain(|r| {
            r.snippet
                .as_deref()
                .map(|s| re.is_match(s))
                .unwrap_or_else(|| re.is_match(&r.path))
        });
    }

    results.truncate(opts.limit);
    Ok(results)
}

/// Extract a snippet and line number from content for the given query.
fn extract_snippet(
    content: &str,
    query: &str,
    case_sensitive: bool,
) -> (Option<String>, Option<u64>) {
    let needle = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };

    for (line_no, line) in content.lines().enumerate() {
        let haystack = if case_sensitive {
            line.to_owned()
        } else {
            line.to_lowercase()
        };
        if haystack.contains(&needle) {
            let snippet = line.trim().chars().take(200).collect::<String>();
            return (Some(snippet), Some(line_no as u64 + 1));
        }
    }
    (None, None)
}

/// Escape a query string for FTS5 (avoids special tokens like AND, OR, NOT, etc.).
fn fts5_escape(query: &str) -> String {
    // Wrap in double quotes so it's treated as a phrase search
    format!("\"{}\"", query.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{open_index_db, rebuild_index};
    use tempfile::tempdir;

    #[test]
    fn test_search_finds_content() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");

        std::fs::write(dir.path().join("needle.txt"), b"the quick brown fox").unwrap();
        std::fs::write(dir.path().join("other.txt"), b"something else entirely").unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true).unwrap();

        let opts = SearchOptions {
            query: "quick brown".into(),
            ..Default::default()
        };
        let results = search_query(&conn, &opts).unwrap();
        assert!(
            results.iter().any(|r| r.path.contains("needle.txt")),
            "expected needle.txt in results, got: {results:?}"
        );
    }

    #[test]
    fn test_search_path_match() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");

        std::fs::write(dir.path().join("my_unique_filename.txt"), b"content").unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true).unwrap();

        let opts = SearchOptions {
            query: "my_unique_filename".into(),
            ..Default::default()
        };
        let results = search_query(&conn, &opts).unwrap();
        assert!(results
            .iter()
            .any(|r| r.path.contains("my_unique_filename")));
    }

    #[test]
    fn test_fts5_escape() {
        assert_eq!(fts5_escape("hello world"), "\"hello world\"");
        assert_eq!(fts5_escape("CVE-2026-1234"), "\"CVE-2026-1234\"");
    }

    #[test]
    fn test_content_filter_parse() {
        let f = ContentFilter::parse("code,logs");
        assert!(f.code);
        assert!(f.logs);
        assert!(!f.pdf);
    }
}

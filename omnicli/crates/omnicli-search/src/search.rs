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
        // An empty or fully unrecognised list falls back to the common categories.
        if f.is_empty() {
            f.files = true;
            f.code = true;
            f.logs = true;
        }
        f
    }

    /// Parse a comma-separated list, rejecting any unrecognised token.
    ///
    /// Unlike [`ContentFilter::parse`], a typo is an error rather than a silent
    /// fall back to the default categories.
    pub fn parse_strict(s: &str) -> Result<Self, SearchError> {
        let mut f = Self::default();
        let mut any = false;
        for part in s.split(',') {
            let token = part.trim();
            if token.is_empty() {
                continue;
            }
            match token {
                "files" => f.files = true,
                "pdf" => f.pdf = true,
                "code" => f.code = true,
                "sqlite" => f.sqlite = true,
                "json" => f.json = true,
                "logs" => f.logs = true,
                "zip" => f.zip = true,
                other => {
                    return Err(SearchError::Other(anyhow::anyhow!(
                        "Unknown --in value '{other}'. Valid: files, pdf, code, sqlite, json, logs, zip."
                    )))
                }
            }
            any = true;
        }
        if !any {
            return Err(SearchError::Other(anyhow::anyhow!(
                "--in was empty. Valid values: files, pdf, code, sqlite, json, logs, zip."
            )));
        }
        Ok(f)
    }

    /// True when entries of `category` (as classified by [`category_of`]) may
    /// appear in results.
    pub fn allows(&self, category: &str) -> bool {
        match category {
            "pdf" => self.pdf,
            "code" => self.code,
            "sqlite" => self.sqlite,
            "json" => self.json,
            "logs" => self.logs,
            "zip" => self.zip,
            _ => self.files,
        }
    }

    /// True when every category is enabled (no `--in` restriction).
    pub fn is_all(&self) -> bool {
        self.files && self.pdf && self.code && self.sqlite && self.json && self.logs && self.zip
    }

    /// True when no category was selected.
    ///
    /// `parse` never returns an empty filter (it falls back to files+code+logs),
    /// so callers can use this to reject an unrecognised `--in` value.
    pub fn is_empty(&self) -> bool {
        !self.files
            && !self.pdf
            && !self.code
            && !self.sqlite
            && !self.json
            && !self.logs
            && !self.zip
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

/// Options for `omnicli search <query>`.
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

/// Map a path to one of the `--in` content categories: files, pdf, code,
/// sqlite, json, logs, or zip.
///
/// Deliberately extension-based — the index stores no separate content-type
/// column, and the categories mirror what
/// [`crate::index::extract_text_content`] reads: `code`/`json`/`logs`/`files`
/// entries have searchable content, while `pdf`/`sqlite`/`zip` entries can
/// match by name.
fn category_of(path: &str) -> &'static str {
    let ext = match path.rsplit_once('.') {
        Some((_, e)) => e.to_ascii_lowercase(),
        None => return "files",
    };
    const CODE: &[&str] = &[
        "rs",
        "py",
        "js",
        "jsx",
        "ts",
        "tsx",
        "go",
        "c",
        "cc",
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
        "xml",
        "html",
        "htm",
        "css",
        "sql",
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
        "php",
        "pl",
        "lua",
        "proto",
        "vue",
    ];
    if ext == "pdf" {
        "pdf"
    } else if matches!(ext.as_str(), "db" | "sqlite" | "sqlite3" | "db3") {
        "sqlite"
    } else if matches!(
        ext.as_str(),
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "rar" | "7z"
    ) {
        "zip"
    } else if matches!(ext.as_str(), "json" | "jsonl" | "ndjson" | "json5") {
        "json"
    } else if ext == "log" {
        "logs"
    } else if CODE.contains(&ext.as_str()) {
        "code"
    } else {
        "files"
    }
}

/// Run a search query against the persistent index.
///
/// Modes:
/// - default: FTS5 phrase search over indexed content, plus name matches;
/// - `use_regex`: the pattern is applied to every indexed file's content and
///   to entry paths, so a pattern like `err` can match inside the token
///   `error` — which full-text search deliberately does not do.
///
/// `content_filter` (`--in`) restricts which entry categories may appear in
/// either mode. `case_sensitive` requires the query to match with exact
/// casing; regex mode honours it by not enabling case-insensitive matching.
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

    let regex = if opts.use_regex {
        let pattern = if opts.case_sensitive {
            opts.query.clone()
        } else {
            format!("(?i){}", opts.query)
        };
        Some(Regex::new(&pattern)?)
    } else {
        None
    };

    if let Some(re) = &regex {
        // 1. Content scan: apply the pattern to every indexed file's text.
        let sql = r#"
            SELECT si.path, si.file_type, si.size_bytes, fts.content
            FROM search_content_fts fts
            JOIN search_index si ON fts.path = si.path
            ORDER BY fts.path
            "#;
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            if results.len() >= opts.limit {
                break;
            }
            let path: String = row.get(0)?;
            if !opts.content_filter.allows(category_of(&path)) {
                continue;
            }
            if let Some((snippet, line_number)) = first_matching_line(&row.get::<_, String>(3)?, re)
            {
                results.push(SearchResult {
                    path,
                    file_type: row.get(1)?,
                    size_bytes: row.get(2)?,
                    snippet: Some(snippet),
                    line_number: Some(line_number),
                });
            }
        }

        // 2. Path scan: also match the pattern against entry names — for
        //    entries whose content is not indexed, and for `pdf`/`sqlite`/`zip`
        //    categories that only have names in the index.
        if results.len() < opts.limit {
            let seen: std::collections::HashSet<String> =
                results.iter().map(|r| r.path.clone()).collect();
            let mut stmt =
                conn.prepare("SELECT path, file_type, size_bytes FROM search_index ORDER BY path")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let path: String = row.get(0)?;
                if seen.contains(&path) || !opts.content_filter.allows(category_of(&path)) {
                    continue;
                }
                if !re.is_match(&path) {
                    continue;
                }
                let file_type: String = row.get(1)?;
                let size_bytes: i64 = row.get(2)?;
                results.push(SearchResult {
                    path,
                    file_type,
                    size_bytes,
                    snippet: None,
                    line_number: None,
                });
                if results.len() >= opts.limit {
                    break;
                }
            }
        }
    } else {
        // 1. FTS5 phrase search over indexed content.
        let fts_query = fts5_escape(&opts.query);
        // With a `--in` filter active, fetched rows can be dropped afterwards,
        // so over-fetch proportionally to still fill the result limit.
        let fetch = if opts.content_filter.is_all() {
            opts.limit as i64
        } else {
            (opts.limit as i64).saturating_mul(20).clamp(200, 20_000)
        };
        let sql = r#"
            SELECT si.path, si.file_type, si.size_bytes, fts.content
            FROM search_content_fts fts
            JOIN search_index si ON fts.path = si.path
            WHERE search_content_fts MATCH ?1
            ORDER BY rank
            LIMIT ?2
            "#;

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![fts_query, fetch], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        // Defensive de-duplication: indexes written before content rows were
        // replaced in place can still hold several rows for one file. Showing a
        // path once (best-ranked) keeps results correct without a rebuild.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for row in rows {
            let (path, file_type, size_bytes, content) = row?;
            if !seen.insert(path.clone()) {
                continue;
            }
            if !opts.content_filter.allows(category_of(&path)) {
                continue;
            }
            let (snippet, line_number) =
                extract_snippet(&content, &opts.query, opts.case_sensitive);
            // FTS MATCH ignores case (and may match via word stemming); when an
            // exact-case query finds no exact line, this row does not truly
            // match what was asked for.
            if opts.case_sensitive && snippet.is_none() {
                continue;
            }
            results.push(SearchResult {
                path,
                file_type,
                size_bytes,
                snippet,
                line_number,
            });
            if results.len() >= opts.limit {
                break;
            }
        }

        // 2. Path/name search (for files not in FTS index).
        if results.len() < opts.limit {
            let path_pattern = format!("%{}%", opts.query);
            let path_sql = r#"
                SELECT path, file_type, size_bytes
                FROM search_index
                WHERE path LIKE ?1 COLLATE NOCASE
                LIMIT ?2
            "#;

            let mut stmt = conn.prepare(path_sql)?;
            let rows = stmt.query_map(params![path_pattern, fetch], |row| {
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
                if existing_paths.contains(&path) || !opts.content_filter.allows(category_of(&path))
                {
                    continue;
                }
                // The LIKE above ignores case; a case-sensitive query must
                // match the name with exact casing.
                if opts.case_sensitive && !path.contains(&opts.query) {
                    continue;
                }
                results.push(SearchResult {
                    path,
                    file_type,
                    size_bytes,
                    snippet: None,
                    line_number: None,
                });
                if results.len() >= opts.limit {
                    break;
                }
            }
        }
    }

    results.truncate(opts.limit);
    Ok(results)
}

/// First line of `content` matching `re`, as a trimmed snippet plus a 1-based
/// line number.
fn first_matching_line(content: &str, re: &Regex) -> Option<(String, u64)> {
    for (idx, line) in content.lines().enumerate() {
        if re.is_match(line) {
            let snippet = line.trim().chars().take(200).collect::<String>();
            return Some((snippet, idx as u64 + 1));
        }
    }
    None
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
    fn regex_search_matches_inside_tokens() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("app.log"), b"unexpected ERROR occurred\n").unwrap();
        let mut conn = open_index_db(&dir.path().join("search.db")).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

        // FTS phrase search matches whole tokens only, so `err` finds nothing…
        let opts = SearchOptions {
            query: "err".into(),
            ..Default::default()
        };
        assert!(
            search_query(&conn, &opts).unwrap().is_empty(),
            "FTS must not substring-match tokens"
        );

        // …but `--regex` applies the pattern to the indexed content itself.
        let opts = SearchOptions {
            query: "err".into(),
            use_regex: true,
            ..Default::default()
        };
        let results = search_query(&conn, &opts).unwrap();
        let hit = results
            .iter()
            .find(|r| r.path.contains("app.log"))
            .expect("regex must match inside the token `ERROR`");
        assert_eq!(hit.line_number, Some(1));
        assert!(hit.snippet.as_deref().unwrap().contains("ERROR"));
    }

    #[test]
    fn regex_alternation_matches_documented_example() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            b"// TODO fix this\nfn main() {}\n// FIXME later\n",
        )
        .unwrap();
        let mut conn = open_index_db(&dir.path().join("search.db")).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

        let opts = SearchOptions {
            query: "TODO|FIXME".into(),
            use_regex: true,
            ..Default::default()
        };
        let results = search_query(&conn, &opts).unwrap();
        let hit = results
            .iter()
            .find(|r| r.path.contains("main.rs"))
            .expect("TODO|FIXME must match");
        assert_eq!(hit.line_number, Some(1));
    }

    #[test]
    fn content_filter_restricts_result_types() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.log"), b"payload in the log\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), b"payload in the code\n").unwrap();
        std::fs::write(dir.path().join("c.db"), b"binary-ish\n").unwrap();
        let mut conn = open_index_db(&dir.path().join("search.db")).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

        let run = |filter: ContentFilter| {
            let opts = SearchOptions {
                query: "payload".into(),
                content_filter: filter,
                ..Default::default()
            };
            search_query(&conn, &opts).unwrap()
        };

        // `--in logs` keeps only the log file.
        let got = run(ContentFilter::parse_strict("logs").unwrap());
        assert!(got.iter().any(|r| r.path.contains("a.log")), "got: {got:?}");
        assert!(!got.iter().any(|r| r.path.contains("b.rs")), "got: {got:?}");

        // `--in code` keeps only the source file.
        let got = run(ContentFilter::parse_strict("code").unwrap());
        assert!(got.iter().any(|r| r.path.contains("b.rs")), "got: {got:?}");
        assert!(
            !got.iter().any(|r| r.path.contains("a.log")),
            "got: {got:?}"
        );

        // `--in sqlite`: the .db entry has no indexed content, so a content
        // query cannot match it.
        let got = run(ContentFilter::parse_strict("sqlite").unwrap());
        assert!(got.is_empty(), "got: {got:?}");
    }

    #[test]
    fn case_sensitive_query_requires_exact_case() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("mixed.txt"),
            b"shout ERROR loud\nplain calm line\n",
        )
        .unwrap();
        let mut conn = open_index_db(&dir.path().join("search.db")).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

        let run = |query: &str, case_sensitive: bool| {
            let opts = SearchOptions {
                query: query.into(),
                case_sensitive,
                ..Default::default()
            };
            search_query(&conn, &opts).unwrap()
        };

        // Exact-case `error`: the file only has `error` inside `ERROR`, so a
        // case-sensitive query must not match…
        assert!(
            run("error", true).is_empty(),
            "case-sensitive `error` must not match `ERROR`"
        );
        // …while `ERROR` matches and reports the right line.
        let got = run("ERROR", true);
        let hit = got
            .iter()
            .find(|r| r.path.contains("mixed.txt"))
            .expect("exact-case ERROR must match");
        assert_eq!(hit.line_number, Some(1));

        // Case-insensitive remains the default.
        assert!(!run("error", false).is_empty());
    }

    #[test]
    fn test_search_finds_content() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("search.db");

        std::fs::write(dir.path().join("needle.txt"), b"the quick brown fox").unwrap();
        std::fs::write(dir.path().join("other.txt"), b"something else entirely").unwrap();

        let mut conn = open_index_db(&db_path).unwrap();
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

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
        rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true, true).unwrap();

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

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::WorkspaceError;

#[derive(Debug, Serialize)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub language: Option<String>,
    pub body: String,
    pub created_at: i64,
}

pub fn list_snippets(
    conn: &Connection,
    language: Option<&str>,
) -> Result<Vec<Snippet>, WorkspaceError> {
    let (sql, use_lang) = match language {
        Some(_) => ("SELECT id, name, language, body, created_at FROM snippets WHERE language = ?1 ORDER BY name", true),
        None    => ("SELECT id, name, language, body, created_at FROM snippets ORDER BY name", false),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if use_lang {
        stmt.query_map(params![language.unwrap()], row_to_snippet)?
    } else {
        stmt.query_map([], row_to_snippet)?
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::from)
}

pub fn create_snippet(
    conn: &Connection,
    name: &str,
    language: Option<&str>,
    body: &str,
) -> Result<Snippet, WorkspaceError> {
    conn.execute(
        "INSERT INTO snippets (name, language, body) VALUES (?1, ?2, ?3)",
        params![name, language, body],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint") {
            WorkspaceError::AlreadyExists {
                item: format!("snippet '{name}'"),
            }
        } else {
            WorkspaceError::Database {
                message: e.to_string(),
            }
        }
    })?;
    let id = conn.last_insert_rowid();
    get_snippet(conn, id)
}

pub fn get_snippet(conn: &Connection, id: i64) -> Result<Snippet, WorkspaceError> {
    conn.query_row(
        "SELECT id, name, language, body, created_at FROM snippets WHERE id = ?1",
        params![id],
        row_to_snippet,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => WorkspaceError::NotFound {
            item: format!("snippet #{id}"),
        },
        other => WorkspaceError::Database {
            message: other.to_string(),
        },
    })
}

pub fn delete_snippet(conn: &Connection, id: i64) -> Result<(), WorkspaceError> {
    let n = conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])?;
    if n == 0 {
        Err(WorkspaceError::NotFound {
            item: format!("snippet #{id}"),
        })
    } else {
        Ok(())
    }
}

fn row_to_snippet(row: &rusqlite::Row<'_>) -> rusqlite::Result<Snippet> {
    Ok(Snippet {
        id: row.get(0)?,
        name: row.get(1)?,
        language: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
    })
}

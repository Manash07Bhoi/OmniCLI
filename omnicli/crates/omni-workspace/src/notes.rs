use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::WorkspaceError;

#[derive(Debug, Serialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub tags: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn list_notes(conn: &Connection, search: Option<&str>) -> Result<Vec<Note>, WorkspaceError> {
    let sql = match search {
        Some(_) => "SELECT id, title, body, tags, created_at, updated_at FROM notes WHERE title LIKE ?1 OR body LIKE ?1 ORDER BY updated_at DESC",
        None    => "SELECT id, title, body, tags, created_at, updated_at FROM notes ORDER BY updated_at DESC",
    };
    let mut stmt = conn.prepare(sql)?;
    let pattern = search.map(|s| format!("%{s}%"));
    let rows = if let Some(ref p) = pattern {
        stmt.query_map(params![p], row_to_note)?
    } else {
        stmt.query_map([], row_to_note)?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::from)
}

pub fn create_note(conn: &Connection, title: &str, body: &str, tags: Option<&str>) -> Result<Note, WorkspaceError> {
    conn.execute(
        "INSERT INTO notes (title, body, tags) VALUES (?1, ?2, ?3)",
        params![title, body, tags],
    )?;
    let id = conn.last_insert_rowid();
    get_note(conn, id)
}

pub fn get_note(conn: &Connection, id: i64) -> Result<Note, WorkspaceError> {
    conn.query_row(
        "SELECT id, title, body, tags, created_at, updated_at FROM notes WHERE id = ?1",
        params![id],
        row_to_note,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => WorkspaceError::NotFound {
            item: format!("note #{id}"),
        },
        other => WorkspaceError::Database { message: other.to_string() },
    })
}

pub fn update_note(conn: &Connection, id: i64, title: Option<&str>, body: Option<&str>, tags: Option<&str>) -> Result<Note, WorkspaceError> {
    // Only update fields that are provided
    if title.is_some() || body.is_some() || tags.is_some() {
        let mut parts = vec!["updated_at = strftime('%s','now')"];
        if title.is_some() { parts.push("title = ?2"); }
        if body.is_some()  { parts.push("body = ?3"); }
        if tags.is_some()  { parts.push("tags = ?4"); }
        let sql = format!("UPDATE notes SET {} WHERE id = ?1", parts.join(", "));
        conn.execute(&sql, params![id, title, body, tags])?;
    }
    get_note(conn, id)
}

pub fn delete_note(conn: &Connection, id: i64) -> Result<(), WorkspaceError> {
    let n = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
    if n == 0 {
        Err(WorkspaceError::NotFound { item: format!("note #{id}") })
    } else {
        Ok(())
    }
}

fn row_to_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id:         row.get(0)?,
        title:      row.get(1)?,
        body:       row.get(2)?,
        tags:       row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

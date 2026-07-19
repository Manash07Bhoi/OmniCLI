use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::WorkspaceError;

#[derive(Debug, Serialize)]
pub struct Todo {
    pub id: i64,
    pub description: String,
    pub done: bool,
    pub due_at: Option<i64>,
    pub created_at: i64,
}

pub fn list_todos(conn: &Connection, done: Option<bool>) -> Result<Vec<Todo>, WorkspaceError> {
    let (sql, use_filter) = match done {
        Some(_) => ("SELECT id, description, done, due_at, created_at FROM todos WHERE done = ?1 ORDER BY created_at DESC", true),
        None    => ("SELECT id, description, done, due_at, created_at FROM todos ORDER BY created_at DESC", false),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if use_filter {
        let d = done.unwrap() as i32;
        stmt.query_map(params![d], row_to_todo)?
    } else {
        stmt.query_map([], row_to_todo)?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::from)
}

pub fn create_todo(conn: &Connection, description: &str, due_at: Option<i64>) -> Result<Todo, WorkspaceError> {
    conn.execute(
        "INSERT INTO todos (description, due_at) VALUES (?1, ?2)",
        params![description, due_at],
    )?;
    let id = conn.last_insert_rowid();
    get_todo(conn, id)
}

pub fn get_todo(conn: &Connection, id: i64) -> Result<Todo, WorkspaceError> {
    conn.query_row(
        "SELECT id, description, done, due_at, created_at FROM todos WHERE id = ?1",
        params![id],
        row_to_todo,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => WorkspaceError::NotFound { item: format!("todo #{id}") },
        other => WorkspaceError::Database { message: other.to_string() },
    })
}

pub fn toggle_todo(conn: &Connection, id: i64) -> Result<Todo, WorkspaceError> {
    let affected = conn.execute(
        "UPDATE todos SET done = NOT done WHERE id = ?1",
        params![id],
    )?;
    if affected == 0 {
        return Err(WorkspaceError::NotFound { item: format!("todo #{id}") });
    }
    get_todo(conn, id)
}

pub fn delete_todo(conn: &Connection, id: i64) -> Result<(), WorkspaceError> {
    let n = conn.execute("DELETE FROM todos WHERE id = ?1", params![id])?;
    if n == 0 {
        Err(WorkspaceError::NotFound { item: format!("todo #{id}") })
    } else {
        Ok(())
    }
}

fn row_to_todo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id:          row.get(0)?,
        description: row.get(1)?,
        done:        row.get::<_, i32>(2)? != 0,
        due_at:      row.get(3)?,
        created_at:  row.get(4)?,
    })
}

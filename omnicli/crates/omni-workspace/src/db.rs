use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::WorkspaceError;

pub fn workspace_db_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("omni").join("workspace.db")
}

pub fn open_workspace_db(db_path: &Path) -> Result<Connection, WorkspaceError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), WorkspaceError> {
    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS notes (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            title     TEXT    NOT NULL,
            body      TEXT    NOT NULL DEFAULT '',
            tags      TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        CREATE TABLE IF NOT EXISTS todos (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            description TEXT    NOT NULL,
            done        INTEGER NOT NULL DEFAULT 0,
            due_at      INTEGER,
            created_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        CREATE TABLE IF NOT EXISTS snippets (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT    NOT NULL UNIQUE,
            language   TEXT,
            body       TEXT    NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );
    "#)?;
    Ok(())
}

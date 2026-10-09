pub mod db;
pub mod error;
pub mod notes;
pub mod snippets;
pub mod todos;

#[cfg(test)]
mod tests;

pub use db::open_workspace_db;
pub use error::WorkspaceError;
pub use notes::{create_note, delete_note, get_note, list_notes, update_note, Note};
pub use snippets::{create_snippet, delete_snippet, get_snippet, list_snippets, Snippet};
pub use todos::{create_todo, delete_todo, get_todo, list_todos, toggle_todo, Todo};

use serde::Serialize;

/// Aggregate counts over the workspace database, used by `omnicli workspace stats`.
#[derive(Debug, Serialize)]
pub struct WorkspaceStats {
    pub notes: i64,
    pub todos: i64,
    pub todos_pending: i64,
    pub snippets: i64,
}

/// Count notes, todos (and pending todos), and snippets.
///
/// Errors from the database are propagated: reporting zeroes for a corrupt or
/// inaccessible database would be indistinguishable from an empty workspace.
pub fn workspace_stats(conn: &rusqlite::Connection) -> Result<WorkspaceStats, WorkspaceError> {
    let count = |sql: &str| -> Result<i64, WorkspaceError> {
        conn.query_row(sql, [], |r| r.get(0))
            .map_err(WorkspaceError::from)
    };
    Ok(WorkspaceStats {
        notes: count("SELECT COUNT(*) FROM notes")?,
        todos: count("SELECT COUNT(*) FROM todos")?,
        todos_pending: count("SELECT COUNT(*) FROM todos WHERE done = 0")?,
        snippets: count("SELECT COUNT(*) FROM snippets")?,
    })
}

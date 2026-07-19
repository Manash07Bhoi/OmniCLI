pub mod db;
pub mod error;
pub mod notes;
pub mod snippets;
pub mod todos;

#[cfg(test)]
mod tests;

pub use db::{open_workspace_db, workspace_db_path};
pub use error::WorkspaceError;
pub use notes::{create_note, delete_note, get_note, list_notes, update_note, Note};
pub use snippets::{create_snippet, delete_snippet, get_snippet, list_snippets, Snippet};
pub use todos::{create_todo, delete_todo, get_todo, list_todos, toggle_todo, Todo};

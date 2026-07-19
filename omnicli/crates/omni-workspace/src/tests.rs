#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use crate::{
        open_workspace_db,
        notes::{create_note, delete_note, get_note, list_notes, update_note},
        todos::{create_todo, delete_todo, get_todo, list_todos, toggle_todo},
        snippets::{create_snippet, delete_snippet, get_snippet, list_snippets},
    };

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        // Apply the workspace schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS notes (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT    NOT NULL,
                body       TEXT    NOT NULL DEFAULT '',
                tags       TEXT,
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
            "#,
        )
        .expect("schema");
        conn
    }

    // ── Notes ─────────────────────────────────────────────────────────────────

    #[test]
    fn create_and_get_note() {
        let conn = in_memory_db();
        let note = create_note(&conn, "Title A", "Body A", Some("tag1,tag2")).unwrap();
        assert_eq!(note.title, "Title A");
        assert_eq!(note.body, "Body A");
        assert_eq!(note.tags.as_deref(), Some("tag1,tag2"));

        let fetched = get_note(&conn, note.id).unwrap();
        assert_eq!(fetched.id, note.id);
        assert_eq!(fetched.title, "Title A");
    }

    #[test]
    fn list_notes_empty_db() {
        let conn = in_memory_db();
        let notes = list_notes(&conn, None).unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn list_notes_returns_all() {
        let conn = in_memory_db();
        create_note(&conn, "Alpha", "body", None).unwrap();
        create_note(&conn, "Beta", "body", None).unwrap();
        let notes = list_notes(&conn, None).unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn list_notes_search_filter() {
        let conn = in_memory_db();
        create_note(&conn, "Rust guide", "cargo tips", None).unwrap();
        create_note(&conn, "Python notes", "pip install", None).unwrap();
        let results = list_notes(&conn, Some("Rust")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust guide");
    }

    #[test]
    fn update_note_changes_fields() {
        let conn = in_memory_db();
        let note = create_note(&conn, "Original", "old body", None).unwrap();
        let updated = update_note(&conn, note.id, Some("Updated"), None, None).unwrap();
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.body, "old body"); // body unchanged
    }

    #[test]
    fn delete_note_removes_it() {
        let conn = in_memory_db();
        let note = create_note(&conn, "ToDelete", "body", None).unwrap();
        delete_note(&conn, note.id).unwrap();
        let result = get_note(&conn, note.id);
        assert!(result.is_err(), "deleted note should not be found");
    }

    #[test]
    fn get_nonexistent_note_returns_error() {
        let conn = in_memory_db();
        let result = get_note(&conn, 99999);
        assert!(result.is_err());
    }

    // ── Todos ─────────────────────────────────────────────────────────────────

    #[test]
    fn create_and_get_todo() {
        let conn = in_memory_db();
        let todo = create_todo(&conn, "Buy groceries", None).unwrap();
        assert_eq!(todo.description, "Buy groceries");
        assert!(!todo.done);

        let fetched = get_todo(&conn, todo.id).unwrap();
        assert_eq!(fetched.id, todo.id);
    }

    #[test]
    fn toggle_todo_flips_done() {
        let conn = in_memory_db();
        let todo = create_todo(&conn, "Write tests", None).unwrap();
        assert!(!todo.done);

        let toggled = toggle_todo(&conn, todo.id).unwrap();
        assert!(toggled.done);

        let toggled_back = toggle_todo(&conn, todo.id).unwrap();
        assert!(!toggled_back.done);
    }

    #[test]
    fn list_todos_returns_all() {
        let conn = in_memory_db();
        create_todo(&conn, "Task 1", None).unwrap();
        create_todo(&conn, "Task 2", None).unwrap();
        let todos = list_todos(&conn, None).unwrap();
        assert_eq!(todos.len(), 2);
    }

    #[test]
    fn delete_todo_removes_it() {
        let conn = in_memory_db();
        let todo = create_todo(&conn, "Temporary", None).unwrap();
        delete_todo(&conn, todo.id).unwrap();
        assert!(get_todo(&conn, todo.id).is_err());
    }

    // ── Snippets ──────────────────────────────────────────────────────────────

    #[test]
    fn create_and_get_snippet() {
        let conn = in_memory_db();
        let s = create_snippet(&conn, "hello-world", Some("rust"), "fn main() {}").unwrap();
        assert_eq!(s.name, "hello-world");
        assert_eq!(s.language.as_deref(), Some("rust"));

        let fetched = get_snippet(&conn, s.id).unwrap();
        assert_eq!(fetched.name, "hello-world");
    }

    #[test]
    fn duplicate_snippet_name_returns_error() {
        let conn = in_memory_db();
        create_snippet(&conn, "unique-name", None, "body").unwrap();
        let r = create_snippet(&conn, "unique-name", None, "other body");
        assert!(r.is_err(), "duplicate snippet name should fail");
    }

    #[test]
    fn list_snippets_returns_all() {
        let conn = in_memory_db();
        create_snippet(&conn, "snip-a", Some("rust"), "fn a() {}").unwrap();
        create_snippet(&conn, "snip-b", Some("python"), "def b(): pass").unwrap();
        let snippets = list_snippets(&conn, None).unwrap();
        assert_eq!(snippets.len(), 2);
    }

    #[test]
    fn delete_snippet_removes_it() {
        let conn = in_memory_db();
        let s = create_snippet(&conn, "temp-snip", None, "body").unwrap();
        delete_snippet(&conn, s.id).unwrap();
        assert!(get_snippet(&conn, s.id).is_err());
    }
}

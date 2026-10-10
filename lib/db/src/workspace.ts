/**
 * The workspace store (`workspace.db`) — owned by the CLI, shared with the API.
 *
 * OmniCLI has exactly one place where notes, todos and snippets live. The CLI
 * creates and owns that file (`omnicli-workspace/src/db.rs` defines the schema),
 * and the dashboard reads and writes the same file so both interfaces always
 * show the same data. This module mirrors the CLI schema one-to-one, including
 * the important detail that timestamps are stored as Unix **seconds**.
 *
 * The dashboard's own bookkeeping (activity log, backup job registry) lives in
 * the separate Drizzle database; see `src/schema/*`.
 */
import Database from "better-sqlite3";
import { drizzle } from "drizzle-orm/better-sqlite3";
import { integer, sqliteTable, text } from "drizzle-orm/sqlite-core";
import fs from "fs";
import path from "path";

// ── Tables (column-for-column identical to the CLI's DDL) ────────────────────

export const workspaceNotesTable = sqliteTable("notes", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  title: text("title").notNull(),
  body: text("body").notNull().default(""),
  tags: text("tags"),
  createdAt: integer("created_at", { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date()),
  updatedAt: integer("updated_at", { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const workspaceTodosTable = sqliteTable("todos", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  description: text("description").notNull(),
  done: integer("done", { mode: "boolean" }).notNull().default(false),
  dueAt: integer("due_at", { mode: "timestamp" }),
  createdAt: integer("created_at", { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const workspaceSnippetsTable = sqliteTable("snippets", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  name: text("name").notNull().unique(),
  language: text("language"),
  body: text("body").notNull(),
  createdAt: integer("created_at", { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const workspaceSchema = {
  notes: workspaceNotesTable,
  todos: workspaceTodosTable,
  snippets: workspaceSnippetsTable,
};

/**
 * Same DDL the CLI runs on first use, so either process can create the file and
 * the other will find a schema it understands.
 */
const SCHEMA_SQL = `
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
`;

export interface WorkspaceStore {
  db: ReturnType<typeof drizzle<typeof workspaceSchema>>;
  sqlite: Database.Database;
  path: string;
}

/**
 * Open (creating if needed) the workspace database at `dbPath`.
 *
 * `busy_timeout` matters here: the CLI and the dashboard can write the same file
 * from different processes, and without it SQLite would fail immediately with
 * `SQLITE_BUSY` instead of waiting for the other writer to finish.
 */
export function openWorkspaceDb(dbPath: string): WorkspaceStore {
  fs.mkdirSync(path.dirname(dbPath), { recursive: true });
  const sqlite = new Database(dbPath);
  sqlite.pragma("journal_mode = WAL");
  sqlite.pragma("synchronous = NORMAL");
  sqlite.pragma("foreign_keys = ON");
  sqlite.pragma("busy_timeout = 5000");
  sqlite.exec(SCHEMA_SQL);
  return { db: drizzle(sqlite, { schema: workspaceSchema }), sqlite, path: dbPath };
}

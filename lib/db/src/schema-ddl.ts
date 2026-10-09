/**
 * DDL for the legacy (Phase-1) activity database, kept next to the Drizzle
 * schema it mirrors.
 *
 * Why this exists: the schema used to be created by a manual `drizzle-kit push`
 * step that needs the development dependencies installed. A production host
 * that only installs `--prod` dependencies therefore could not start the API at
 * all. Creating the schema on open removes that step while staying
 * non-destructive: every statement is `IF NOT EXISTS`, no table is dropped, no
 * column is altered and no row is touched, so an existing database keeps its
 * data and any future column additions are still applied by a deliberate
 * migration rather than here.
 *
 * `artifacts/api-server/tests/schema.test.ts` compares these tables against the
 * Drizzle definitions column by column, so the two cannot drift silently.
 */
export const LEGACY_SCHEMA_SQL = `
CREATE TABLE IF NOT EXISTS notes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  tags TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS todos (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  description TEXT NOT NULL,
  done INTEGER NOT NULL DEFAULT 0,
  due_at INTEGER,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS snippets (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  language TEXT,
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS activity_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL,
  description TEXT NOT NULL,
  timestamp INTEGER NOT NULL,
  metadata TEXT
);
CREATE INDEX IF NOT EXISTS idx_activity_log_timestamp ON activity_log (timestamp DESC);
CREATE TABLE IF NOT EXISTS backup_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  source_path TEXT NOT NULL,
  dest_path TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS backup_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id INTEGER NOT NULL REFERENCES backup_jobs(id) ON DELETE CASCADE,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  files_total INTEGER,
  files_changed INTEGER,
  bytes_transferred INTEGER,
  status TEXT NOT NULL DEFAULT 'running',
  checksum_root TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_backup_snapshots_job ON backup_snapshots (job_id);
`;

/** Table names the API requires before it will accept traffic. */
export const LEGACY_TABLES = [
  "notes",
  "todos",
  "snippets",
  "activity_log",
  "backup_jobs",
  "backup_snapshots",
] as const;

/** Apply the schema above. Idempotent and safe to call on every startup. */
export function ensureLegacySchema(run: (sql: string) => unknown): void {
  run(LEGACY_SCHEMA_SQL);
}

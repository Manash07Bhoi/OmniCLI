import { drizzle } from "drizzle-orm/better-sqlite3";
import Database from "better-sqlite3";
import path from "path";
import os from "os";
import fs from "fs";
import * as schema from "./schema";
import { ensureLegacySchema } from "./schema-ddl";

const defaultDbPath = path.join(
  process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
  "omni",
  "omni.db"
);

const dbPath = process.env["DB_PATH"] ?? defaultDbPath;

// Ensure the directory exists before opening the database
const dbDir = path.dirname(dbPath);
fs.mkdirSync(dbDir, { recursive: true });

const sqlite = new Database(dbPath);

// Performance + correctness pragmas
sqlite.pragma("journal_mode = WAL");
sqlite.pragma("foreign_keys = ON");
sqlite.pragma("synchronous = NORMAL");

// Create the tables when they are missing so a fresh deployment works with
// runtime dependencies only. Non-destructive: see ./schema-ddl.ts.
ensureLegacySchema((sql) => sqlite.exec(sql));

export const db = drizzle(sqlite, { schema });

export * from "./schema";
export { LEGACY_SCHEMA_SQL, LEGACY_TABLES, ensureLegacySchema } from "./schema-ddl";
// The CLI-owned workspace store (notes/todos/snippets) — a separate SQLite file
// that both the CLI and the dashboard use. See ./workspace.ts for the contract.
export {
  openWorkspaceDb,
  workspaceNotesTable,
  workspaceTodosTable,
  workspaceSnippetsTable,
  workspaceSchema,
  type WorkspaceStore,
} from "./workspace";

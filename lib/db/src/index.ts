import { drizzle } from "drizzle-orm/better-sqlite3";
import Database from "better-sqlite3";
import path from "path";
import os from "os";
import fs from "fs";
import * as schema from "./schema";

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

export const db = drizzle(sqlite, { schema });

export * from "./schema";

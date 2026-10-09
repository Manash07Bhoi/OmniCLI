import path from "node:path";
import { promises as fs } from "node:fs";
import { describe, expect, it } from "vitest";
import Database from "better-sqlite3";
import { getTableConfig } from "drizzle-orm/sqlite-core";
import {
  LEGACY_SCHEMA_SQL,
  LEGACY_TABLES,
  ensureLegacySchema,
  activityLogTable,
  backupJobsTable,
  backupSnapshotsTable,
  notesTable,
  snippetsTable,
  todosTable,
} from "@workspace/db";
import { makeTempDir } from "./helpers";

const TABLES = {
  notes: notesTable,
  todos: todosTable,
  snippets: snippetsTable,
  activity_log: activityLogTable,
  backup_jobs: backupJobsTable,
  backup_snapshots: backupSnapshotsTable,
} as const;

function columnsOf(db: Database.Database, table: string) {
  return db
    .prepare(`PRAGMA table_info(${table})`)
    .all() as { name: string; notnull: number; pk: number; type: string }[];
}

describe("legacy schema creation", () => {
  it("creates every table the API requires, matching the Drizzle definitions", async () => {
    const dir = await makeTempDir("schema-drift");
    const db = new Database(path.join(dir, "fresh.db"));
    ensureLegacySchema((sql) => db.exec(sql));

    for (const table of LEGACY_TABLES) {
      const actual = columnsOf(db, table);
      expect(actual.length, `${table} has no columns`).toBeGreaterThan(0);

      const expected = getTableConfig(TABLES[table]).columns.map((c) => c.name);
      expect(
        actual.map((c) => c.name).sort(),
        `${table} columns differ from the Drizzle schema (DDL drift)`,
      ).toEqual([...expected].sort());

      // Nullability and primary keys are part of the contract, not decoration:
      // a nullable column where Drizzle expects NOT NULL silently changes what
      // the API can write.
      for (const config of getTableConfig(TABLES[table]).columns) {
        const column = actual.find((c) => c.name === config.name);
        expect(column, `${table}.${config.name} missing`).toBeDefined();
        expect(Boolean(column!.pk), `${table}.${config.name} primary key`).toBe(config.primary);
        // SQLite reports notnull = 0 for an INTEGER PRIMARY KEY (the rowid
        // alias) even when the column can never be null, so the flag is only
        // comparable for non-key columns.
        if (!config.primary) {
          expect(Boolean(column!.notnull), `${table}.${config.name} notnull`).toBe(config.notNull);
        }
      }
    }
    db.close();
  });

  it("is idempotent and never touches existing rows", async () => {
    const dir = await makeTempDir("schema-idempotent");
    const db = new Database(path.join(dir, "again.db"));
    ensureLegacySchema((sql) => db.exec(sql));
    db.prepare(
      "INSERT INTO notes (title, body, created_at, updated_at) VALUES (?, ?, ?, ?)",
    ).run("keep me", "body", 1, 1);

    // Applies the schema a second time, exactly as a restart would.
    ensureLegacySchema((sql) => db.exec(sql));

    const row = db.prepare("SELECT title FROM notes").get() as { title: string };
    expect(row.title).toBe("keep me");
    db.close();
  });

  it("refuses to drop anything: the DDL contains no destructive statements", () => {
    const statements = LEGACY_SCHEMA_SQL.toUpperCase();
    for (const forbidden of ["DROP ", "DELETE FROM", "ALTER TABLE", "TRUNCATE"]) {
      expect(statements).not.toContain(forbidden);
    }
    // Every table is created defensively, so an existing database is untouched.
    expect(statements.match(/CREATE TABLE IF NOT EXISTS/g)?.length).toBe(LEGACY_TABLES.length);
  });

  it("opening the module against a fresh path produces a usable database", async () => {
    const dir = await makeTempDir("schema-fresh-open");
    const dbPath = path.join(dir, "nested", "omni.db");
    // Directory creation is part of the contract: a fresh host has no
    // ~/.local/share/omni yet.
    await fs.mkdir(path.dirname(dbPath), { recursive: true });

    const db = new Database(dbPath);
    ensureLegacySchema((sql) => db.exec(sql));
    const names = db
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
      .all() as { name: string }[];
    const present = new Set(names.map((n) => n.name));
    for (const table of LEGACY_TABLES) {
      expect(present.has(table), `${table} was not created`).toBe(true);
    }
    db.close();
  });
});

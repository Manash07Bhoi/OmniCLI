import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import path from "node:path";
import Database from "better-sqlite3";
import { drizzle } from "drizzle-orm/better-sqlite3";
import { notesTable, todosTable, snippetsTable, openWorkspaceDb } from "@workspace/db";
import { cliJson, loadApp, makeTempDir, sandbox, uniqueName } from "./helpers";
import { migrateLegacyWorkspace } from "../src/lib/workspace-migration";
import { workspaceStore } from "../src/lib/workspace";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

interface CliNote {
  id: number;
  title: string;
  created_at: number;
}

describe("shared workspace store", () => {
  it("resolves the same workspace.db the CLI reports", async () => {
    const store = await workspaceStore();
    // The CLI is asked first (it honours a configured db_path); in the sandbox
    // that answer is $XDG_DATA_HOME/omni/workspace.db.
    const expected = path.join(sandbox().data, "omni", "workspace.db");
    expect(store.path).toBe(expected);

    const cli = await cliJson<{ db_path: string }>(["workspace", "stats"]);
    expect(cli.db_path).toBe(expected);
  });

  it("a note created through the API is visible to the CLI", async () => {
    const title = uniqueName("api-note");
    const created = await request(app).post("/api/workspace/notes").send({ title, body: "from the API" });
    expect(created.status).toBe(201);

    const notes = await cliJson<CliNote[]>(["workspace", "note", "list"]);
    const match = notes.find((n) => n.title === title);
    expect(match, "the CLI must see the note the API wrote").toBeTruthy();
    expect(match?.id).toBe(created.body.id);
  });

  it("a note created with the CLI is visible to the API", async () => {
    const title = uniqueName("cli-note");
    await cliJson<CliNote>(["workspace", "note", "new", title, "--body", "from the CLI"]);

    const listed = await request(app).get("/api/workspace/notes").query({ search: title });
    expect(listed.status).toBe(200);
    expect(listed.body.some((n: { title: string }) => n.title === title)).toBe(true);
  });

  it("stores timestamps in the CLI's unit (Unix seconds) and reads them back as ISO", async () => {
    const title = uniqueName("timestamp-note");
    const created = await request(app).post("/api/workspace/notes").send({ title });

    // Read the raw column: it must be seconds, because that is what the CLI's
    // schema declares — a millisecond value here would show up as a 1970 date
    // for every CLI user.
    const store = await workspaceStore();
    const row = store.sqlite
      .prepare("SELECT created_at, updated_at FROM notes WHERE id = ?")
      .get(created.body.id) as { created_at: number; updated_at: number };
    const nowSeconds = Math.floor(Date.now() / 1000);
    expect(Math.abs(row.created_at - nowSeconds)).toBeLessThan(10);
    expect(row.created_at).toBeLessThan(10_000_000_000); // seconds, not ms

    // And the API contract still returns ISO-8601.
    expect(Date.parse(created.body.createdAt)).not.toBeNaN();
  });

  it("sets busy_timeout so the CLI can write while the dashboard is open", async () => {
    const store = await workspaceStore();
    const [row] = store.sqlite.pragma("busy_timeout") as { timeout: number }[];
    expect(row.timeout).toBe(5000);
  });

  it("keeps /api/dashboard/stats consistent with the workspace store", async () => {
    const [stats, workspace] = await Promise.all([
      request(app).get("/api/dashboard/stats"),
      request(app).get("/api/workspace/stats"),
    ]);
    expect(stats.status).toBe(200);
    expect(workspace.status).toBe(200);
    expect(stats.body.notes).toBe(workspace.body.totalNotes);
    expect(stats.body.todos).toBe(workspace.body.totalTodos);
    expect(stats.body.snippets).toBe(workspace.body.totalSnippets);
  });
});

describe("legacy workspace migration", () => {
  /** A stand-in for the old dashboard database (millisecond timestamps). */
  function legacyDatabase(dbPath: string) {
    const sqlite = new Database(dbPath);
    sqlite.exec(`
      CREATE TABLE notes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        body TEXT NOT NULL DEFAULT '',
        tags TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );
      CREATE TABLE todos (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        description TEXT NOT NULL,
        done INTEGER NOT NULL DEFAULT 0,
        due_at INTEGER,
        created_at INTEGER NOT NULL
      );
      CREATE TABLE snippets (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        language TEXT,
        body TEXT NOT NULL,
        created_at INTEGER NOT NULL
      );
    `);
    return { sqlite, db: drizzle(sqlite, { schema: { notes: notesTable, todos: todosTable, snippets: snippetsTable } }) };
  }

  it("copies legacy rows once, converts ms→s, and never deletes the source", async () => {
    const dir = await makeTempDir("migration");
    const legacyPath = path.join(dir, "omni.db");
    const { sqlite: legacySqlite, db: legacyDb } = legacyDatabase(legacyPath);

    const createdAt = new Date("2024-03-01T10:00:00.000Z");
    legacyDb.insert(notesTable).values({ title: "Legacy note", body: "old", tags: "a,b", createdAt, updatedAt: createdAt }).run();
    legacyDb.insert(todosTable).values({ description: "Legacy todo", done: true, createdAt }).run();
    legacyDb.insert(snippetsTable).values({ name: "legacy-snippet", language: "rust", body: "fn main() {}", createdAt }).run();

    const target = openWorkspaceDb(path.join(dir, "workspace.db"));
    const first = await migrateLegacyWorkspace(legacyDb as never, target);
    expect(first).toEqual({ skipped: false, notes: 1, todos: 1, snippets: 1 });

    const note = target.sqlite.prepare("SELECT title, tags, created_at FROM notes").get() as {
      title: string;
      tags: string;
      created_at: number;
    };
    expect(note.title).toBe("Legacy note");
    expect(note.tags).toBe("a,b");
    expect(note.created_at).toBe(Math.floor(createdAt.getTime() / 1000));

    const todo = target.sqlite.prepare("SELECT description, done FROM todos").get() as {
      description: string;
      done: number;
    };
    expect(todo).toEqual({ description: "Legacy todo", done: 1 });

    // Re-running is a no-op and duplicates nothing.
    const second = await migrateLegacyWorkspace(legacyDb as never, target);
    expect(second.skipped).toBe(true);
    expect((target.sqlite.prepare("SELECT COUNT(*) AS n FROM notes").get() as { n: number }).n).toBe(1);

    // The old rows are still there: the migration copies, it does not move.
    expect((legacySqlite.prepare("SELECT COUNT(*) AS n FROM notes").get() as { n: number }).n).toBe(1);
    legacySqlite.close();
  });

  it("does not overwrite rows that already exist in the shared store", async () => {
    const dir = await makeTempDir("migration-conflict");
    const { db: legacyDb } = legacyDatabase(path.join(dir, "omni.db"));
    const createdAt = new Date("2024-01-01T00:00:00.000Z");
    legacyDb.insert(notesTable).values({ id: 1, title: "Legacy", createdAt, updatedAt: createdAt }).run();

    const target = openWorkspaceDb(path.join(dir, "workspace.db"));
    target.sqlite
      .prepare("INSERT INTO notes (id, title, body, created_at, updated_at) VALUES (1, 'Newer', '', ?, ?)")
      .run(Math.floor(createdAt.getTime() / 1000), Math.floor(createdAt.getTime() / 1000));

    const result = await migrateLegacyWorkspace(legacyDb as never, target);
    expect(result.notes).toBe(0);
    const kept = target.sqlite.prepare("SELECT title FROM notes WHERE id = 1").get() as { title: string };
    expect(kept.title).toBe("Newer");
  });
});

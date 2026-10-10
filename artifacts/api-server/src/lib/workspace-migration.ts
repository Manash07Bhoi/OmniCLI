import { sql } from "drizzle-orm";
import { db as defaultLegacyDb, notesTable, todosTable, snippetsTable } from "@workspace/db";
import { workspaceNotesTable, workspaceTodosTable, workspaceSnippetsTable } from "@workspace/db";
import type { WorkspaceStore } from "@workspace/db";
import { logger } from "./logger";

const MARKER = "legacy_dashboard_workspace_rows";

type LegacyDb = typeof defaultLegacyDb;

export interface MigrationResult {
  skipped: boolean;
  notes: number;
  todos: number;
  snippets: number;
}

/**
 * One-time, non-destructive copy of workspace rows that the dashboard used to
 * keep in its own database.
 *
 * Earlier versions of the dashboard stored notes/todos/snippets in `omni.db`
 * with millisecond timestamps, while the CLI kept its own `workspace.db` with
 * second timestamps. Both stores are now the same file, so existing dashboard
 * rows are copied across (ms → s) exactly once. Rows are never deleted from the
 * old database: it stays as a read-only backup, and re-running the migration is
 * a no-op.
 */
export async function migrateLegacyWorkspace(
  legacy: LegacyDb,
  target: WorkspaceStore
): Promise<MigrationResult> {
  await legacy.run(
    sql`CREATE TABLE IF NOT EXISTS workspace_migration (
          state TEXT PRIMARY KEY,
          migrated_at INTEGER NOT NULL
        )`
  );

  const done = await legacy.all<{ state: string }>(
    sql`SELECT state FROM workspace_migration WHERE state = ${MARKER}`
  );
  if (done.length > 0) {
    return { skipped: true, notes: 0, todos: 0, snippets: 0 };
  }

  const seconds = (d: Date | null): Date | null => (d ? new Date(Math.floor(d.getTime() / 1000) * 1000) : null);

  const legacyNotes = await legacy.select().from(notesTable);
  const legacyTodos = await legacy.select().from(todosTable);
  const legacySnippets = await legacy.select().from(snippetsTable);

  let notes = 0;
  for (const n of legacyNotes) {
    const exists = await target.db
      .select({ id: workspaceNotesTable.id })
      .from(workspaceNotesTable)
      .where(sql`${workspaceNotesTable.id} = ${n.id}`);
    if (exists.length > 0) continue;
    await target.db.insert(workspaceNotesTable).values({
      id: n.id,
      title: n.title,
      body: n.body,
      tags: n.tags ?? null,
      createdAt: seconds(n.createdAt) ?? new Date(),
      updatedAt: seconds(n.updatedAt) ?? new Date(),
    });
    notes += 1;
  }

  let todos = 0;
  for (const t of legacyTodos) {
    const exists = await target.db
      .select({ id: workspaceTodosTable.id })
      .from(workspaceTodosTable)
      .where(sql`${workspaceTodosTable.id} = ${t.id}`);
    if (exists.length > 0) continue;
    await target.db.insert(workspaceTodosTable).values({
      id: t.id,
      description: t.description,
      done: t.done,
      dueAt: seconds(t.dueAt),
      createdAt: seconds(t.createdAt) ?? new Date(),
    });
    todos += 1;
  }

  let snippets = 0;
  for (const s of legacySnippets) {
    const clash = await target.db
      .select({ id: workspaceSnippetsTable.id })
      .from(workspaceSnippetsTable)
      .where(sql`${workspaceSnippetsTable.id} = ${s.id} OR ${workspaceSnippetsTable.name} = ${s.name}`);
    if (clash.length > 0) continue; // same snippet already in the shared store
    await target.db.insert(workspaceSnippetsTable).values({
      id: s.id,
      name: s.name,
      language: s.language ?? null,
      body: s.body,
      createdAt: seconds(s.createdAt) ?? new Date(),
    });
    snippets += 1;
  }

  await legacy.run(
    sql`INSERT INTO workspace_migration (state, migrated_at) VALUES (${MARKER}, ${Date.now()})`
  );

  const result = { skipped: false, notes, todos, snippets };
  if (notes + todos + snippets > 0) {
    logger.info(result, "copied legacy dashboard workspace rows into the shared workspace store");
  }
  return result;
}

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { sql } from "drizzle-orm";
import { db } from "@workspace/db";
import app from "./app";
import { logger } from "./lib/logger";
import { workspaceStore } from "./lib/workspace";
import { migrateLegacyWorkspace } from "./lib/workspace-migration";

const moduleDir = path.dirname(fileURLToPath(import.meta.url));

/**
 * Load a `.env` file when one exists (developer convenience). Real deployments
 * set environment variables directly, and this never overrides them.
 */
function loadDotEnv(): void {
  const explicit = process.env["ENV_FILE"];
  const candidates = explicit
    ? [path.resolve(explicit)]
    : [
        path.resolve(moduleDir, "..", ".env"), // artifacts/api-server/.env
        path.resolve(moduleDir, "..", "..", "..", ".env"), // repository root
      ];
  const envFile = candidates.find((p) => fs.existsSync(p));
  if (!envFile) return;
  if (typeof process.loadEnvFile !== "function") {
    logger.warn({ envFile }, "environment file found but this Node version cannot load it");
    return;
  }
  process.loadEnvFile(envFile);
  logger.info({ envFile }, "loaded environment file");
}

/**
 * Tripwire after the automatic schema creation.
 *
 * Opening the database already applies the idempotent DDL (lib/db
 * `ensureLegacySchema`), so under normal operation every required table
 * exists by the time this runs. If any are still missing, the environment
 * itself is broken — typically an unwritable DB_PATH or a stale file that
 * is not a valid database — and the server must not pretend to be healthy.
 */
async function verifySchema(): Promise<void> {
  const required = ["notes", "todos", "snippets", "activity_log", "backup_jobs", "backup_snapshots"];
  const rows = await db.all<{ name: string }>(
    sql`SELECT name FROM sqlite_master WHERE type = 'table'`
  );
  const present = new Set(rows.map((r) => r.name));
  const missing = required.filter((t) => !present.has(t));
  if (missing.length > 0) {
    throw new Error(
      `The database at ${process.env["DB_PATH"] ?? "its default location"} is missing ` +
        `${missing.join(", ")} even after the automatic schema creation. ` +
        `Check that DB_PATH points to a writable location containing a valid SQLite database.`
    );
  }
}

async function start(): Promise<void> {
  loadDotEnv();

  // Sensible development default: the dashboard's dev proxy targets 8080. A
  // deployment that needs another port sets PORT explicitly.
  const port = Number(process.env["PORT"] ?? 8080);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`Invalid PORT value: "${process.env["PORT"]}"`);
  }
  const host = process.env["HOST"] ?? "0.0.0.0";

  await verifySchema();
  logger.info({ port, host, nodeEnv: process.env["NODE_ENV"] ?? "development" }, "starting API server");

  // One-time, non-destructive copy of rows that older dashboard versions kept
  // in their own database. A failure here must not stop the server: the shared
  // workspace store still works, only the legacy rows stay behind.
  try {
    const store = await workspaceStore();
    const result = await migrateLegacyWorkspace(db, store);
    if (!result.skipped && result.notes + result.todos + result.snippets > 0) {
      logger.info(result, "migrated legacy workspace rows");
    }
  } catch (err) {
    logger.error({ err }, "legacy workspace migration failed (server continues)");
  }

  const server = app.listen(port, host, () => {
    logger.info({ port, host }, "Server listening");
  });

  const shutdown = (signal: string): void => {
    logger.info({ signal }, "shutting down");
    server.close(() => process.exit(0));
    // Do not hang forever on keep-alive connections.
    setTimeout(() => process.exit(0), 5_000).unref();
  };
  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));
}

start().catch((err: unknown) => {
  logger.error({ err }, "failed to start the API server");
  process.exit(1);
});

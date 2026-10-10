import os from "node:os";
import path from "node:path";
import { workspaceNotesTable, workspaceTodosTable, workspaceSnippetsTable } from "@workspace/db";
import { openWorkspaceDb, type WorkspaceStore } from "@workspace/db";
import { omniJsonOk } from "./exec";
import { logger } from "./logger";

/**
 * Where the workspace store lives.
 *
 * The CLI owns the rule: `platform::data_dir()/workspace.db`, i.e.
 * `dirs::data_local_dir()/omni/workspace.db`. This mirrors `dirs` exactly per
 * platform rather than guessing, and the CLI itself is asked first so a custom
 * `workspace.db_path` in the user's config is honoured too.
 */
export function defaultWorkspaceDbPath(): string {
  const localDataDir =
    process.platform === "win32"
      ? (process.env["LOCALAPPDATA"] ?? path.join(os.homedir(), "AppData", "Local"))
      : process.platform === "darwin"
        ? path.join(os.homedir(), "Library", "Application Support")
        : (process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"));
  return path.join(localDataDir, "omni", "workspace.db");
}

/**
 * Ask the CLI for the database it actually uses (`workspace stats --json`
 * reports `db_path`). Returns null when the CLI cannot answer — for example
 * when it is not installed — so callers can fall back to the default path.
 */
export async function cliWorkspaceDbPath(): Promise<string | null> {
  try {
    const info = await omniJsonOk<{ db_path?: string }>(["workspace", "stats"]);
    const reported = info?.db_path;
    return typeof reported === "string" && reported.length > 0 ? reported : null;
  } catch {
    return null;
  }
}

let storePromise: Promise<WorkspaceStore> | null = null;

/**
 * Memoised handle to the workspace store. Resolution order:
 *   1. `OMNI_WORKSPACE_DB` (deployments, tests)
 *   2. the path the CLI reports (honours a configured `workspace.db_path`)
 *   3. the platform default
 */
export function workspaceStore(): Promise<WorkspaceStore> {
  storePromise ??= (async () => {
    const override = process.env["OMNI_WORKSPACE_DB"];
    const dbPath = override
      ? path.resolve(override)
      : ((await cliWorkspaceDbPath()) ?? defaultWorkspaceDbPath());
    const store = openWorkspaceDb(dbPath);
    logger.info({ workspaceDb: dbPath }, "workspace store opened");
    return store;
  })();
  return storePromise;
}

export { workspaceNotesTable, workspaceTodosTable, workspaceSnippetsTable };

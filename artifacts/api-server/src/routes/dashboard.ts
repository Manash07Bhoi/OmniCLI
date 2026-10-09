import { Router, type IRouter } from "express";
import { promises as fs, type Dirent } from "node:fs";
import path from "node:path";
import { db } from "@workspace/db";
import { backupJobsTable, activityLogTable } from "@workspace/db";
import { workspaceStore, workspaceNotesTable, workspaceTodosTable, workspaceSnippetsTable } from "../lib/workspace";
import { count, desc } from "drizzle-orm";
import { omniExec, omniJsonOk, isOmniUnavailable } from "../lib/exec";
import {
  GetDashboardStatsResponse,
  GetDashboardActivityResponse,
  GetDashboardActivityQueryParams,
  GetModuleStatusResponse,
} from "@workspace/api-zod";
import { invalidRequest } from "../lib/validation";

const router: IRouter = Router();

// ── Filesystem stats (real scan of user home, capped for speed) ──────────────

interface FsStats {
  totalFiles: number;
  totalDirs: number;
  totalSizeBytes: number;
}

async function scanFsStats(rootPath: string, maxFiles = 5000): Promise<FsStats> {
  let totalFiles = 0;
  let totalDirs = 0;
  let totalSizeBytes = 0;

  const SKIP_DIRS = new Set([
    "node_modules", ".git", "target", ".cache", "__pycache__",
    ".npm", ".pnpm-store", "dist", "build", ".next",
  ]);

  async function walk(dir: string, depth: number): Promise<void> {
    if (depth > 5 || totalFiles + totalDirs > maxFiles) return;
    let entries: Dirent<string>[];
    try {
      entries = await fs.readdir(dir, { withFileTypes: true }) as Dirent<string>[];
    } catch {
      return;
    }
    for (const e of entries) {
      if (SKIP_DIRS.has(e.name) || e.name.startsWith(".")) continue;
      const full = path.join(dir, e.name);
      try {
        if (e.isDirectory()) {
          totalDirs++;
          await walk(full, depth + 1);
        } else if (e.isFile()) {
          const stat = await fs.stat(full).catch(() => null);
          if (stat) {
            totalFiles++;
            totalSizeBytes += stat.size;
          }
        }
      } catch {
        // skip inaccessible entries
      }
    }
  }

  await walk(rootPath, 0);
  return { totalFiles, totalDirs, totalSizeBytes };
}

// ── Search index stats (query SQLite if available) ────────────────────────────

interface IndexSummary {
  indexedFiles: number;
  lastIndexedAt: string | null;
}

/**
 * Index statistics come from the CLI, which owns the search database — this
 * avoids a second guess at the database path or schema.
 */
async function getIndexSummary(): Promise<IndexSummary> {
  const info = await omniJsonOk<{ db_path?: string; total_entries?: number }>(["search", "info"]);
  let lastIndexedAt: string | null = null;
  if (info.db_path) {
    const stat = await fs.stat(info.db_path).catch(() => null);
    if (stat) lastIndexedAt = stat.mtime.toISOString();
  }
  return { indexedFiles: info.total_entries ?? 0, lastIndexedAt };
}

// ── Real command surface (derived from the binary, not a roadmap) ─────────────

interface CliModule {
  name: string;
  phase: number;
  status: "active";
  description: string;
  verbCount: number;
  color: string;
}

/** Parse a clap `Commands:` block into `[name, description]` pairs. */
function parseCommandList(helpText: string): { name: string; description: string }[] {
  const lines = helpText.split("\n");
  const start = lines.findIndex((l) => l.trim() === "Commands:");
  if (start < 0) return [];

  const out: { name: string; description: string }[] = [];
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (!line.trim()) break;
    const m = /^\s{2}([A-Za-z0-9_-]+)\s{2,}(.*)$/.exec(line);
    if (m) {
      out.push({ name: m[1] as string, description: (m[2] ?? "").trim() });
    } else if (out.length > 0) {
      // Wrapped description line.
      const last = out[out.length - 1];
      if (last) last.description = `${last.description} ${line.trim()}`.trim();
    }
  }
  return out;
}

const MODULE_COLORS = [
  "#2ECC71", "#3498DB", "#9B59B6", "#E67E22", "#E74C3C",
  "#F39C12", "#1ABC9C", "#7F8C8D", "#8E44AD", "#2980B9",
];

let moduleCache: { at: number; modules: CliModule[] } | null = null;
const MODULE_CACHE_MS = 60_000;

/**
 * The module list is exactly the set of command groups the installed binary
 * exposes, with verb counts parsed from the same help output. Nothing is listed
 * that the binary cannot do.
 */
async function loadCliModules(): Promise<CliModule[]> {
  if (moduleCache && Date.now() - moduleCache.at < MODULE_CACHE_MS) {
    return moduleCache.modules;
  }

  const top = await omniExec(["--help"], 10_000);
  if (!top.ok) {
    throw new Error(top.stderr.trim() || "omnicli --help failed");
  }

  const groups = parseCommandList(top.stdout).filter((g) => g.name !== "help");
  const modules: CliModule[] = [];

  for (const [index, group] of groups.entries()) {
    let verbCount = 1; // the group itself is one command
    const groupHelp = await omniExec([group.name, "--help"], 10_000);
    if (groupHelp.ok) {
      const subcommands = parseCommandList(groupHelp.stdout).filter((c) => c.name !== "help");
      if (subcommands.length > 0) verbCount = subcommands.length;
    }
    modules.push({
      name: group.name,
      phase: 1,
      status: "active",
      description: group.description || `omnicli ${group.name}`,
      verbCount,
      color: MODULE_COLORS[index % MODULE_COLORS.length] as string,
    });
  }

  moduleCache = { at: Date.now(), modules };
  return modules;
}

// ── Dashboard stats endpoint ─────────────────────────────────────────────────

router.get("/dashboard/stats", async (req, res): Promise<void> => {
  let index: IndexSummary;
  let modules: CliModule[];
  let conversionFormats: number;
  try {
    [index, modules, conversionFormats] = await Promise.all([
      getIndexSummary(),
      loadCliModules(),
      omniJsonOk<unknown[]>(["convert", "list"]).then((pairs) => pairs.length),
    ]);
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(502).json({ error: `Could not read CLI state: ${(err as Error).message}` });
    return;
  }

  // Workspace counts are read from the CLI's own store (`workspace.db`) so the
  // dashboard can never disagree with `omni workspace stats`.
  const { db: wdb } = await workspaceStore();
  const [
    [noteCount],
    [todoCount],
    [snippetCount],
    [jobCount],
    fsStats,
  ] = await Promise.all([
    wdb.select({ count: count() }).from(workspaceNotesTable),
    wdb.select({ count: count() }).from(workspaceTodosTable),
    wdb.select({ count: count() }).from(workspaceSnippetsTable),
    db.select({ count: count() }).from(backupJobsTable),
    scanFsStats(process.env["HOME"] ?? process.cwd()).catch(() => ({ totalFiles: 0, totalDirs: 0, totalSizeBytes: 0 })),
  ]);

  const stats = GetDashboardStatsResponse.parse({
    totalFiles:       fsStats.totalFiles,
    totalSizeBytes:   fsStats.totalSizeBytes,
    indexedFiles:     index.indexedFiles,
    backupJobs:       jobCount?.count ?? 0,
    notes:            noteCount?.count ?? 0,
    todos:            todoCount?.count ?? 0,
    snippets:         snippetCount?.count ?? 0,
    conversionFormats,
    modulesActive:    modules.length,
    lastIndexedAt:    index.lastIndexedAt,
  });

  res.json(stats);
});

router.get("/dashboard/activity", async (req, res): Promise<void> => {
  const parsed = GetDashboardActivityQueryParams.safeParse(req.query);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }
  const limit = parsed.data.limit ?? 20;

  const rows = await db
    .select()
    .from(activityLogTable)
    .orderBy(desc(activityLogTable.timestamp))
    .limit(limit);

  const items = rows.map((r) => ({
    id: r.id,
    type: r.type as
      | "search"
      | "backup"
      | "convert"
      | "note"
      | "todo"
      | "snippet"
      | "file_find",
    description: r.description,
    timestamp: r.timestamp.toISOString(),
    metadata: r.metadata ?? null,
  }));

  res.json(GetDashboardActivityResponse.parse(items));
});

router.get("/dashboard/module-status", async (_req, res): Promise<void> => {
  try {
    const modules = await loadCliModules();
    res.json(GetModuleStatusResponse.parse(modules));
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(502).json({ error: `Could not read module status: ${(err as Error).message}` });
  }
});

export default router;

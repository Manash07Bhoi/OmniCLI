import { Router, type IRouter } from "express";
import { promises as fs, type Dirent } from "node:fs";
import path from "node:path";
import os from "node:os";
import { db } from "@workspace/db";
import {
  notesTable,
  todosTable,
  snippetsTable,
  backupJobsTable,
  activityLogTable,
} from "@workspace/db";
import { count, desc } from "drizzle-orm";
import {
  GetDashboardStatsResponse,
  GetDashboardActivityResponse,
  GetDashboardActivityQueryParams,
  GetModuleStatusResponse,
} from "@workspace/api-zod";

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

async function getIndexedFileCount(): Promise<number> {
  const dbPath = path.join(
    process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
    "omni",
    "search.db",
  );
  try {
    await fs.access(dbPath);
    // Try to get count from search_index table using raw sqlite3 CLI
    const { execFile } = await import("node:child_process");
    const { promisify } = await import("node:util");
    const execAsync = promisify(execFile);
    const { stdout } = await execAsync("sqlite3", [dbPath, "SELECT COUNT(*) FROM search_index;"], {
      timeout: 3000,
    });
    return parseInt(stdout.trim(), 10) || 0;
  } catch {
    return 0;
  }
}

// ── Dashboard stats endpoint ─────────────────────────────────────────────────

router.get("/dashboard/stats", async (req, res): Promise<void> => {
  const [
    [noteCount],
    [todoCount],
    [snippetCount],
    [jobCount],
    fsStats,
    indexedFiles,
  ] = await Promise.all([
    db.select({ count: count() }).from(notesTable),
    db.select({ count: count() }).from(todosTable),
    db.select({ count: count() }).from(snippetsTable),
    db.select({ count: count() }).from(backupJobsTable),
    scanFsStats(process.env["HOME"] ?? process.cwd()).catch(() => ({ totalFiles: 0, totalDirs: 0, totalSizeBytes: 0 })),
    getIndexedFileCount().catch(() => 0),
  ]);

  const stats = GetDashboardStatsResponse.parse({
    totalFiles:       fsStats.totalFiles,
    totalSizeBytes:   fsStats.totalSizeBytes,
    indexedFiles,
    backupJobs:       jobCount?.count ?? 0,
    notes:            noteCount?.count ?? 0,
    todos:            todoCount?.count ?? 0,
    snippets:         snippetCount?.count ?? 0,
    conversionFormats: 16,
    modulesActive:    4,
    lastIndexedAt:    null,
  });

  res.json(stats);
});

router.get("/dashboard/activity", async (req, res): Promise<void> => {
  const parsed = GetDashboardActivityQueryParams.safeParse(req.query);
  const limit = parsed.success ? (parsed.data.limit ?? 20) : 20;

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
  const modules = GetModuleStatusResponse.parse([
    {
      name: "file",
      phase: 1,
      status: "active",
      description: "Find, copy, move, hash, encrypt, sync files",
      verbCount: 10,
      color: "#2ECC71",
    },
    {
      name: "search",
      phase: 1,
      status: "active",
      description: "Full-text search across indexed files via SQLite FTS5",
      verbCount: 3,
      color: "#3498DB",
    },
    {
      name: "archive",
      phase: 1,
      status: "active",
      description: "Create, extract, list, convert archives (ZIP/TAR/GZ/XZ/BZ2)",
      verbCount: 4,
      color: "#9B59B6",
    },
    {
      name: "convert",
      phase: 1,
      status: "active",
      description: "16 format pairs — CSV, JSON, YAML, TOML, images, Markdown",
      verbCount: 2,
      color: "#E67E22",
    },
    {
      name: "dev",
      phase: 2,
      status: "active",
      description: "Hash, JSON, Base64, UUID, regex, JWT decode",
      verbCount: 6,
      color: "#E74C3C",
    },
    {
      name: "workspace",
      phase: 2,
      status: "active",
      description: "Notes, todos, snippets stored in SQLite",
      verbCount: 3,
      color: "#F39C12",
    },
    {
      name: "backup",
      phase: 2,
      status: "active",
      description: "Incremental backup with BLAKE3 content-hash deduplication",
      verbCount: 4,
      color: "#1ABC9C",
    },
    {
      name: "config",
      phase: 2,
      status: "active",
      description: "Schema-aware config management: JSON, YAML, TOML, XML, INI",
      verbCount: 4,
      color: "#7F8C8D",
    },
    {
      name: "plugin",
      phase: 3,
      status: "phase3",
      description: "WASM-sandboxed plugin system with capability manifest",
      verbCount: 4,
      color: "#8E44AD",
    },
    {
      name: "new",
      phase: 3,
      status: "phase3",
      description: "Project scaffolding for Rust, Go, React, Java, CLI",
      verbCount: 5,
      color: "#2980B9",
    },
    {
      name: "shell",
      phase: 3,
      status: "phase3",
      description: "Session-aware REPL layer — history, bookmarks, variables",
      verbCount: 1,
      color: "#27AE60",
    },
    {
      name: "install",
      phase: 3,
      status: "phase3",
      description: "Self-update and optional feature module package manager",
      verbCount: 3,
      color: "#D35400",
    },
  ]);

  res.json(modules);
});

export default router;

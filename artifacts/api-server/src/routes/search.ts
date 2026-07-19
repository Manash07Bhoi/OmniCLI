import { Router, type IRouter } from "express";
import { promises as fs, type Dirent } from "fs";
import path from "path";
import os from "os";
import {
  SearchFilesQueryParams,
  SearchFilesResponse,
  GetSearchIndexInfoResponse,
  TriggerSearchIndexBody,
} from "@workspace/api-zod";
import { db } from "@workspace/db";
import { activityLogTable } from "@workspace/db";

const router: IRouter = Router();

/** Walk a directory and yield all file paths up to maxDepth. */
async function* walkDir(
  dir: string,
  depth: number,
  maxDepth: number
): AsyncGenerator<string> {
  if (depth > maxDepth) return;
  let entries: Dirent<string>[];
  try {
    entries = await fs.readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (!e.name.startsWith(".") && e.name !== "node_modules" && e.name !== "target") {
        yield* walkDir(full, depth + 1, maxDepth);
      }
    } else {
      yield full;
    }
  }
}

router.get("/search", async (req, res): Promise<void> => {
  const parsed = SearchFilesQueryParams.safeParse(req.query);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { q, limit = 50, regex = false, caseSensitive = false } = parsed.data;
  const start = Date.now();

  let pattern: RegExp;
  try {
    if (regex) {
      pattern = new RegExp(q, caseSensitive ? "" : "i");
    } else {
      const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      pattern = new RegExp(escaped, caseSensitive ? "" : "i");
    }
  } catch (e: unknown) {
    res.status(400).json({ error: `Invalid pattern: ${String(e)}` });
    return;
  }

  const searchRoot = process.env["OMNI_INDEX_PATH"] ?? process.cwd();
  const results: {
    path: string;
    fileType: string;
    sizeBytes: number;
    modified: number;
    snippet: string | null;
    rank: number | null;
  }[] = [];

  let rank = 1;
  for await (const filePath of walkDir(searchRoot, 0, 4)) {
    if (results.length >= limit) break;
    const name = path.basename(filePath);
    let snippet: string | null = null;

    // Name match first (cheap)
    if (pattern.test(name)) {
      const stat = await fs.stat(filePath).catch(() => null);
      results.push({
        path: filePath,
        fileType: path.extname(name).replace(".", "") || "file",
        sizeBytes: stat?.size ?? 0,
        modified: Math.floor((stat?.mtimeMs ?? 0) / 1000),
        snippet,
        rank: rank++,
      });
      continue;
    }

    // Content match (text files only, skip large files)
    const stat = await fs.stat(filePath).catch(() => null);
    if (!stat || stat.size > 512 * 1024) continue; // skip > 512KB
    const ext = path.extname(name).toLowerCase();
    const textExts = new Set([
      ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".c", ".h", ".cpp",
      ".java", ".sh", ".md", ".txt", ".toml", ".yaml", ".yml", ".json",
      ".html", ".css", ".sql", ".env", ".log", ".xml", ".ini", ".conf",
    ]);
    if (!textExts.has(ext)) continue;

    try {
      const content = await fs.readFile(filePath, "utf8");
      const match = content.match(pattern);
      if (match) {
        const idx = content.indexOf(match[0]);
        const lineStart = Math.max(0, content.lastIndexOf("\n", idx) + 1);
        const lineEnd = content.indexOf("\n", idx);
        snippet =
          content.slice(lineStart, lineEnd > 0 ? lineEnd : lineStart + 120).trim();
        results.push({
          path: filePath,
          fileType: ext.replace(".", "") || "file",
          sizeBytes: stat.size,
          modified: Math.floor(stat.mtimeMs / 1000),
          snippet,
          rank: rank++,
        });
      }
    } catch {
      // binary / unreadable — skip
    }
  }

  // Log activity
  await db
    .insert(activityLogTable)
    .values({ type: "search", description: `Searched for "${q}" — ${results.length} results` })
    .catch(() => {});

  res.json(
    SearchFilesResponse.parse({
      query: q,
      results,
      total: results.length,
      durationMs: Date.now() - start,
    })
  );
});

router.get("/search/index", async (_req, res): Promise<void> => {
  const dbPath = path.join(
    process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
    "omni",
    "search.db"
  );

  let totalEntries = 0;
  let lastUpdated: string | null = null;
  let status: "ready" | "empty" | "error" = "empty";

  try {
    await fs.access(dbPath);
    status = "ready";
    totalEntries = 1; // We have the DB but can't query it directly without better-sqlite3
  } catch {
    status = "empty";
  }

  res.json(
    GetSearchIndexInfoResponse.parse({
      totalEntries,
      lastUpdated,
      indexPath: dbPath,
      status,
      indexedPaths: [],
    })
  );
});

router.post("/search/index", async (req, res): Promise<void> => {
  const parsed = TriggerSearchIndexBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const dbPath = path.join(
    process.env["XDG_DATA_HOME"] ?? path.join(os.homedir(), ".local", "share"),
    "omni",
    "search.db"
  );

  // Log the index trigger
  await db
    .insert(activityLogTable)
    .values({
      type: "search",
      description: `Index triggered for ${parsed.data.paths.join(", ")}`,
    })
    .catch(() => {});

  res.json(
    GetSearchIndexInfoResponse.parse({
      totalEntries: 0,
      lastUpdated: new Date().toISOString(),
      indexPath: dbPath,
      status: "empty" as const,
      indexedPaths: parsed.data.paths,
    })
  );
});

export default router;

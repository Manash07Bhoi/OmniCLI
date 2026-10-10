import { Router, type IRouter } from "express";
import { promises as fs, type Dirent } from "fs";
import path from "path";
import {
  SearchFilesQueryParams,
  SearchFilesResponse,
  GetSearchIndexInfoResponse,
  TriggerSearchIndexBody,
  TriggerSearchIndexResponse,
} from "@workspace/api-zod";
import { db } from "@workspace/db";
import { activityLogTable } from "@workspace/db";
import { omniJsonOk, isOmniUnavailable } from "../lib/exec";
import { invalidRequest, missingRequiredParams } from "../lib/validation";

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
  // `q` is required in the OpenAPI spec; `zod.coerce.string()` would coerce a
  // missing value to "undefined" and search for that literal word.
  const missing = missingRequiredParams(req.query as Record<string, unknown>, ["q"]);
  if (missing.length > 0) {
    invalidRequest(res, {
      issues: missing.map((name) => ({ path: [name], message: "Required" })),
    });
    return;
  }
  const parsed = SearchFilesQueryParams.safeParse(req.query);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }

  const { q, limit = 50, regex = false, caseSensitive = false } = parsed.data;
  const start = Date.now();

  let pattern: RegExp;
  try {
    // `m` keeps ^/$ anchored per line, matching the CLI's line-oriented search
    // (a plain `^foo$` pattern would otherwise never match file content).
    const flags = `m${caseSensitive ? "" : "i"}`;
    if (regex) {
      pattern = new RegExp(q, flags);
    } else {
      const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      pattern = new RegExp(escaped, flags);
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

/** `omnicli search info --json` payload. */
interface SearchInfoOutput {
  db_path?: string;
  status?: string;
  total_entries?: number;
}

/** `omnicli search index --json` payload. */
interface SearchIndexOutput {
  files_indexed?: number;
  files_skipped?: number;
  content_docs_indexed?: number;
}

async function readIndexInfo() {
  const info = await omniJsonOk<SearchInfoOutput>(["search", "info"]);

  let lastUpdated: string | null = null;
  if (info.db_path) {
    // Real modification time of the index database.
    const stat = await fs.stat(info.db_path).catch(() => null);
    if (stat) lastUpdated = stat.mtime.toISOString();
  }

  return {
    totalEntries: info.total_entries ?? 0,
    lastUpdated,
    indexPath: info.db_path ?? "",
    status: (info.status === "ready" ? "ready" : "empty") as "ready" | "empty",
  };
}

router.get("/search/index", async (_req, res): Promise<void> => {
  try {
    res.json(GetSearchIndexInfoResponse.parse(await readIndexInfo()));
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(502).json({ error: `Could not read index info: ${(err as Error).message}` });
  }
});

router.post("/search/index", async (req, res): Promise<void> => {
  const parsed = TriggerSearchIndexBody.safeParse(req.body);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }

  const { paths } = parsed.data;

  const missing: string[] = [];
  for (const p of paths) {
    const stat = await fs.stat(p).catch(() => null);
    if (!stat?.isDirectory()) missing.push(p);
  }
  if (missing.length > 0) {
    res.status(400).json({ error: `Not a directory: ${missing.join(", ")}` });
    return;
  }

  try {
    // Index each requested path with the real indexer, then report the index's
    // actual state instead of an optimistic placeholder.
    const results: SearchIndexOutput[] = [];
    for (const p of paths) {
      results.push(await omniJsonOk<SearchIndexOutput>(["search", "index", p], 300_000));
    }

    const info = await readIndexInfo();

    await db
      .insert(activityLogTable)
      .values({
        type: "search",
        description:
          `Indexed ${paths.join(", ")} — ` +
          `${results.reduce((a, r) => a + (r.files_indexed ?? 0), 0)} files`,
      })
      .catch(() => {});

    res.json(
      TriggerSearchIndexResponse.parse({
        totalEntries: info.totalEntries,
        lastUpdated: new Date().toISOString(),
        indexPath: info.indexPath,
        status: info.status,
        indexedPaths: paths,
      })
    );
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(502).json({ error: `Indexing failed: ${(err as Error).message}` });
  }
});

export default router;

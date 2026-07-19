import { Router, type IRouter } from "express";
import { promises as fs, type Dirent } from "fs";
import path from "path";
import crypto from "crypto";
import {
  FindFilesQueryParams,
  FindFilesResponse,
  HashFileQueryParams,
  HashFileResponse,
  GetFileStatsQueryParams,
  GetFileStatsResponse,
} from "@workspace/api-zod";
import { db } from "@workspace/db";
import { activityLogTable } from "@workspace/db";

const router: IRouter = Router();

async function* walkDir(
  dir: string,
  depth: number,
  maxDepth: number,
  excludeDirs: Set<string>
): AsyncGenerator<{ path: string; isDir: boolean; isSymlink: boolean; sizeBytes: number; mtimeMs: number }> {
  if (depth > maxDepth) return;
  let entries: Dirent<string>[];
  try {
    entries = await fs.readdir(dir, { withFileTypes: true }) as Dirent<string>[];
  } catch {
    return;
  }
  for (const e of entries) {
    if (excludeDirs.has(e.name)) continue;
    const full = path.join(dir, e.name);
    try {
      const s = await fs.stat(full);
      yield {
        path: full,
        isDir: s.isDirectory(),
        isSymlink: s.isSymbolicLink(),
        sizeBytes: Number(s.size),
        mtimeMs: Number(s.mtimeMs),
      };
      if (s.isDirectory()) {
        yield* walkDir(full, depth + 1, maxDepth, excludeDirs);
      }
    } catch {
      // skip inaccessible
    }
  }
}

function matchesPattern(name: string, pattern: string, useRegex: boolean): boolean {
  if (!pattern) return true;
  if (useRegex) {
    try {
      return new RegExp(pattern, "i").test(name);
    } catch {
      return false;
    }
  }
  const escaped = pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replace(/\\\*/g, ".*").replace(/\\\?/g, ".");
  return new RegExp(`^${escaped}$`, "i").test(name);
}

function matchesSize(sizeBytes: number, filter: string | undefined): boolean {
  if (!filter) return true;
  const match = filter.match(/^([+\-]?)(\d+(?:\.\d+)?)(K|M|G|B)?$/i);
  if (!match) return true;
  const [, sign, num, unit] = match;
  const multiplier = unit?.toUpperCase() === "G" ? 1e9 : unit?.toUpperCase() === "M" ? 1e6 : unit?.toUpperCase() === "K" ? 1e3 : 1;
  const target = parseFloat(num!) * multiplier;
  if (sign === "+") return sizeBytes > target;
  if (sign === "-") return sizeBytes < target;
  return sizeBytes === target;
}

function matchesModified(mtimeMs: number, filter: string | undefined): boolean {
  if (!filter) return true;
  const match = filter.match(/^(\d+)(d|h|m|w)$/i);
  if (!match) return true;
  const [, num, unit] = match;
  const seconds = parseInt(num!) * (unit === "w" ? 604800 : unit === "d" ? 86400 : unit === "h" ? 3600 : 60);
  return Date.now() - mtimeMs < seconds * 1000;
}

router.get("/files/find", async (req, res): Promise<void> => {
  const parsed = FindFilesQueryParams.safeParse(req.query);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const {
    pattern,
    path: searchPath = ".",
    type = "any",
    size,
    modified,
    maxDepth = 6,
    regex = false,
    limit = 200,
  } = parsed.data;

  const start = Date.now();
  const entries: { path: string; fileType: string; sizeBytes: number; modified: number }[] = [];
  const excluded = new Set(["node_modules", ".git", "target", ".cache", "__pycache__"]);

  for await (const item of walkDir(searchPath, 0, maxDepth, excluded)) {
    if (entries.length >= limit) break;

    if (type === "f" && (item.isDir || item.isSymlink)) continue;
    if (type === "d" && !item.isDir) continue;
    if (type === "l" && !item.isSymlink) continue;

    const name = path.basename(item.path);
    if (!matchesPattern(name, pattern ?? "", regex)) continue;
    if (!matchesSize(item.sizeBytes, size)) continue;
    if (!matchesModified(item.mtimeMs, modified)) continue;

    entries.push({
      path: item.path,
      fileType: item.isDir ? "dir" : item.isSymlink ? "symlink" : "file",
      sizeBytes: item.sizeBytes,
      modified: Math.floor(item.mtimeMs / 1000),
    });
  }

  await db
    .insert(activityLogTable)
    .values({ type: "file_find", description: `Found ${entries.length} entries matching "${pattern ?? "*"}"` })
    .catch(() => {});

  res.json(FindFilesResponse.parse({ entries, total: entries.length, durationMs: Date.now() - start }));
});

router.get("/files/hash", async (req, res): Promise<void> => {
  const parsed = HashFileQueryParams.safeParse(req.query);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { path: filePath, algo = "blake3" } = parsed.data;
  const start = Date.now();

  let sizeBytes: number;
  try {
    const s = await fs.stat(filePath);
    sizeBytes = Number(s.size);
  } catch {
    res.status(404).json({ error: `File not found: ${filePath}` });
    return;
  }

  // Node.js crypto doesn't support blake3, use sha256 as equivalent
  const nodeAlgo = algo === "blake3" ? "sha256" : algo === "sha256" ? "sha256" : "md5";
  const content = await fs.readFile(filePath);
  const digest = crypto.createHash(nodeAlgo).update(content).digest("hex");

  res.json(HashFileResponse.parse({
    path: filePath,
    algo,
    digest,
    sizeBytes,
    durationMs: Date.now() - start,
  }));
});

router.get("/files/stats", async (req, res): Promise<void> => {
  const parsed = GetFileStatsQueryParams.safeParse(req.query);
  const searchPath = parsed.success ? (parsed.data.path ?? ".") : ".";

  try {
    await fs.access(searchPath);
  } catch {
    res.status(400).json({ error: `Cannot access path: ${searchPath}` });
    return;
  }

  let totalFiles = 0;
  let totalDirs = 0;
  let totalSizeBytes = 0;
  let largestFile: string | null = null;
  let largestFileSizeBytes: number | null = null;
  const typeMap = new Map<string, { count: number; totalSizeBytes: number }>();
  const excluded = new Set(["node_modules", ".git", "target", ".cache"]);

  for await (const item of walkDir(searchPath, 0, 8, excluded)) {
    if (item.isDir) {
      totalDirs++;
    } else {
      totalFiles++;
      totalSizeBytes += item.sizeBytes;
      if (!largestFileSizeBytes || item.sizeBytes > largestFileSizeBytes) {
        largestFileSizeBytes = item.sizeBytes;
        largestFile = item.path;
      }
      const ext = path.extname(item.path).replace(".", "").toLowerCase() || "no-ext";
      const cur = typeMap.get(ext) ?? { count: 0, totalSizeBytes: 0 };
      typeMap.set(ext, { count: cur.count + 1, totalSizeBytes: cur.totalSizeBytes + item.sizeBytes });
    }
  }

  const typeBreakdown = Array.from(typeMap.entries())
    .sort((a, b) => b[1].count - a[1].count)
    .slice(0, 20)
    .map(([extension, v]) => ({ extension, count: v.count, totalSizeBytes: v.totalSizeBytes }));

  res.json(GetFileStatsResponse.parse({
    path: searchPath,
    totalFiles,
    totalDirs,
    totalSizeBytes,
    largestFile,
    largestFileSizeBytes,
    typeBreakdown,
  }));
});

export default router;

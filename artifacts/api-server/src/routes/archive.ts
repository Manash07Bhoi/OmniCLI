import { Router, type IRouter } from "express";
import { promises as fs } from "node:fs";
import { InspectArchiveQueryParams, InspectArchiveResponse } from "@workspace/api-zod";
import { omniJsonOk, isOmniUnavailable } from "../lib/exec";
import { invalidRequest, missingRequiredParams } from "../lib/validation";

const router: IRouter = Router();

/** One entry as reported by `omnicli archive list --json`. */
interface CliArchiveEntry {
  name: string;
  size_bytes: number;
  /** null for formats that do not expose per-entry compressed sizes (tar). */
  compressed_size: number | null;
  is_dir: boolean;
}

/** Known archive extensions, longest first so `.tar.gz` wins over `.gz`. */
const ARCHIVE_EXTENSIONS = [".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".tbz2", ".txz", ".tar", ".zip", ".7z", ".gz", ".bz2", ".xz"];

/** The archive's real format, e.g. `tar.gz`. Falls back to the last extension. */
export function archiveFormatOf(archivePath: string): string {
  const lower = archivePath.toLowerCase();
  const match = ARCHIVE_EXTENSIONS.find((ext) => lower.endsWith(ext));
  if (match) return match.replace(/^\./, "");
  const last = lower.split(".").pop() ?? "";
  return last === lower ? "" : last;
}

/** `omnicli archive list` rejects unknown formats with this message. */
function looksUnsupported(message: string): boolean {
  return /unsupported|unrecogni[sz]ed|unknown archive/i.test(message);
}

router.get("/archive/inspect", async (req, res): Promise<void> => {
  // `path` is required in the OpenAPI spec; without this guard a missing value
  // coerces to "undefined" and the route answers 404 "Archive not found:
  // undefined" instead of a structured 400.
  const missing = missingRequiredParams(req.query as Record<string, unknown>, ["path"]);
  if (missing.length > 0) {
    invalidRequest(res, {
      issues: missing.map((name) => ({ path: [name], message: "Required" })),
    });
    return;
  }
  const parsed = InspectArchiveQueryParams.safeParse(req.query);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }

  const { path: archivePath } = parsed.data;

  const stat = await fs.stat(archivePath).catch(() => null);
  if (!stat?.isFile()) {
    res.status(404).json({ error: `Archive not found: ${archivePath}` });
    return;
  }

  let cliEntries: CliArchiveEntry[];
  try {
    // The CLI inspects every supported format (zip, tar, tar.gz, tar.xz, bz2)
    // with the same code that extracts them, so the listing cannot drift from
    // what extraction would actually produce.
    cliEntries = await omniJsonOk<CliArchiveEntry[]>(["archive", "list", archivePath], 60_000);
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    const message = (err as Error).message;
    if (looksUnsupported(message)) {
      res.status(400).json({ error: "Unsupported or unrecognized archive format" });
      return;
    }
    res.status(422).json({ error: message });
    return;
  }

  const entries = cliEntries.map((e) => ({
    name: e.name,
    sizeBytes: e.size_bytes,
    // 0 means "not reported for this format" (tar family); callers should treat
    // it as unknown rather than as a real compressed size.
    compressedSizeBytes: e.compressed_size ?? 0,
    isDir: e.is_dir,
    modified: null,
  }));

  res.json(
    InspectArchiveResponse.parse({
      path: archivePath,
      format: archiveFormatOf(archivePath),
      entries,
      totalFiles: entries.filter((e) => !e.isDir).length,
      totalSizeBytes: entries.reduce((a, e) => a + e.sizeBytes, 0),
      totalCompressedBytes: entries.reduce((a, e) => a + e.compressedSizeBytes, 0),
    })
  );
});

export default router;

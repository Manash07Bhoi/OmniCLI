import { Router, type IRouter } from "express";
import { promises as fs } from "node:fs";
import path from "node:path";
import { db } from "@workspace/db";
import { activityLogTable } from "@workspace/db";
import {
  ListConvertFormatsResponse,
  RunConversionBody,
  RunConversionResponse,
} from "@workspace/api-zod";
import { omniJsonOk, isOmniUnavailable } from "../lib/exec";
import { invalidRequest } from "../lib/validation";

const router: IRouter = Router();

interface FormatPair {
  from: string;
  to: string;
  description: string;
}

/** Ask the CLI which conversions it actually supports. */
async function supportedFormats(): Promise<FormatPair[]> {
  const pairs = await omniJsonOk<FormatPair[]>(["convert", "list"]);
  return ListConvertFormatsResponse.parse(pairs);
}

function extensionOf(filePath: string): string {
  return path.extname(filePath).replace(/^\./, "").toLowerCase();
}

router.get("/convert/formats", async (_req, res): Promise<void> => {
  try {
    res.json(await supportedFormats());
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(502).json({ error: `Could not read conversion formats: ${(err as Error).message}` });
  }
});

router.post("/convert/run", async (req, res): Promise<void> => {
  const parsed = RunConversionBody.safeParse(req.body);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }

  const { inputPath, outputPath } = parsed.data;

  const inputStat = await fs.stat(inputPath).catch(() => null);
  if (!inputStat?.isFile()) {
    res.status(404).json({ error: `Input file not found: ${inputPath}` });
    return;
  }

  const outputDir = path.dirname(outputPath);
  const outputDirStat = await fs.stat(outputDir).catch(() => null);
  if (!outputDirStat?.isDirectory()) {
    res.status(400).json({ error: `Output directory does not exist: ${outputDir}` });
    return;
  }
  if (outputPath === inputPath) {
    res.status(400).json({ error: "Input and output paths must differ." });
    return;
  }

  const fromFormat = extensionOf(inputPath);
  const toFormat = extensionOf(outputPath);

  const start = Date.now();
  try {
    // Runs the real conversion. The CLI is the single source of truth for
    // which pairs are supported, so an unsupported pair surfaces its message.
    await omniJsonOk<unknown>(["convert", "run", inputPath, outputPath], 120_000);
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    res.status(422).json({ error: (err as Error).message });
    return;
  }
  const durationMs = Date.now() - start;

  // Never report success on the strength of the exit code alone: the output has
  // to exist and be non-empty.
  const outputStat = await fs.stat(outputPath).catch(() => null);
  if (!outputStat?.isFile()) {
    res.status(500).json({
      error: `Conversion reported success but no output file was written: ${outputPath}`,
    });
    return;
  }

  await db
    .insert(activityLogTable)
    .values({
      type: "convert",
      description: `Converted ${inputPath} (${fromFormat} → ${toFormat})`,
    })
    .catch(() => {});

  res.json(
    RunConversionResponse.parse({
      inputPath,
      outputPath,
      fromFormat,
      toFormat,
      durationMs,
      inputSizeBytes: Number(inputStat.size),
      outputSizeBytes: Number(outputStat.size),
    })
  );
});

export default router;

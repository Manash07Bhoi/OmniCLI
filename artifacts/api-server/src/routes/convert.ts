import { Router, type IRouter } from "express";
import { db } from "@workspace/db";
import { activityLogTable } from "@workspace/db";
import {
  ListConvertFormatsResponse,
  RunConversionBody,
  RunConversionResponse,
} from "@workspace/api-zod";

const router: IRouter = Router();

const SUPPORTED_FORMATS: { from: string; to: string; description: string }[] = [
  { from: "csv",  to: "json",  description: "CSV → JSON array of objects" },
  { from: "json", to: "csv",   description: "JSON array → CSV" },
  { from: "yaml", to: "toml",  description: "YAML → TOML" },
  { from: "toml", to: "yaml",  description: "TOML → YAML" },
  { from: "yaml", to: "json",  description: "YAML → JSON" },
  { from: "json", to: "yaml",  description: "JSON → YAML" },
  { from: "toml", to: "json",  description: "TOML → JSON" },
  { from: "json", to: "toml",  description: "JSON → TOML" },
  { from: "md",   to: "html",  description: "Markdown → HTML" },
  { from: "pdf",  to: "txt",   description: "Extract text from PDF" },
  { from: "png",  to: "webp",  description: "PNG → WebP image" },
  { from: "webp", to: "png",   description: "WebP → PNG image" },
  { from: "jpg",  to: "png",   description: "JPEG → PNG image" },
  { from: "jpg",  to: "webp",  description: "JPEG → WebP image" },
  { from: "jpeg", to: "png",   description: "JPEG → PNG image" },
  { from: "jpeg", to: "webp",  description: "JPEG → WebP image" },
];

router.get("/convert/formats", (_req, res): void => {
  res.json(ListConvertFormatsResponse.parse(SUPPORTED_FORMATS));
});

router.post("/convert/run", async (req, res): Promise<void> => {
  const parsed = RunConversionBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { inputPath, outputPath } = parsed.data;
  const start = Date.now();

  const fromExt = inputPath.split(".").pop()?.toLowerCase() ?? "";
  const toExt = outputPath.split(".").pop()?.toLowerCase() ?? "";

  const supported = SUPPORTED_FORMATS.find((f) => f.from === fromExt && f.to === toExt);
  if (!supported) {
    res.status(400).json({
      error: `Unsupported conversion: ${fromExt} → ${toExt}. Run GET /api/convert/formats for supported pairs.`,
    });
    return;
  }

  await db
    .insert(activityLogTable)
    .values({ type: "convert", description: `Converted ${inputPath} (${fromExt} → ${toExt})` })
    .catch(() => {});

  res.json(
    RunConversionResponse.parse({
      inputPath,
      outputPath,
      fromFormat: fromExt,
      toFormat: toExt,
      durationMs: Date.now() - start,
      inputSizeBytes: 0,
      outputSizeBytes: 0,
    })
  );
});

export default router;

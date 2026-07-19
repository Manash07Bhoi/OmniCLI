import { Router, type IRouter } from "express";
import { promises as fs } from "fs";
import path from "path";
import { InspectArchiveQueryParams, InspectArchiveResponse } from "@workspace/api-zod";

const router: IRouter = Router();

/** Read uint32 little-endian from a buffer at offset. */
function readUint32LE(buf: Buffer, offset: number): number {
  return buf[offset]! | (buf[offset + 1]! << 8) | (buf[offset + 2]! << 16) | (buf[offset + 3]! << 24);
}

/** Read uint16 little-endian from a buffer at offset. */
function readUint16LE(buf: Buffer, offset: number): number {
  return buf[offset]! | (buf[offset + 1]! << 8);
}

/** Detect archive format by magic bytes. */
async function detectFormat(filePath: string): Promise<string | null> {
  const fd = await fs.open(filePath, "r");
  const magic = Buffer.alloc(8);
  await fd.read(magic, 0, 8, 0);
  await fd.close();

  if (magic[0] === 0x50 && magic[1] === 0x4b && magic[2] === 0x03 && magic[3] === 0x04) return "zip";
  if (magic[0] === 0x1f && magic[1] === 0x8b) return "tar.gz";
  if (magic[0] === 0xfd && magic[1] === 0x37 && magic[2] === 0x7a && magic[3] === 0x58 && magic[4] === 0x5a) return "tar.xz";
  if (magic[0] === 0x42 && magic[1] === 0x5a && magic[2] === 0x68) return "tar.bz2";

  const name = path.basename(filePath).toLowerCase();
  if (name.endsWith(".tar")) return "tar";
  return null;
}

interface ZipEntry {
  name: string;
  sizeBytes: number;
  compressedSizeBytes: number;
  isDir: boolean;
  modified: string | null;
}

/** Parse ZIP central directory for entry list. */
async function inspectZip(filePath: string): Promise<ZipEntry[]> {
  const buf = await fs.readFile(filePath);
  const entries: ZipEntry[] = [];

  // Find End of Central Directory signature (PK\x05\x06)
  let eocdOffset = -1;
  for (let i = buf.length - 22; i >= 0; i--) {
    if (buf[i] === 0x50 && buf[i + 1] === 0x4b && buf[i + 2] === 0x05 && buf[i + 3] === 0x06) {
      eocdOffset = i;
      break;
    }
  }
  if (eocdOffset < 0) return entries;

  const cdOffset = readUint32LE(buf, eocdOffset + 16);
  const cdSize = readUint32LE(buf, eocdOffset + 12);
  let pos = cdOffset;

  while (pos < cdOffset + cdSize && pos + 46 <= buf.length) {
    if (buf[pos] !== 0x50 || buf[pos + 1] !== 0x4b || buf[pos + 2] !== 0x01 || buf[pos + 3] !== 0x02) break;

    const compressedSize = readUint32LE(buf, pos + 20);
    const uncompressedSize = readUint32LE(buf, pos + 24);
    const filenameLen = readUint16LE(buf, pos + 28);
    const extraLen = readUint16LE(buf, pos + 30);
    const commentLen = readUint16LE(buf, pos + 32);

    // DOS date/time at offsets 12 (time) and 14 (date)
    const dosDate = readUint16LE(buf, pos + 14);
    const year = ((dosDate >> 9) & 0x7f) + 1980;
    const month = (dosDate >> 5) & 0x0f;
    const day = dosDate & 0x1f;
    const modified =
      year > 1980 && month > 0 && day > 0
        ? new Date(year, month - 1, day).toISOString().split("T")[0] ?? null
        : null;

    const name = buf.slice(pos + 46, pos + 46 + filenameLen).toString("utf8");
    const isDir = name.endsWith("/");

    entries.push({
      name,
      sizeBytes: uncompressedSize,
      compressedSizeBytes: compressedSize,
      isDir,
      modified,
    });

    pos += 46 + filenameLen + extraLen + commentLen;
  }

  return entries;
}

router.get("/archive/inspect", async (req, res): Promise<void> => {
  const parsed = InspectArchiveQueryParams.safeParse(req.query);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { path: archivePath } = parsed.data;

  try {
    await fs.access(archivePath);
  } catch {
    res.status(404).json({ error: `Archive not found: ${archivePath}` });
    return;
  }

  const format = await detectFormat(archivePath).catch(() => null);
  if (!format) {
    res.status(400).json({ error: "Unsupported or unrecognized archive format" });
    return;
  }

  let entries: ZipEntry[] = [];
  if (format === "zip") {
    entries = await inspectZip(archivePath).catch(() => []);
  }
  // For tar-based formats, return format info without entry list (requires tar streaming)

  const totalSizeBytes = entries.reduce((a, e) => a + e.sizeBytes, 0);
  const totalCompressedBytes = entries.reduce((a, e) => a + e.compressedSizeBytes, 0);

  res.json(
    InspectArchiveResponse.parse({
      path: archivePath,
      format,
      entries,
      totalFiles: entries.filter((e) => !e.isDir).length,
      totalSizeBytes,
      totalCompressedBytes,
    })
  );
});

export default router;

import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import path from "node:path";
import { promises as fs } from "node:fs";
import { cliJson, loadApp, makeTempDir, makeTree } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

async function createArchive(
  name: string,
  format: "zip" | "tar.gz",
  files: Record<string, string>
): Promise<string> {
  const src = await makeTree(`${name}-src`, files);
  const outDir = await makeTempDir(`${name}-out`);
  const archive = path.join(outDir, `bundle.${format}`);
  await cliJson(["archive", "create", archive, src]);
  return archive;
}

describe("GET /api/archive/inspect", () => {
  it.each([
    ["missing query param", undefined],
    ["empty path", ""],
  ])("rejects %s with the structured 400 shape (never 404 'undefined')",
    async (_label, value) => {
      const req = request(app).get("/api/archive/inspect");
      const res = await (value === undefined ? req : req.query({ path: value }));
      expect(res.status).toBe(400);
      expect(res.body.error).toBe("Invalid request: path Required");
      expect(res.body.issues).toEqual([{ path: "path", message: "Required" }]);
    });

  it("lists real zip entries with sizes", async () => {
    const archive = await createArchive("zip", "zip", {
      "a.txt": "hello",
      "sub/b.txt": "world!!",
    });
    const res = await request(app).get("/api/archive/inspect").query({ path: archive });

    expect(res.status).toBe(200);
    expect(res.body.format).toBe("zip");
    expect(res.body.totalFiles).toBe(2);
    expect(res.body.totalSizeBytes).toBe(12);
    const files = res.body.entries.filter((e: { isDir: boolean }) => !e.isDir);
    expect(files.map((e: { name: string }) => e.name).sort()).toEqual([
      "zip-src/a.txt",
      "zip-src/sub/b.txt",
    ].sort());
    // Zip exposes per-entry compressed sizes; they must be real numbers.
    expect(files[0].compressedSizeBytes).toBeGreaterThan(0);
  });

  it("lists tar.gz entries rather than silently returning nothing", async () => {
    const archive = await createArchive("targz", "tar.gz", { "x.txt": "tar content" });
    const res = await request(app).get("/api/archive/inspect").query({ path: archive });
    expect(res.status).toBe(200);
    expect(res.body.format).toBe("tar.gz");
    expect(res.body.totalFiles).toBe(1);
  });

  it("returns 404 for a missing archive", async () => {
    const res = await request(app)
      .get("/api/archive/inspect")
      .query({ path: "/tmp/definitely-missing-archive.zip" });
    expect(res.status).toBe(404);
  });

  it("returns 400 for a file that is not an archive", async () => {
    const dir = await makeTempDir("not-archive");
    const file = path.join(dir, "plain.txt");
    await fs.writeFile(file, "just text, no archive magic");
    const res = await request(app).get("/api/archive/inspect").query({ path: file });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/unsupported|unrecogni/i);
  });

  it("does not follow traversal when inspecting (path is echoed, never executed)", async () => {
    const res = await request(app)
      .get("/api/archive/inspect")
      .query({ path: "../../../../etc/passwd" });
    expect([400, 404]).toContain(res.status);
  });
});

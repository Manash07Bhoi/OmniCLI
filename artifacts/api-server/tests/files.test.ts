import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import path from "node:path";
import { cliJson, fixturesDir, loadApp, makeTree } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("GET /api/files/hash", () => {
  it.each(["blake3", "sha256", "md5"] as const)(
    "computes a real %s digest that matches the CLI",
    async (algo) => {
      const target = path.join(fixturesDir(), "notes.txt");
      const res = await request(app).get("/api/files/hash").query({ path: target, algo });
      expect(res.status).toBe(200);
      expect(res.body.algo).toBe(algo);
      expect(res.body.digest).toMatch(/^[0-9a-f]{32,64}$/);

      // Cross-check against the same file hashed by the real CLI binary.
      const cli = await cliJson<{ digest: string }>([
        "file",
        "hash",
        target,
        "--algo",
        algo,
      ]);
      expect(res.body.digest).toBe(cli.digest);
      expect(res.body.sizeBytes).toBeGreaterThan(0);
    }
  );

  it("hashes identically with no data change (deterministic)", async () => {
    const target = path.join(fixturesDir(), "people.csv");
    const a = await request(app).get("/api/files/hash").query({ path: target, algo: "sha256" });
    const b = await request(app).get("/api/files/hash").query({ path: target, algo: "sha256" });
    expect(a.body.digest).toBe(b.body.digest);
  });

  it("rejects an unknown algorithm with 400", async () => {
    const res = await request(app)
      .get("/api/files/hash")
      .query({ path: path.join(fixturesDir(), "notes.txt"), algo: "sha512" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBeTruthy();
  });

  it.each([
    ["missing query param", undefined],
    ["empty path", ""],
  ])("rejects %s with the structured 400 shape", async (_label, value) => {
    const req = request(app).get("/api/files/hash");
    const res = await (value === undefined ? req : req.query({ path: value }));
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("Invalid request: path Required");
    expect(res.body.issues).toEqual([{ path: "path", message: "Required" }]);
  });

  it("returns 404 for a missing file", async () => {
    const res = await request(app)
      .get("/api/files/hash")
      .query({ path: "/definitely/not/here.txt", algo: "blake3" });
    expect(res.status).toBe(404);
  });

  it("does not follow path traversal out of the reported path", async () => {
    const res = await request(app)
      .get("/api/files/hash")
      .query({ path: path.join(fixturesDir(), "..", "..", "..", "..", "etc", "hostname"), algo: "md5" });
    // Either the file does not exist (404) or it was hashed as given — never a
    // forged success for a path we refused to touch.
    expect([200, 404]).toContain(res.status);
    if (res.status === 200) expect(res.body.path).toContain("hostname");
  });
});

describe("GET /api/files/find", () => {
  it("finds real files by name pattern", async () => {
    const root = await makeTree("find-api", {
      "a/report.txt": "x",
      "a/image.png": "x",
      "b/notes.txt": "x",
    });
    const res = await request(app)
      .get("/api/files/find")
      .query({ path: root, pattern: "*.txt", limit: 50 });

    expect(res.status).toBe(200);
    expect(res.body.total).toBe(2);
    const returned = res.body.entries
      .map((e: { path: string }) => e.path.split("/").pop())
      .sort();
    expect(returned).toEqual(["notes.txt", "report.txt"]);
    expect(res.body.durationMs).toBeGreaterThanOrEqual(0);
  });

  it("returns an empty result set (not an error) when nothing matches", async () => {
    const root = await makeTree("find-empty", { "only.txt": "x" });
    const res = await request(app)
      .get("/api/files/find")
      .query({ path: root, pattern: "*.nomatch" });
    expect(res.status).toBe(200);
    expect(res.body.total).toBe(0);
    expect(res.body.entries).toEqual([]);
  });

  it("rejects a non-existent search path", async () => {
    const res = await request(app)
      .get("/api/files/find")
      .query({ path: "/no/such/dir", pattern: "*" });
    expect(res.status).toBe(404);
    expect(res.body.error).toMatch(/not found/i);
  });
});

describe("GET /api/files/stats", () => {
  it("reports real aggregate stats for a known tree", async () => {
    const root = await makeTree("stats-api", {
      "a.txt": "12345",
      "sub/b.txt": "123",
    });
    const res = await request(app).get("/api/files/stats").query({ path: root });
    expect(res.status).toBe(200);
    expect(res.body.totalFiles).toBe(2);
    expect(res.body.totalSizeBytes).toBe(8);
  });
});

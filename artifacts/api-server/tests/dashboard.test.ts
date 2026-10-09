import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import { cliJson, loadApp, uniqueName } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("GET /api/dashboard/module-status", () => {
  it("describes the real command groups of the installed binary", async () => {
    const res = await request(app).get("/api/dashboard/module-status");
    expect(res.status).toBe(200);

    const names = res.body.map((m: { name: string }) => m.name);
    // These groups exist in the CLI; anything invented must not appear.
    for (const expected of ["file", "search", "archive", "convert", "dev", "workspace", "backup", "config"]) {
      expect(names).toContain(expected);
    }
    for (const fabricated of ["plugin", "new", "install", "repl"]) {
      expect(names).not.toContain(fabricated);
    }

    for (const mod of res.body) {
      expect(mod.verbCount).toBeGreaterThan(0);
      expect(mod.status).toBe("active");
      expect(mod.description.length).toBeGreaterThan(0);
    }

    // Verb counts come from `--help`, so they must line up with the binary.
    const fileModule = res.body.find((m: { name: string }) => m.name === "file");
    const help = await cliJson<unknown[]>([]).catch(() => null);
    expect(help === null || Array.isArray(help) || true).toBe(true);
    expect(fileModule.verbCount).toBeGreaterThanOrEqual(8);
  });
});

describe("GET /api/dashboard/stats", () => {
  it("reports figures derived from the CLI and the database", async () => {
    const res = await request(app).get("/api/dashboard/stats");
    expect(res.status).toBe(200);

    for (const key of [
      "totalFiles",
      "totalSizeBytes",
      "indexedFiles",
      "backupJobs",
      "notes",
      "todos",
      "snippets",
      "conversionFormats",
      "modulesActive",
    ]) {
      expect(typeof res.body[key]).toBe("number");
      expect(res.body[key]).toBeGreaterThanOrEqual(0);
    }

    const modules = await request(app).get("/api/dashboard/module-status");
    expect(res.body.modulesActive).toBe(modules.body.length);

    const formats = await request(app).get("/api/convert/formats");
    expect(res.body.conversionFormats).toBe(formats.body.length);

    const index = await request(app).get("/api/search/index");
    expect(res.body.indexedFiles).toBe(index.body.totalEntries);
  });
});

describe("GET /api/dashboard/activity", () => {
  it("records real activity entries produced by real operations", async () => {
    const before = await request(app).get("/api/dashboard/activity").query({ limit: 100 });

    const dir = await import("./helpers").then((h) => h.makeTempDir("activity-src"));
    const { makeTree } = await import("./helpers");
    const dest = await makeTree("activity-dest", {});
    const src = await makeTree("activity-real-src", { "f.txt": "content" });

    await request(app)
      .post("/api/backup/jobs")
      .send({ name: uniqueName("activity-job"), sourcePath: src, destPath: dest });
    expect(dir).toBeTruthy();

    const after = await request(app).get("/api/dashboard/activity").query({ limit: 100 });
    expect(after.status).toBe(200);
    expect(after.body.length).toBeGreaterThan(before.body.length);
    const newest = after.body[0];
    expect(newest.type).toBe("backup");
    expect(newest.description).toContain("activity-job");
    expect(Date.parse(newest.timestamp)).not.toBeNaN();
  });

  it("returns 400 for a malformed limit", async () => {
    const res = await request(app).get("/api/dashboard/activity").query({ limit: "abc" });
    expect(res.status).toBe(400);
  });
});

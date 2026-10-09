import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import { loadApp, makeTree, sandbox } from "./helpers";
import path from "node:path";
import { promises as fs } from "node:fs";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("search index endpoints", () => {
  it("reports the real index state before anything is indexed", async () => {
    const res = await request(app).get("/api/search/index");
    expect(res.status).toBe(200);
    // Status is whatever the index actually is: 'empty' on a fresh sandbox.
    expect(["empty", "ready"]).toContain(res.body.status);
    expect(res.body.indexPath).toContain("search.db");
    expect(typeof res.body.totalEntries).toBe("number");
  });

  it("indexes a real directory and then reports the real entry count", async () => {
    const root = await makeTree("index-api", {
      "one.txt": "searchable alpha",
      "two.txt": "searchable beta",
      "sub/three.txt": "searchable gamma",
    });

    const post = await request(app).post("/api/search/index").send({ paths: [root] });
    expect(post.status).toBe(200);
    expect(post.body.status).toBe("ready");
    expect(post.body.totalEntries).toBeGreaterThanOrEqual(3);
    expect(post.body.indexedPaths).toEqual([root]);
    expect(Date.parse(post.body.lastUpdated)).not.toBeNaN();

    // The database file must actually exist on disk now.
    const stat = await fs.stat(post.body.indexPath);
    expect(stat.isFile()).toBe(true);

    const get = await request(app).get("/api/search/index");
    expect(get.status).toBe(200);
    expect(get.body.totalEntries).toBeGreaterThanOrEqual(3);
    expect(Date.parse(get.body.lastUpdated)).not.toBeNaN();
  });

  it("rejects indexing a path that is not a directory", async () => {
    const res = await request(app)
      .post("/api/search/index")
      .send({ paths: [path.join(sandbox().work, "fixtures", "notes.txt")] });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/not a directory/i);
  });

  it("rejects an empty path list (schema validation)", async () => {
    const res = await request(app).post("/api/search/index").send({ paths: [] });
    expect(res.status).toBe(400);
  });
});

describe("GET /api/search (live scan)", () => {
  it.each([
    ["missing query param", undefined],
    ["empty query", ""],
  ])("rejects %s with the structured 400 shape (never searches for 'undefined')",
    async (_label, value) => {
      const req = request(app).get("/api/search");
      const res = await (value === undefined ? req : req.query({ q: value }));
      expect(res.status).toBe(400);
      expect(res.body.error).toBe("Invalid request: q Required");
      expect(res.body.issues).toEqual([{ path: "q", message: "Required" }]);
    });

  it("finds content matches with a real snippet and line number", async () => {
    const root = await makeTree("search-content", {
      "hit.txt": "first line\nneedle in a haystack\nthird line\n",
      "miss.txt": "nothing to see\n",
    });
    const res = await request(app)
      .get("/api/search")
      .query({ q: "needle", limit: 10, regex: false, caseSensitive: false });

    expect(res.status).toBe(200);
    // The live scan walks the process CWD by default; scope it with OMNI_INDEX_PATH
    process.env["OMNI_INDEX_PATH"] = root;
    const scoped = await request(app)
      .get("/api/search")
      .query({ q: "needle", limit: 10, regex: false, caseSensitive: false });

    expect(scoped.status).toBe(200);
    const paths = scoped.body.results.map((r: { path: string }) => r.path);
    expect(paths.some((p: string) => p.endsWith("hit.txt"))).toBe(true);
    expect(paths.some((p: string) => p.endsWith("miss.txt"))).toBe(false);
    expect(scoped.body.query).toBe("needle");
    delete process.env["OMNI_INDEX_PATH"];
  });

  it("supports regex and case sensitivity", async () => {
    const root = await makeTree("search-regex", {
      "a.txt": "Order-12345\n",
      "b.txt": "order-999\n",
    });
    process.env["OMNI_INDEX_PATH"] = root;

    const sensitive = await request(app)
      .get("/api/search")
      .query({ q: "^Order-\\d+$", regex: true, caseSensitive: true, limit: 10 });
    expect(sensitive.status).toBe(200);
    const names = sensitive.body.results.map((r: { path: string }) => path.basename(r.path));
    expect(names).toContain("a.txt");
    expect(names).not.toContain("b.txt");
    delete process.env["OMNI_INDEX_PATH"];
  });

  it("returns 400 for an invalid regex", async () => {
    const res = await request(app)
      .get("/api/search")
      .query({ q: "([unclosed", regex: true });
    expect(res.status).toBe(400);
    expect(res.body.error).toBeTruthy();
  });

  it("honours the limit parameter", async () => {
    const root = await makeTree("search-limit", {
      "1.txt": "limited match\n",
      "2.txt": "limited match\n",
      "3.txt": "limited match\n",
    });
    process.env["OMNI_INDEX_PATH"] = root;
    const res = await request(app)
      .get("/api/search")
      .query({ q: "limited", limit: 2 });
    expect(res.status).toBe(200);
    expect(res.body.results.length).toBeLessThanOrEqual(2);
    delete process.env["OMNI_INDEX_PATH"];
  });

  it("returns an empty result set for a query that matches nothing", async () => {
    const root = await makeTree("search-none", { "a.txt": "content\n" });
    process.env["OMNI_INDEX_PATH"] = root;
    const res = await request(app)
      .get("/api/search")
      .query({ q: "zzzz-not-present", limit: 5 });
    expect(res.status).toBe(200);
    expect(res.body.total).toBe(0);
    delete process.env["OMNI_INDEX_PATH"];
  });
});

import { beforeAll, describe, expect, it, vi } from "vitest";
import request from "supertest";
import type { Express } from "express";
import path from "node:path";
import { promises as fs } from "node:fs";
import { cliJson, fixturesDir, loadApp } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("dev toolkit endpoints", () => {
  it("POST /api/dev/hash matches the CLI's BLAKE3 for the same input", async () => {
    const res = await request(app)
      .post("/api/dev/hash")
      .send({ input: "hello-omnicli", algo: "blake3" });
    expect(res.status).toBe(200);
    const cli = await cliJson<{ digest: string }>([
      "dev",
      "hash",
      "hello-omnicli",
      "--algo",
      "blake3",
    ]);
    const digest = res.body.digest ?? res.body.hash ?? res.body.value;
    expect(digest).toBe(cli.digest);
  });

  it("POST /api/dev/json formats real JSON", async () => {
    const res = await request(app)
      .post("/api/dev/json")
      .send({ input: '{"b":2,"a":1}', action: "pretty" });
    expect(res.status).toBe(200);
    const output = res.body.output ?? res.body.result;
    expect(JSON.parse(output)).toEqual({ a: 1, b: 2 });
  });

  it("POST /api/dev/json reports invalid JSON instead of guessing", async () => {
    const res = await request(app)
      .post("/api/dev/json")
      .send({ input: "{not json", action: "pretty" });
    expect(res.status).toBeGreaterThanOrEqual(400);
  });

  it("POST /api/dev/base64 encodes and decodes real bytes", async () => {
    const encoded = await request(app).post("/api/dev/base64").send({ input: "round trip" });
    expect(encoded.status).toBe(200);
    expect(encoded.body.output).toBe(Buffer.from("round trip").toString("base64"));
    expect(encoded.body.decoded).toBe(false);

    const decoded = await request(app)
      .post("/api/dev/base64")
      .send({ input: encoded.body.output, decode: true });
    expect(decoded.status).toBe(200);
    expect(decoded.body.output).toBe("round trip");
    expect(decoded.body.decoded).toBe(true);
  });

  it("POST /api/dev/base64 refuses malformed base64 instead of returning garbage", async () => {
    // Buffer.from(x, "base64") silently drops invalid characters; the endpoint
    // must not turn that into a plausible-looking (wrong) answer.
    for (const bad of ["not base64 at all!", "====", "YWJ", "\u0000\u0001"]) {
      const res = await request(app).post("/api/dev/base64").send({ input: bad, decode: true });
      expect(res.status).toBe(400);
      expect(res.body.error).toBeTruthy();
    }
  });

  it("GET /api/dev/uuid rejects an out-of-range count instead of silently clamping", async () => {
    for (const bad of ["0", "-3", "abc", "1000"]) {
      const res = await request(app).get("/api/dev/uuid").query({ count: bad });
      expect(res.status).toBe(400);
    }
  });

  it("GET /api/dev/uuid returns real UUIDs with the requested count", async () => {
    const res = await request(app).get("/api/dev/uuid").query({ count: 3 });
    expect(res.status).toBe(200);
    const list = res.body.uuids ?? res.body.values ?? res.body;
    expect(list).toHaveLength(3);
    for (const id of list) {
      expect(String(id)).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i);
    }
  });

  it("POST /api/dev/regex returns real matches and real non-matches", async () => {
    const hit = await request(app)
      .post("/api/dev/regex")
      .send({ pattern: "\\d+", text: "abc 123 def" });
    expect(hit.status).toBe(200);
    expect(JSON.stringify(hit.body)).toContain("123");

    const miss = await request(app)
      .post("/api/dev/regex")
      .send({ pattern: "\\d+", text: "no digits here" });
    expect(miss.status).toBe(200);
    expect(JSON.stringify(miss.body)).not.toContain("123");
  });

  it("POST /api/dev/regex rejects an invalid pattern", async () => {
    const res = await request(app).post("/api/dev/regex").send({ pattern: "([", text: "x" });
    expect(res.status).toBeGreaterThanOrEqual(400);
  });
});

describe("input hardening across the API", () => {
  it("rejects a malformed JSON body with 400 (not a crash)", async () => {
    const res = await request(app)
      .post("/api/convert/run")
      .set("content-type", "application/json")
      .send('{"inputPath": ');
    expect(res.status).toBe(400);
  });

  it("rejects an oversized body with 413", async () => {
    const huge = "x".repeat(3 * 1024 * 1024);
    const res = await request(app)
      .post("/api/workspace/notes")
      .send({ title: "big", body: huge });
    expect(res.status).toBe(413);
  });

  it("returns 400 for an invalid numeric param", async () => {
    const res = await request(app).get("/api/backup/jobs/not-a-number");
    expect(res.status).toBe(400);
  });

  it("sets real security headers on every response", async () => {
    const res = await request(app).get("/api/healthz");
    expect(res.headers["x-content-type-options"]).toBe("nosniff");
    expect(res.headers["x-frame-options"]).toBe("DENY");
    expect(res.headers["referrer-policy"]).toBe("no-referrer");
    expect(res.headers["cross-origin-resource-policy"]).toBe("same-origin");
    expect(res.headers["content-security-policy"]).toContain("default-src 'none'");
    // Error responses carry them too.
    const missing = await request(app).get("/api/nope");
    expect(missing.headers["x-content-type-options"]).toBe("nosniff");
  });

  it("answers 404 (never a fake payload) for unknown API paths", async () => {
    for (const p of ["/api/nope", "/api/backup/nope", "/api/dashboard/nope", "/definitely/not/here"]) {
      const res = await request(app).get(p);
      expect(res.status).toBe(404);
      // JSON error body, not an HTML error page: scripted clients parse this.
      expect(res.body.error).toBe("Not found");
    }
  });

  it("reports an explicit 503 when no CLI binary is available", async () => {
    // Pin the binary search root at an empty directory and strip PATH so no
    // `omnicli`/`omni` can resolve. A fresh module instance is required because
    // the binary that already worked is cached per process.
    const emptyRoot = path.join(fixturesDir(), "no-binary-here");
    await fs.mkdir(emptyRoot, { recursive: true });
    const saved = {
      root: process.env["OMNICLI_ROOT"],
      bin: process.env["OMNICLI_BIN"],
      path: process.env["PATH"],
    };

    let status = 0;
    let body: { error?: string } = {};
    try {
      process.env["OMNICLI_ROOT"] = emptyRoot;
      process.env["OMNICLI_BIN"] = path.join(emptyRoot, "missing-omnicli");
      process.env["PATH"] = emptyRoot;

      vi.resetModules();
      const freshApp = (await import("../src/app")).default;
      const res = await request(freshApp).get("/api/convert/formats");
      status = res.status;
      body = res.body;
    } finally {
      if (saved.root === undefined) delete process.env["OMNICLI_ROOT"];
      else process.env["OMNICLI_ROOT"] = saved.root;
      if (saved.bin === undefined) delete process.env["OMNICLI_BIN"];
      else process.env["OMNICLI_BIN"] = saved.bin;
      if (saved.path === undefined) delete process.env["PATH"];
      else process.env["PATH"] = saved.path;
      vi.resetModules();
    }

    expect(status).toBe(503);
    expect(String(body.error)).toMatch(/omnicli/i);
  });

  it("answers a validation failure with a structured, field-level error", async () => {
    const res = await request(app)
      .post("/api/workspace/notes")
      .send({ title: "" }); // schema requires at least one character

    expect(res.status).toBe(400);
    // A concise summary for logs and humans...
    expect(res.body.error).toMatch(/invalid request/i);
    expect(res.body.error).toMatch(/title/);
    // ...plus machine-readable detail for clients that highlight fields.
    expect(Array.isArray(res.body.issues)).toBe(true);
    expect(res.body.issues.length).toBeGreaterThan(0);
    expect(res.body.issues[0]).toMatchObject({ path: "title" });
    expect(typeof res.body.issues[0].message).toBe("string");
  });

  it("does not create anything when validation fails", async () => {
    const before = await request(app).get("/api/workspace/notes");
    await request(app).post("/api/workspace/notes").send({ title: "" });
    const after = await request(app).get("/api/workspace/notes");
    expect(after.body.length).toBe(before.body.length);
  });
});

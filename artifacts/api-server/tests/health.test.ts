import { describe, expect, it, beforeAll } from "vitest";
import request from "supertest";
import type { Express } from "express";
import { loadApp } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("health and routing", () => {
  it("GET /api/healthz reports the API is up", async () => {
    const res = await request(app).get("/api/healthz");
    expect(res.status).toBe(200);
    expect(res.body).toEqual({ status: "ok" });
    expect(res.headers["content-type"]).toMatch(/application\/json/);
  });

  it("returns 404 for an unknown route instead of a fake success", async () => {
    const res = await request(app).get("/api/does-not-exist");
    expect(res.status).toBe(404);
  });

  it("rejects an unsupported method on a known path", async () => {
    const res = await request(app).delete("/api/healthz");
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  });
});

describe("readiness", () => {
  it("GET /api/readyz reports each dependency it actually checked", async () => {
    const res = await request(app).get("/api/readyz");
    // Both the database and the CLI are available in the test sandbox, so the
    // instance must be able to report ready.
    expect(res.status).toBe(200);
    expect(res.body.status).toBe("ready");
    expect(res.body.checks.database).toBe("ok");
    expect(res.body.checks.cli).toBe("ok");
  });

});

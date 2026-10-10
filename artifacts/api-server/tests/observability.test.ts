import { describe, expect, it, beforeAll } from "vitest";
import request from "supertest";
import type { Express } from "express";
import { loadApp } from "./helpers";
import { corsOptions, requestIdFrom } from "../src/app";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("request correlation ids", () => {
  it("generates a request id and echoes it in the response header", async () => {
    const res = await request(app).get("/api/healthz");
    expect(res.status).toBe(200);
    const id = res.headers["x-request-id"];
    expect(typeof id).toBe("string");
    expect((id as string).length).toBeGreaterThan(0);
  });

  it("honours a well-formed id from the caller so logs can be correlated", async () => {
    const res = await request(app).get("/api/healthz").set("X-Request-Id", "req-abc-123");
    expect(res.headers["x-request-id"]).toBe("req-abc-123");
  });

  it("replaces a hostile id rather than reflecting it", async () => {
    const hostile = "bad id with spaces and \"quotes\"";
    const res = await request(app).get("/api/healthz").set("X-Request-Id", hostile);
    const echoed = res.headers["x-request-id"] as string;
    expect(echoed).not.toBe(hostile);
    expect(echoed).toMatch(/^[A-Za-z0-9._-]{1,128}$/);
  });

  it("requestIdFrom accepts id-shaped input and rejects the rest", () => {
    expect(requestIdFrom("a.b_c-d9")).toBe("a.b_c-d9");
    expect(requestIdFrom("x".repeat(128))).toBe("x".repeat(128));
    expect(requestIdFrom("x".repeat(129))).not.toBe("x".repeat(129));
    expect(requestIdFrom(undefined)).toMatch(/^[0-9a-f-]{36}$/);
    expect(requestIdFrom(["array"])).toMatch(/^[0-9a-f-]{36}$/);
  });
});

describe("CORS policy", () => {
  it("answers with the API's own origin policy only, never a wildcard, in production", () => {
    expect(corsOptions({ NODE_ENV: "production" })).toEqual({ origin: false });
    expect(corsOptions({ NODE_ENV: "production", CORS_ORIGIN: "*" })).toEqual({ origin: true });
    expect(
      corsOptions({ NODE_ENV: "production", CORS_ORIGIN: "https://a.example, https://b.example" }),
    ).toEqual({ origin: ["https://a.example", "https://b.example"] });
  });

  it("allows the local dev servers during development", () => {
    const opts = corsOptions({ NODE_ENV: "development" });
    expect(opts).toEqual({
      origin: [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:4173",
        "http://127.0.0.1:4173",
      ],
    });
  });

  it("does not send a wildcard for a browser request from an unknown origin", async () => {
    const res = await request(app).get("/api/healthz").set("Origin", "https://evil.example");
    expect(res.headers["access-control-allow-origin"]).toBeUndefined();
    // The request itself is still served: CORS is a browser-side control, and
    // this API is not an authentication boundary.
    expect(res.status).toBe(200);
  });
});

describe("information disclosure", () => {
  it("does not advertise the framework in a response header", async () => {
    const res = await request(app).get("/api/healthz");
    expect(res.headers["x-powered-by"]).toBeUndefined();
  });
});

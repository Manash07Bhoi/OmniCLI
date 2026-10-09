import path from "node:path";
import { promises as fs } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import request from "supertest";
import { fixturesDir, sandbox } from "./helpers";

/**
 * `/api/readyz` must never report ready for something it did not measure, so
 * the failure path is exercised for real: the binary search root is pinned at
 * an empty directory and PATH is emptied, which is the same situation as a
 * deployment where `omnicli` was never installed.
 *
 * A fresh module instance is required because the app caches the resolved
 * binary once it has run successfully.
 */
describe("readiness with no CLI available", () => {
  it("answers 503 and names the failing dependency, while still reporting the database honestly", async () => {
    const sb = sandbox();
    const emptyRoot = path.join(fixturesDir(), "no-binary-for-readiness");
    await fs.mkdir(emptyRoot, { recursive: true });

    const saved = {
      root: process.env["OMNICLI_ROOT"],
      path: process.env["PATH"],
      bin: process.env["OMNICLI_BIN"],
      db: process.env["DB_PATH"],
      xdg: process.env["XDG_DATA_HOME"],
      home: process.env["HOME"],
    };

    let status = 0;
    let body: { status?: string; checks?: Record<string, string> } = {};
    try {
      process.env["DB_PATH"] = sb.dbPath;
      process.env["XDG_DATA_HOME"] = sb.data;
      process.env["HOME"] = sb.home;
      process.env["OMNICLI_ROOT"] = emptyRoot;
      process.env["PATH"] = emptyRoot;
      delete process.env["OMNICLI_BIN"];

      vi.resetModules();
      const app = (await import("../src/app")).default;
      const res = await request(app).get("/api/readyz");
      status = res.status;
      body = res.body;
    } finally {
      for (const [key, value] of Object.entries({
        OMNICLI_ROOT: saved.root,
        PATH: saved.path,
        OMNICLI_BIN: saved.bin,
        DB_PATH: saved.db,
        XDG_DATA_HOME: saved.xdg,
        HOME: saved.home,
      })) {
        if (value === undefined) delete process.env[key];
        else process.env[key] = value;
      }
      vi.resetModules();
    }

    expect(status).toBe(503);
    expect(body.status).toBe("not-ready");
    expect(body.checks?.["cli"]).toBe("unavailable");
    // The database check is independent of the CLI and must still report the
    // truth rather than inheriting the CLI's failure.
    expect(body.checks?.["database"]).toBe("ok");
  });
});

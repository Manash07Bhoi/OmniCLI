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

describe("GET /api/convert/formats", () => {
  it("reflects the CLI's real conversion table", async () => {
    const res = await request(app).get("/api/convert/formats");
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
    expect(res.body.length).toBeGreaterThan(0);
    for (const pair of res.body) {
      expect(typeof pair.from).toBe("string");
      expect(typeof pair.to).toBe("string");
      expect(typeof pair.description).toBe("string");
    }

    const cli = await cliJson<{ from: string; to: string }[]>(["convert", "list"]);
    expect(res.body.length).toBe(cli.length);
  });
});

describe("POST /api/convert/run", () => {
  it("performs a real CSV → JSON conversion with real byte counts", async () => {
    const dir = await makeTree("convert-csv", {
      "data.csv": "name,age\nalice,30\nbob,25\n",
    });
    const input = path.join(dir, "data.csv");
    const output = path.join(dir, "data.json");
    const inputSize = (await fs.stat(input)).size;

    const res = await request(app)
      .post("/api/convert/run")
      .send({ inputPath: input, outputPath: output });

    expect(res.status).toBe(200);
    expect(res.body.fromFormat).toBe("csv");
    expect(res.body.toFormat).toBe("json");
    expect(res.body.inputSizeBytes).toBe(inputSize);
    expect(res.body.outputSizeBytes).toBeGreaterThan(0);

    const produced = JSON.parse(await fs.readFile(output, "utf8"));
    expect(produced).toEqual([
      { name: "alice", age: "30" },
      { name: "bob", age: "25" },
    ]);
  });

  it("performs a real JSON → YAML conversion", async () => {
    const dir = await makeTree("convert-yaml", { "config.json": '{"a":1,"b":[true,false]}' });
    const output = path.join(dir, "config.yaml");
    const res = await request(app)
      .post("/api/convert/run")
      .send({ inputPath: path.join(dir, "config.json"), outputPath: output });
    expect(res.status).toBe(200);
    const text = await fs.readFile(output, "utf8");
    expect(text).toContain("a: 1");
  });

  it("refuses an unsupported pair with the CLI's reason (no fake success)", async () => {
    const dir = await makeTree("convert-bad", { "file.txt": "hello" });
    const res = await request(app)
      .post("/api/convert/run")
      .send({ inputPath: path.join(dir, "file.txt"), outputPath: path.join(dir, "file.zzz") });
    expect(res.status).toBe(422);
    expect(res.body.error).toMatch(/unsupported/i);
  });

  it("returns 404 when the input file does not exist", async () => {
    const dir = await makeTempDir("convert-missing");
    const res = await request(app)
      .post("/api/convert/run")
      .send({ inputPath: path.join(dir, "nope.csv"), outputPath: path.join(dir, "out.json") });
    expect(res.status).toBe(404);
  });

  it("returns 400 when the output directory does not exist", async () => {
    const dir = await makeTree("convert-nodir", { "in.csv": "a,b\n1,2\n" });
    const res = await request(app)
      .post("/api/convert/run")
      .send({
        inputPath: path.join(dir, "in.csv"),
        outputPath: path.join(dir, "missing-dir", "out.json"),
      });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/output directory/i);
  });

  it("refuses to overwrite the input with itself", async () => {
    const dir = await makeTree("convert-same", { "in.csv": "a,b\n1,2\n" });
    const same = path.join(dir, "in.csv");
    const res = await request(app)
      .post("/api/convert/run")
      .send({ inputPath: same, outputPath: same });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/must differ/i);
  });

  it("rejects a malformed body", async () => {
    const res = await request(app).post("/api/convert/run").send({ inputPath: "" });
    expect(res.status).toBe(400);
  });
});

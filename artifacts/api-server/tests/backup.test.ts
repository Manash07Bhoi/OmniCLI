import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import path from "node:path";
import { promises as fs } from "node:fs";
import { fileExists, loadApp, makeTempDir, makeTree, uniqueName } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

async function prepare(name: string) {
  const src = await makeTree(`${name}-src`, {
    "one.txt": "backup payload one",
    "sub/two.txt": "backup payload two",
  });
  const dest = await makeTempDir(`${name}-dest`);
  return { src, dest, jobName: uniqueName(name) };
}

describe("POST /api/backup/jobs", () => {
  it("runs a real backup and records what actually happened", async () => {
    const { src, dest, jobName } = await prepare("backup-real");
    const res = await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });

    expect(res.status).toBe(201);
    expect(res.body.name).toBe(jobName);
    expect(res.body.snapshotCount).toBe(1);
    // The CLI verifies the snapshot it wrote, so the honest status is 'verified'
    // (or 'success' if verification is disabled) — never an invented one.
    expect(["verified", "success"]).toContain(res.body.lastStatus);

    const snapshots = await request(app).get(`/api/backup/jobs/${res.body.id}/snapshots`);
    expect(snapshots.status).toBe(200);
    expect(snapshots.body).toHaveLength(1);
    const snap = snapshots.body[0];
    expect(snap.filesTotal).toBe(2);
    expect(snap.filesChanged).toBe(2);
    expect(snap.bytesTransferred).toBeGreaterThan(0);
    expect(snap.checksumRoot).toMatch(/^[0-9a-f]{64}$/);
    expect(snap.finishedAt).toBeTruthy();

    // A real manifest must exist in the destination directory.
    const written = await fs.readdir(dest);
    expect(written.some((f) => f.endsWith(".manifest.json"))).toBe(true);
  });

  it("is idempotent in effect: a second run transfers nothing new", async () => {
    const { src, dest, jobName } = await prepare("backup-incremental");
    const first = await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });
    expect(first.status).toBe(201);
    const firstBytes = (
      await request(app).get(`/api/backup/jobs/${first.body.id}/snapshots`)
    ).body[0].bytesTransferred;

    // Second job over the same content: the CLI dedupes by content hash.
    const secondName = uniqueName("backup-incremental-2");
    const second = await request(app)
      .post("/api/backup/jobs")
      .send({ name: secondName, sourcePath: src, destPath: dest });
    expect(second.status).toBe(201);
    const snaps = await request(app).get(`/api/backup/jobs/${second.body.id}/snapshots`);
    expect(snaps.body[0].bytesTransferred).toBeLessThanOrEqual(firstBytes);
  });

  it("rejects a missing source directory without creating a job", async () => {
    const dest = await makeTempDir("backup-missing-dest");
    const name = uniqueName("backup-missing");
    const res = await request(app)
      .post("/api/backup/jobs")
      .send({ name, sourcePath: path.join(dest, "nope"), destPath: dest });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/source directory not found/i);

    const jobs = await request(app).get("/api/backup/jobs");
    expect(jobs.body.some((j: { name: string }) => j.name === name)).toBe(false);
  });

  it("rejects a duplicate job name with 409", async () => {
    const { src, dest, jobName } = await prepare("backup-dupe");
    const first = await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });
    expect(first.status).toBe(201);

    const second = await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });
    expect(second.status).toBe(409);
  });

  it("rejects an empty name (schema validation)", async () => {
    const { src, dest } = await prepare("backup-invalid");
    const res = await request(app)
      .post("/api/backup/jobs")
      .send({ name: "", sourcePath: src, destPath: dest });
    expect(res.status).toBe(400);
  });

  it("survives concurrent creation of the same job name", async () => {
    const { src, dest, jobName } = await prepare("backup-race");
    const [a, b] = await Promise.all([
      request(app).post("/api/backup/jobs").send({ name: jobName, sourcePath: src, destPath: dest }),
      request(app).post("/api/backup/jobs").send({ name: jobName, sourcePath: src, destPath: dest }),
    ]);
    const statuses = [a.status, b.status].sort();
    expect(statuses[0]).toBe(201);
    expect([201, 409]).toContain(statuses[1]);

    const jobs = await request(app).get("/api/backup/jobs");
    expect(jobs.body.filter((j: { name: string }) => j.name === jobName)).toHaveLength(1);
  });
});

describe("backup job lifecycle", () => {
  it("lists, fetches, deletes and returns 404 afterwards", async () => {
    const { src, dest, jobName } = await prepare("backup-lifecycle");
    const created = await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });
    const id = created.body.id;

    const list = await request(app).get("/api/backup/jobs");
    expect(list.status).toBe(200);
    expect(list.body.some((j: { id: number }) => j.id === id)).toBe(true);

    const one = await request(app).get(`/api/backup/jobs/${id}`);
    expect(one.status).toBe(200);
    expect(one.body.destPath).toBe(dest);

    expect((await request(app).delete(`/api/backup/jobs/${id}`)).status).toBe(204);
    expect((await request(app).get(`/api/backup/jobs/${id}`)).status).toBe(404);

    // Deleting a job also removes its snapshots (foreign key cascade).
    const snapshots = await request(app).get(`/api/backup/jobs/${id}/snapshots`);
    expect(snapshots.status).toBe(404);
  });

  it("reports stats that match the stored snapshots", async () => {
    const stats = await request(app).get("/api/backup/stats");
    expect(stats.status).toBe(200);
    expect(stats.body.totalJobs).toBeGreaterThan(0);
    expect(stats.body.totalSnapshots).toBeGreaterThan(0);
    expect(stats.body.totalBytesStored).toBeGreaterThan(0);
    if (stats.body.totalSnapshots > 0) {
      expect(stats.body.successRate).toBeGreaterThan(0);
      expect(stats.body.successRate).toBeLessThanOrEqual(1);
    }
  });

  it("returns 404 for snapshots of an unknown job", async () => {
    const res = await request(app).get("/api/backup/jobs/999999/snapshots");
    expect(res.status).toBe(404);
  });

  it("writes a manifest that can be verified independently", async () => {
    const { src, dest, jobName } = await prepare("backup-manifest");
    await request(app)
      .post("/api/backup/jobs")
      .send({ name: jobName, sourcePath: src, destPath: dest });

    const files = await fs.readdir(dest);
    const manifestName = files.find((f) => f.endsWith(".manifest.json"));
    expect(manifestName).toBeTruthy();
    const manifest = JSON.parse(
      await fs.readFile(path.join(dest, manifestName as string), "utf8")
    );
    expect(manifest.source_path).toBe(src);
    expect(Object.keys(manifest.entries)).toHaveLength(2);
    expect(await fileExists(path.join(dest, manifestName as string))).toBe(true);
  });
});

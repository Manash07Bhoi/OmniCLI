import { Router, type IRouter } from "express";
import { promises as fs } from "node:fs";
import { db } from "@workspace/db";
import {
  backupJobsTable,
  backupSnapshotsTable,
  activityLogTable,
} from "@workspace/db";
import { count, eq, desc, max, sum } from "drizzle-orm";
import {
  ListBackupJobsResponse,
  CreateBackupJobBody,
  CreateBackupJobResponse,
  GetBackupJobParams,
  GetBackupJobResponse,
  DeleteBackupJobParams,
  ListBackupSnapshotsParams,
  ListBackupSnapshotsResponse,
  GetBackupStatsResponse,
} from "@workspace/api-zod";
import { blake3 } from "@noble/hashes/blake3.js";
import { bytesToHex } from "@noble/hashes/utils.js";
import { omniJsonOk, isOmniUnavailable } from "../lib/exec";
import { invalidRequest } from "../lib/validation";

const router: IRouter = Router();

/** `omnicli backup create --json` payload (snake_case, as emitted by the CLI). */
interface BackupCreateOutput {
  snapshot_id?: string;
  manifest_path?: string;
  source_path?: string;
  dest_path?: string;
  files_total?: number;
  files_new?: number;
  files_unchanged?: number;
  bytes_transferred?: number;
  duration_ms?: number;
  verified?: {
    passed?: boolean;
    files_checked?: number;
    files_corrupt?: number;
    files_missing?: number;
  };
}

async function jobWithMeta(id: number) {
  const [job] = await db
    .select()
    .from(backupJobsTable)
    .where(eq(backupJobsTable.id, id));
  if (!job) return null;

  const [snapshotMeta] = await db
    .select({
      snapshotCount: count(backupSnapshotsTable.id),
      lastSnapshotAt: max(backupSnapshotsTable.startedAt),
    })
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.jobId, id));

  // Get last status
  const [lastSnap] = await db
    .select({ status: backupSnapshotsTable.status })
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.jobId, id))
    .orderBy(desc(backupSnapshotsTable.startedAt))
    .limit(1);

  return {
    id: job.id,
    name: job.name,
    sourcePath: job.sourcePath,
    destPath: job.destPath,
    createdAt: job.createdAt.toISOString(),
    snapshotCount: Number(snapshotMeta?.snapshotCount ?? 0),
    lastSnapshotAt: snapshotMeta?.lastSnapshotAt
      ? new Date(snapshotMeta.lastSnapshotAt).toISOString()
      : null,
    lastStatus: lastSnap?.status ?? null,
  };
}

/**
 * Digest of the snapshot manifest the CLI wrote. The manifest lists every
 * backed-up file with its content hash, so hashing it yields a real root digest
 * for the snapshot ("" when no manifest was produced).
 */
async function manifestDigest(manifestPath: string | undefined): Promise<string> {
  if (!manifestPath) return "";
  try {
    const bytes = await fs.readFile(manifestPath);
    return bytesToHex(blake3(bytes));
  } catch {
    return "";
  }
}

router.get("/backup/jobs", async (_req, res): Promise<void> => {
  const jobs = await db
    .select()
    .from(backupJobsTable)
    .orderBy(desc(backupJobsTable.createdAt));

  const result = await Promise.all(jobs.map((j) => jobWithMeta(j.id)));
  res.json(ListBackupJobsResponse.parse(result.filter(Boolean)));
});

router.post("/backup/jobs", async (req, res): Promise<void> => {
  const parsed = CreateBackupJobBody.safeParse(req.body);
  if (!parsed.success) {
    invalidRequest(res, parsed.error);
    return;
  }

  const { name, sourcePath, destPath } = parsed.data;

  const sourceStat = await fs.stat(sourcePath).catch(() => null);
  if (!sourceStat?.isDirectory()) {
    res.status(400).json({ error: `Source directory not found: ${sourcePath}` });
    return;
  }

  const [existing] = await db
    .select({ id: backupJobsTable.id })
    .from(backupJobsTable)
    .where(eq(backupJobsTable.name, name));
  if (existing) {
    res.status(409).json({ error: `A backup job named "${name}" already exists.` });
    return;
  }

  const [job] = await db
    .insert(backupJobsTable)
    .values({ name, sourcePath, destPath })
    .returning();

  if (!job) {
    res.status(500).json({ error: "Failed to create backup job" });
    return;
  }

  const startedAt = new Date();
  let status: "success" | "verified" | "failed" = "failed";
  let filesTotal: number | null = null;
  let filesChanged: number | null = null;
  let bytesTransferred: number | null = null;
  let checksumRoot = "";
  let failure = "";

  try {
    // Run the real backup. The CLI performs content-hash deduplication and
    // verifies the snapshot it just wrote.
    const run = await omniJsonOk<BackupCreateOutput>(
      ["backup", "create", sourcePath, "--dest", destPath, "--name", name],
      600_000
    );
    status = run.verified?.passed ? "verified" : "success";
    filesTotal = run.files_total ?? null;
    filesChanged = run.files_new ?? null;
    bytesTransferred = run.bytes_transferred ?? null;
    checksumRoot = await manifestDigest(run.manifest_path);
  } catch (err: unknown) {
    if (isOmniUnavailable(err)) {
      // No snapshot was taken, so remove the job row rather than leaving a
      // phantom job behind.
      await db.delete(backupJobsTable).where(eq(backupJobsTable.id, job.id));
      res.status(503).json({ error: (err as Error).message });
      return;
    }
    failure = (err as Error).message;
  }

  await db.insert(backupSnapshotsTable).values({
    jobId: job.id,
    startedAt,
    finishedAt: new Date(),
    status,
    filesTotal,
    filesChanged,
    bytesTransferred,
    checksumRoot,
  });

  await db
    .insert(activityLogTable)
    .values({
      type: "backup",
      description: failure
        ? `Backup job "${name}" failed: ${failure}`
        : `Backup job "${name}" ${status} (${sourcePath} → ${destPath})`,
    })
    .catch(() => {});

  const meta = await jobWithMeta(job.id);
  // 201 means the job was created; `lastStatus` reports what the actual run did.
  res.status(201).json(CreateBackupJobResponse.parse(meta));
});

router.get("/backup/jobs/:id", async (req, res): Promise<void> => {
  const params = GetBackupJobParams.safeParse(req.params);
  if (!params.success) {
    invalidRequest(res, params.error);
    return;
  }

  const meta = await jobWithMeta(params.data.id);
  if (!meta) {
    res.status(404).json({ error: "Backup job not found" });
    return;
  }

  res.json(GetBackupJobResponse.parse(meta));
});

router.delete("/backup/jobs/:id", async (req, res): Promise<void> => {
  const params = DeleteBackupJobParams.safeParse(req.params);
  if (!params.success) {
    invalidRequest(res, params.error);
    return;
  }

  const [deleted] = await db
    .delete(backupJobsTable)
    .where(eq(backupJobsTable.id, params.data.id))
    .returning();

  if (!deleted) {
    res.status(404).json({ error: "Backup job not found" });
    return;
  }

  res.sendStatus(204);
});

router.get("/backup/jobs/:id/snapshots", async (req, res): Promise<void> => {
  const params = ListBackupSnapshotsParams.safeParse(req.params);
  if (!params.success) {
    invalidRequest(res, params.error);
    return;
  }

  const [job] = await db
    .select()
    .from(backupJobsTable)
    .where(eq(backupJobsTable.id, params.data.id));

  if (!job) {
    res.status(404).json({ error: "Backup job not found" });
    return;
  }

  const snapshots = await db
    .select()
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.jobId, params.data.id))
    .orderBy(desc(backupSnapshotsTable.startedAt));

  const result = snapshots.map((s) => ({
    id: s.id,
    jobId: s.jobId,
    startedAt: s.startedAt.toISOString(),
    finishedAt: s.finishedAt?.toISOString() ?? null,
    filesTotal: s.filesTotal ?? null,
    filesChanged: s.filesChanged ?? null,
    bytesTransferred: s.bytesTransferred ?? null,
    status: s.status as "running" | "success" | "failed" | "verified" | "interrupted",
    checksumRoot: s.checksumRoot,
  }));

  res.json(ListBackupSnapshotsResponse.parse(result));
});

router.get("/backup/stats", async (_req, res): Promise<void> => {
  const [[jobCount], [snapCount]] = await Promise.all([
    db.select({ count: count() }).from(backupJobsTable),
    db.select({
      count: count(),
      bytes: sum(backupSnapshotsTable.bytesTransferred),
      lastAt: max(backupSnapshotsTable.startedAt),
    }).from(backupSnapshotsTable),
  ]);

  const totalSnapshots = Number(snapCount?.count ?? 0);
  // Snapshots count as successful only when the run succeeded or verified.
  const goodSnaps = await db
    .select({ count: count() })
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.status, "success"));
  const verifiedSnaps = await db
    .select({ count: count() })
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.status, "verified"));

  const successCount = Number(goodSnaps[0]?.count ?? 0) + Number(verifiedSnaps[0]?.count ?? 0);

  res.json(
    GetBackupStatsResponse.parse({
      totalJobs: Number(jobCount?.count ?? 0),
      totalSnapshots,
      totalBytesStored: Number(snapCount?.bytes ?? 0),
      lastBackupAt: snapCount?.lastAt ? new Date(snapCount.lastAt).toISOString() : null,
      // Omitted entirely when there is nothing to measure, rather than
      // reporting a made-up 100%.
      ...(totalSnapshots > 0 ? { successRate: successCount / totalSnapshots } : {}),
    })
  );
});

export default router;

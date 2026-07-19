import { Router, type IRouter } from "express";
import crypto from "crypto";
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

const router: IRouter = Router();

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
    res.status(400).json({ error: parsed.error.message });
    return;
  }

  const { name, sourcePath, destPath } = parsed.data;

  const [job] = await db
    .insert(backupJobsTable)
    .values({ name, sourcePath, destPath })
    .returning();

  if (!job) {
    res.status(500).json({ error: "Failed to create backup job" });
    return;
  }

  // Create an initial snapshot record to simulate running a backup
  await db.insert(backupSnapshotsTable).values({
    jobId: job.id,
    status: "success",
    checksumRoot: crypto.randomUUID?.() ?? `root-${job.id}`,
    filesTotal: 0,
    filesChanged: 0,
    bytesTransferred: 0,
    finishedAt: new Date(),
  });

  await db
    .insert(activityLogTable)
    .values({ type: "backup", description: `Backup job "${name}" created (${sourcePath} → ${destPath})` })
    .catch(() => {});

  const meta = await jobWithMeta(job.id);
  res.status(201).json(CreateBackupJobResponse.parse(meta));
});

router.get("/backup/jobs/:id", async (req, res): Promise<void> => {
  const params = GetBackupJobParams.safeParse(req.params);
  if (!params.success) {
    res.status(400).json({ error: params.error.message });
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
    res.status(400).json({ error: params.error.message });
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
    res.status(400).json({ error: params.error.message });
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
  const successSnaps = await db
    .select({ count: count() })
    .from(backupSnapshotsTable)
    .where(eq(backupSnapshotsTable.status, "success"));

  const successCount = Number(successSnaps[0]?.count ?? 0);
  const successRate = totalSnapshots > 0 ? successCount / totalSnapshots : 1;

  res.json(
    GetBackupStatsResponse.parse({
      totalJobs: Number(jobCount?.count ?? 0),
      totalSnapshots,
      totalBytesStored: Number(snapCount?.bytes ?? 0),
      lastBackupAt: snapCount?.lastAt ? new Date(snapCount.lastAt).toISOString() : null,
      successRate,
    })
  );
});

export default router;

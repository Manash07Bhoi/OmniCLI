import { sqliteTable, integer, text } from "drizzle-orm/sqlite-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod/v4";

export const backupJobsTable = sqliteTable("backup_jobs", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  name: text("name").notNull().unique(),
  sourcePath: text("source_path").notNull(),
  destPath: text("dest_path").notNull(),
  createdAt: integer("created_at", { mode: "timestamp_ms" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const backupSnapshotsTable = sqliteTable("backup_snapshots", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  jobId: integer("job_id")
    .notNull()
    .references(() => backupJobsTable.id, { onDelete: "cascade" }),
  startedAt: integer("started_at", { mode: "timestamp_ms" })
    .notNull()
    .$defaultFn(() => new Date()),
  finishedAt: integer("finished_at", { mode: "timestamp_ms" }),
  filesTotal: integer("files_total"),
  filesChanged: integer("files_changed"),
  bytesTransferred: integer("bytes_transferred"),
  status: text("status").notNull().default("running"),
  checksumRoot: text("checksum_root").notNull().default(""),
});

export const insertBackupJobSchema = createInsertSchema(backupJobsTable).omit({
  id: true,
  createdAt: true,
});
export type InsertBackupJob = z.infer<typeof insertBackupJobSchema>;
export type BackupJob = typeof backupJobsTable.$inferSelect;

export const insertBackupSnapshotSchema = createInsertSchema(
  backupSnapshotsTable
).omit({ id: true });
export type InsertBackupSnapshot = z.infer<typeof insertBackupSnapshotSchema>;
export type BackupSnapshot = typeof backupSnapshotsTable.$inferSelect;

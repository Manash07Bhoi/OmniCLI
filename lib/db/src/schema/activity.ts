import { sqliteTable, integer, text } from "drizzle-orm/sqlite-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod/v4";

export const activityLogTable = sqliteTable("activity_log", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  type: text("type").notNull(), // search|backup|convert|note|todo|snippet|file_find
  description: text("description").notNull(),
  timestamp: integer("timestamp", { mode: "timestamp_ms" })
    .notNull()
    .$defaultFn(() => new Date()),
  metadata: text("metadata"), // JSON string with extra context
});

export const insertActivitySchema = createInsertSchema(activityLogTable).omit({
  id: true,
  timestamp: true,
});
export type InsertActivity = z.infer<typeof insertActivitySchema>;
export type ActivityLog = typeof activityLogTable.$inferSelect;

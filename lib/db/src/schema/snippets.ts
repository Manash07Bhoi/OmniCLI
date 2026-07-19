import { sqliteTable, integer, text } from "drizzle-orm/sqlite-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod/v4";

export const snippetsTable = sqliteTable("snippets", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  name: text("name").notNull().unique(),
  language: text("language"),
  body: text("body").notNull(),
  createdAt: integer("created_at", { mode: "timestamp_ms" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const insertSnippetSchema = createInsertSchema(snippetsTable).omit({
  id: true,
  createdAt: true,
});
export type InsertSnippet = z.infer<typeof insertSnippetSchema>;
export type Snippet = typeof snippetsTable.$inferSelect;

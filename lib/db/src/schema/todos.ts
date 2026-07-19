import { sqliteTable, integer, text } from "drizzle-orm/sqlite-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod/v4";

export const todosTable = sqliteTable("todos", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  description: text("description").notNull(),
  done: integer("done", { mode: "boolean" }).notNull().default(false),
  dueAt: integer("due_at", { mode: "timestamp_ms" }),
  createdAt: integer("created_at", { mode: "timestamp_ms" })
    .notNull()
    .$defaultFn(() => new Date()),
});

export const insertTodoSchema = createInsertSchema(todosTable).omit({
  id: true,
  createdAt: true,
});
export type InsertTodo = z.infer<typeof insertTodoSchema>;
export type Todo = typeof todosTable.$inferSelect;

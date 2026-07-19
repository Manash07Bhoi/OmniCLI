import { Router, type IRouter } from "express";
import { db } from "@workspace/db";
import {
  notesTable,
  todosTable,
  snippetsTable,
  activityLogTable,
} from "@workspace/db";
import { count, eq, desc, like } from "drizzle-orm";
import {
  ListNotesQueryParams,
  ListNotesResponse,
  CreateNoteBody,
  CreateNoteResponse,
  GetNoteParams,
  GetNoteResponse,
  UpdateNoteParams,
  UpdateNoteBody,
  UpdateNoteResponse,
  DeleteNoteParams,
  ListTodosQueryParams,
  ListTodosResponse,
  CreateTodoBody,
  CreateTodoResponse,
  UpdateTodoParams,
  UpdateTodoBody,
  UpdateTodoResponse,
  DeleteTodoParams,
  ListSnippetsQueryParams,
  ListSnippetsResponse,
  CreateSnippetBody,
  CreateSnippetResponse,
  DeleteSnippetParams,
  GetWorkspaceStatsResponse,
} from "@workspace/api-zod";

const router: IRouter = Router();

// ── Notes ──────────────────────────────────────────────────────────────────
function serializeNote(n: typeof notesTable.$inferSelect) {
  return {
    id: n.id,
    title: n.title,
    body: n.body,
    tags: n.tags ?? null,
    createdAt: n.createdAt.toISOString(),
    updatedAt: n.updatedAt.toISOString(),
  };
}

router.get("/workspace/notes", async (req, res): Promise<void> => {
  const params = ListNotesQueryParams.safeParse(req.query);
  const search = params.success ? params.data.search : undefined;

  const notes = search
    ? await db.select().from(notesTable).where(like(notesTable.title, `%${search}%`)).orderBy(desc(notesTable.updatedAt))
    : await db.select().from(notesTable).orderBy(desc(notesTable.updatedAt));

  res.json(ListNotesResponse.parse(notes.map(serializeNote)));
});

router.post("/workspace/notes", async (req, res): Promise<void> => {
  const parsed = CreateNoteBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.message });
    return;
  }
  const [note] = await db
    .insert(notesTable)
    .values({ title: parsed.data.title, body: parsed.data.body ?? "", tags: parsed.data.tags })
    .returning();
  await db.insert(activityLogTable).values({ type: "note", description: `Note created: "${parsed.data.title}"` }).catch(() => {});
  res.status(201).json(CreateNoteResponse.parse(serializeNote(note!)));
});

router.get("/workspace/notes/:id", async (req, res): Promise<void> => {
  const params = GetNoteParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const [note] = await db.select().from(notesTable).where(eq(notesTable.id, params.data.id));
  if (!note) { res.status(404).json({ error: "Note not found" }); return; }
  res.json(GetNoteResponse.parse(serializeNote(note)));
});

router.patch("/workspace/notes/:id", async (req, res): Promise<void> => {
  const params = UpdateNoteParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const body = UpdateNoteBody.safeParse(req.body);
  if (!body.success) { res.status(400).json({ error: body.error.message }); return; }
  const [note] = await db
    .update(notesTable)
    .set({ ...body.data, updatedAt: new Date() })
    .where(eq(notesTable.id, params.data.id))
    .returning();
  if (!note) { res.status(404).json({ error: "Note not found" }); return; }
  res.json(UpdateNoteResponse.parse(serializeNote(note)));
});

router.delete("/workspace/notes/:id", async (req, res): Promise<void> => {
  const params = DeleteNoteParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const [d] = await db.delete(notesTable).where(eq(notesTable.id, params.data.id)).returning();
  if (!d) { res.status(404).json({ error: "Note not found" }); return; }
  res.sendStatus(204);
});

// ── Todos ──────────────────────────────────────────────────────────────────
function serializeTodo(t: typeof todosTable.$inferSelect) {
  return {
    id: t.id,
    description: t.description,
    done: t.done,
    dueAt: t.dueAt?.toISOString() ?? null,
    createdAt: t.createdAt.toISOString(),
  };
}

router.get("/workspace/todos", async (req, res): Promise<void> => {
  const params = ListTodosQueryParams.safeParse(req.query);
  const doneFilter = params.success ? params.data.done : undefined;

  const todos = await db.select().from(todosTable).orderBy(desc(todosTable.createdAt));
  const filtered = doneFilter !== undefined ? todos.filter((t) => t.done === doneFilter) : todos;
  res.json(ListTodosResponse.parse(filtered.map(serializeTodo)));
});

router.post("/workspace/todos", async (req, res): Promise<void> => {
  const parsed = CreateTodoBody.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: parsed.error.message }); return; }
  const [todo] = await db
    .insert(todosTable)
    .values({ description: parsed.data.description, dueAt: parsed.data.dueAt ? new Date(parsed.data.dueAt) : null })
    .returning();
  await db.insert(activityLogTable).values({ type: "todo", description: `Todo created: "${parsed.data.description}"` }).catch(() => {});
  res.status(201).json(CreateTodoResponse.parse(serializeTodo(todo!)));
});

router.patch("/workspace/todos/:id", async (req, res): Promise<void> => {
  const params = UpdateTodoParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const body = UpdateTodoBody.safeParse(req.body);
  if (!body.success) { res.status(400).json({ error: body.error.message }); return; }
  const updateData: Partial<typeof todosTable.$inferInsert> = {};
  if (body.data.description !== undefined) updateData.description = body.data.description;
  if (body.data.done !== undefined) updateData.done = body.data.done;
  if ("dueAt" in body.data) updateData.dueAt = body.data.dueAt ? new Date(body.data.dueAt) : null;
  const [todo] = await db.update(todosTable).set(updateData).where(eq(todosTable.id, params.data.id)).returning();
  if (!todo) { res.status(404).json({ error: "Todo not found" }); return; }
  res.json(UpdateTodoResponse.parse(serializeTodo(todo)));
});

router.delete("/workspace/todos/:id", async (req, res): Promise<void> => {
  const params = DeleteTodoParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const [d] = await db.delete(todosTable).where(eq(todosTable.id, params.data.id)).returning();
  if (!d) { res.status(404).json({ error: "Todo not found" }); return; }
  res.sendStatus(204);
});

// ── Snippets ───────────────────────────────────────────────────────────────
function serializeSnippet(s: typeof snippetsTable.$inferSelect) {
  return {
    id: s.id,
    name: s.name,
    language: s.language ?? null,
    body: s.body,
    createdAt: s.createdAt.toISOString(),
  };
}

router.get("/workspace/snippets", async (req, res): Promise<void> => {
  const params = ListSnippetsQueryParams.safeParse(req.query);
  const langFilter = params.success ? params.data.language : undefined;
  const snippets = await db.select().from(snippetsTable).orderBy(desc(snippetsTable.createdAt));
  const filtered = langFilter ? snippets.filter((s) => s.language === langFilter) : snippets;
  res.json(ListSnippetsResponse.parse(filtered.map(serializeSnippet)));
});

router.post("/workspace/snippets", async (req, res): Promise<void> => {
  const parsed = CreateSnippetBody.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: parsed.error.message }); return; }
  const [snippet] = await db
    .insert(snippetsTable)
    .values({ name: parsed.data.name, language: parsed.data.language, body: parsed.data.body })
    .returning();
  await db.insert(activityLogTable).values({ type: "snippet", description: `Snippet saved: "${parsed.data.name}"` }).catch(() => {});
  res.status(201).json(CreateSnippetResponse.parse(serializeSnippet(snippet!)));
});

router.delete("/workspace/snippets/:id", async (req, res): Promise<void> => {
  const params = DeleteSnippetParams.safeParse(req.params);
  if (!params.success) { res.status(400).json({ error: params.error.message }); return; }
  const [d] = await db.delete(snippetsTable).where(eq(snippetsTable.id, params.data.id)).returning();
  if (!d) { res.status(404).json({ error: "Snippet not found" }); return; }
  res.sendStatus(204);
});

// ── Workspace stats ────────────────────────────────────────────────────────
router.get("/workspace/stats", async (_req, res): Promise<void> => {
  const [[noteCount], [todoCount], [pendingCount], [snippetCount], langs] = await Promise.all([
    db.select({ count: count() }).from(notesTable),
    db.select({ count: count() }).from(todosTable),
    db.select({ count: count() }).from(todosTable).where(eq(todosTable.done, false)),
    db.select({ count: count() }).from(snippetsTable),
    db.select({ language: snippetsTable.language }).from(snippetsTable),
  ]);

  const languageSet = new Set(langs.map((l) => l.language).filter(Boolean) as string[]);

  res.json(
    GetWorkspaceStatsResponse.parse({
      totalNotes: Number(noteCount?.count ?? 0),
      totalTodos: Number(todoCount?.count ?? 0),
      pendingTodos: Number(pendingCount?.count ?? 0),
      totalSnippets: Number(snippetCount?.count ?? 0),
      languages: Array.from(languageSet),
    })
  );
});

export default router;

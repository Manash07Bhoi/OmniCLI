import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";
import type { Express } from "express";
import { loadApp, uniqueName } from "./helpers";

let app: Express;

beforeAll(async () => {
  app = await loadApp();
});

describe("workspace notes", () => {
  it("creates, lists, updates and deletes a note", async () => {
    const title = uniqueName("note");

    const created = await request(app)
      .post("/api/workspace/notes")
      .send({ title, body: "first body", tags: "test" });
    expect(created.status).toBe(201);
    expect(created.body.id).toBeGreaterThan(0);
    expect(Date.parse(created.body.createdAt)).not.toBeNaN();
    const id = created.body.id as number;

    const list = await request(app).get("/api/workspace/notes");
    expect(list.status).toBe(200);
    expect(list.body.some((n: { id: number }) => n.id === id)).toBe(true);

    const fetched = await request(app).get(`/api/workspace/notes/${id}`);
    expect(fetched.status).toBe(200);
    expect(fetched.body.body).toBe("first body");

    const patched = await request(app)
      .patch(`/api/workspace/notes/${id}`)
      .send({ body: "second body" });
    expect(patched.status).toBe(200);
    expect(patched.body.body).toBe("second body");
    expect(Date.parse(patched.body.updatedAt)).toBeGreaterThanOrEqual(
      Date.parse(patched.body.createdAt)
    );

    expect((await request(app).delete(`/api/workspace/notes/${id}`)).status).toBe(204);
    expect((await request(app).get(`/api/workspace/notes/${id}`)).status).toBe(404);
  });

  it("rejects an empty title", async () => {
    const res = await request(app).post("/api/workspace/notes").send({ title: "" });
    expect(res.status).toBe(400);
  });

  it("returns 404 for an unknown id and 400 for a non-numeric id", async () => {
    expect((await request(app).get("/api/workspace/notes/424242")).status).toBe(404);
    expect((await request(app).get("/api/workspace/notes/not-a-number")).status).toBe(400);
  });

  it("filters by search term", async () => {
    const needle = uniqueName("searchable-note");
    await request(app).post("/api/workspace/notes").send({ title: needle });
    await request(app).post("/api/workspace/notes").send({ title: uniqueName("other") });

    const res = await request(app).get("/api/workspace/notes").query({ search: needle });
    expect(res.status).toBe(200);
    expect(res.body).toHaveLength(1);
    expect(res.body[0].title).toBe(needle);
  });
});

describe("workspace todos", () => {
  it("creates, toggles and deletes a todo", async () => {
    const description = uniqueName("todo");
    const created = await request(app)
      .post("/api/workspace/todos")
      .send({ description });
    expect(created.status).toBe(201);
    expect(created.body.done).toBe(false);
    const id = created.body.id as number;

    const toggled = await request(app)
      .patch(`/api/workspace/todos/${id}`)
      .send({ done: true });
    expect(toggled.status).toBe(200);
    expect(toggled.body.done).toBe(true);

    const list = await request(app).get("/api/workspace/todos");
    const found = list.body.find((t: { id: number }) => t.id === id);
    expect(found?.done).toBe(true);

    expect((await request(app).delete(`/api/workspace/todos/${id}`)).status).toBe(204);
  });

  it("rejects an empty description", async () => {
    const res = await request(app).post("/api/workspace/todos").send({ description: "" });
    expect(res.status).toBe(400);
  });
});

describe("workspace snippets", () => {
  it("stores and removes a snippet, rejecting duplicates", async () => {
    const name = uniqueName("snippet");
    const created = await request(app)
      .post("/api/workspace/snippets")
      .send({ name, language: "rust", body: "fn main() {}" });
    expect(created.status).toBe(201);
    expect(created.body.language).toBe("rust");

    const list = await request(app).get("/api/workspace/snippets");
    expect(list.body.some((s: { name: string }) => s.name === name)).toBe(true);

    // A second snippet with the same name violates the unique constraint and
    // must surface as an error rather than a silent overwrite.
    const dupe = await request(app)
      .post("/api/workspace/snippets")
      .send({ name, body: "different body" });
    // A unique-name clash is a client error, not a 500.
    expect(dupe.status).toBe(409);

    expect((await request(app).delete(`/api/workspace/snippets/${created.body.id}`)).status).toBe(204);
  });

  it("rejects an empty body", async () => {
    const res = await request(app)
      .post("/api/workspace/snippets")
      .send({ name: uniqueName("empty"), body: "" });
    expect(res.status).toBe(400);
  });
});

describe("workspace stats", () => {
  it("reports counts consistent with the list endpoints", async () => {
    const [stats, notes, todos, snippets] = await Promise.all([
      request(app).get("/api/workspace/stats"),
      request(app).get("/api/workspace/notes"),
      request(app).get("/api/workspace/todos"),
      request(app).get("/api/workspace/snippets"),
    ]);
    expect(stats.status).toBe(200);
    expect(stats.body.totalNotes).toBe(notes.body.length);
    expect(stats.body.totalTodos).toBe(todos.body.length);
    expect(stats.body.totalSnippets).toBe(snippets.body.length);
    expect(Array.isArray(stats.body.languages)).toBe(true);
  });
});

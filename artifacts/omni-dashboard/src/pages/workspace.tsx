import { useState } from "react";
import {
  useListNotes, useListTodos, useListSnippets,
  useCreateNote, useCreateTodo, useUpdateTodo, useCreateSnippet,
} from "@workspace/api-client-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { BookOpen, Plus, CheckSquare, Square, Code, FileText, RefreshCw } from "lucide-react";
import { SkeletonCard, ErrorState, EmptyState } from "@/components/ui/states";

export default function WorkspacePage() {
  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
          <BookOpen className="w-6 h-6 sm:w-8 sm:h-8" />
          Workspace
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Notes, todos, and code snippets — stored in your local SQLite workspace.
        </p>
      </div>
      <Tabs defaultValue="notes">
        <TabsList className="bg-muted/30 border border-border">
          <TabsTrigger value="notes" className="gap-1.5 text-xs sm:text-sm"><FileText className="w-3.5 h-3.5" />Notes</TabsTrigger>
          <TabsTrigger value="todos" className="gap-1.5 text-xs sm:text-sm"><CheckSquare className="w-3.5 h-3.5" />Todos</TabsTrigger>
          <TabsTrigger value="snippets" className="gap-1.5 text-xs sm:text-sm"><Code className="w-3.5 h-3.5" />Snippets</TabsTrigger>
        </TabsList>
        <TabsContent value="notes" className="mt-4"><NotesTab /></TabsContent>
        <TabsContent value="todos" className="mt-4"><TodosTab /></TabsContent>
        <TabsContent value="snippets" className="mt-4"><SnippetsTab /></TabsContent>
      </Tabs>
    </div>
  );
}

function NotesTab() {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const { data: notes, isLoading, isError, error, refetch } = useListNotes({});
  const createNote = useCreateNote();

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    createNote.mutate({ data: { title, body, tags: undefined } }, {
      onSuccess: () => { setOpen(false); setTitle(""); setBody(""); refetch(); },
    });
  };

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Button size="sm" onClick={() => setOpen(!open)} className="gap-2">
          <Plus className="w-4 h-4" />New Note
        </Button>
      </div>
      {open && (
        <Card className="border-primary/30 glow-border bg-card/80">
          <CardContent className="p-4">
            <form onSubmit={handleCreate} className="space-y-3">
              <Input placeholder="Note title" value={title} onChange={(e) => setTitle(e.target.value)} required autoFocus />
              <Textarea placeholder="Note body (optional)" value={body} onChange={(e) => setBody(e.target.value)} className="font-mono min-h-[100px]" />
              <div className="flex gap-2">
                <Button type="submit" size="sm" disabled={createNote.isPending}>
                  {createNote.isPending && <RefreshCw className="w-4 h-4 animate-spin mr-1" />}Save
                </Button>
                <Button type="button" size="sm" variant="secondary" onClick={() => setOpen(false)}>Cancel</Button>
              </div>
            </form>
          </CardContent>
        </Card>
      )}
      {isLoading ? (
        <div className="grid sm:grid-cols-2 gap-3">{Array.from({length: 4}).map((_,i)=><SkeletonCard key={i} rows={3}/>)}</div>
      ) : isError ? (
        <ErrorState compact title="Failed to load notes" message={(error as Error)?.message} onRetry={refetch} />
      ) : !notes?.length ? (
        <EmptyState icon={<FileText className="w-8 h-8 text-muted-foreground"/>} title="No notes yet" description="Create one above or run: omni workspace note new 'My Note'" />
      ) : (
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {notes.map((n) => (
            <Card key={n.id} className="border-border bg-card hover:border-primary/30 transition-colors">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-mono">{n.title}</CardTitle>
                {n.tags && (
                  <div className="flex gap-1 flex-wrap mt-1">
                    {n.tags.split(",").map(t => <Badge key={t} variant="secondary" className="text-[10px]">{t.trim()}</Badge>)}
                  </div>
                )}
              </CardHeader>
              <CardContent>
                <p className="text-xs text-muted-foreground line-clamp-3 font-mono">
                  {n.body || <em>Empty note</em>}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function TodosTab() {
  const [open, setOpen] = useState(false);
  const [desc, setDesc] = useState("");
  const { data: todos, isLoading, isError, error, refetch } = useListTodos({});
  const createTodo = useCreateTodo();
  const updateTodo = useUpdateTodo();

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!desc.trim()) return;
    createTodo.mutate({ data: { description: desc, dueAt: undefined } }, {
      onSuccess: () => { setOpen(false); setDesc(""); refetch(); },
    });
  };

  const handleToggle = (id: number, done: boolean) => {
    updateTodo.mutate({ id, data: { done: !done } }, { onSuccess: () => refetch() });
  };

  const pending = todos?.filter(t => !t.done) ?? [];
  const done = todos?.filter(t => t.done) ?? [];

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Button size="sm" onClick={() => setOpen(!open)} className="gap-2">
          <Plus className="w-4 h-4" />Add Todo
        </Button>
      </div>
      {open && (
        <Card className="border-primary/30 glow-border bg-card/80">
          <CardContent className="p-4">
            <form onSubmit={handleCreate} className="flex gap-2">
              <Input placeholder="What needs to be done?" value={desc} onChange={(e) => setDesc(e.target.value)} required autoFocus className="flex-1" />
              <Button type="submit" size="sm" disabled={createTodo.isPending}>Add</Button>
              <Button type="button" size="sm" variant="secondary" onClick={() => setOpen(false)}>Cancel</Button>
            </form>
          </CardContent>
        </Card>
      )}
      {isLoading ? (
        <div className="space-y-2">{Array.from({length:5}).map((_,i)=><div key={i} className="h-12 rounded-sm bg-muted/40 animate-pulse border border-border"/>)}</div>
      ) : isError ? (
        <ErrorState compact title="Failed to load todos" message={(error as Error)?.message} onRetry={refetch} />
      ) : !todos?.length ? (
        <EmptyState icon={<CheckSquare className="w-8 h-8 text-muted-foreground"/>} title="All clear!" description="Add todos above or run: omni workspace todo add 'Task'" />
      ) : (
        <div className="space-y-1">
          {[...pending, ...done].map((t) => (
            <div
              key={t.id}
              className={`flex items-center gap-3 p-3 border rounded-sm transition-colors cursor-pointer hover:border-primary/40 ${t.done ? "border-border bg-muted/20 opacity-60" : "border-border bg-card"}`}
              onClick={() => handleToggle(t.id, t.done)}
            >
              {t.done
                ? <CheckSquare className="w-4 h-4 text-primary flex-shrink-0" />
                : <Square className="w-4 h-4 text-muted-foreground flex-shrink-0" />}
              <span className={`text-sm flex-1 ${t.done ? "line-through text-muted-foreground" : "text-foreground"}`}>
                {t.description}
              </span>
              <span className="text-[10px] text-muted-foreground font-mono">#{t.id}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function SnippetsTab() {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [lang, setLang] = useState("");
  const [body, setBody] = useState("");
  const { data: snippets, isLoading, isError, error, refetch } = useListSnippets({});
  const createSnippet = useCreateSnippet();

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !body.trim()) return;
    createSnippet.mutate({ data: { name, language: lang || undefined, body } }, {
      onSuccess: () => { setOpen(false); setName(""); setLang(""); setBody(""); refetch(); },
    });
  };

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Button size="sm" onClick={() => setOpen(!open)} className="gap-2">
          <Plus className="w-4 h-4" />Save Snippet
        </Button>
      </div>
      {open && (
        <Card className="border-primary/30 glow-border bg-card/80">
          <CardContent className="p-4">
            <form onSubmit={handleCreate} className="space-y-3">
              <div className="flex gap-3">
                <Input placeholder="Snippet name (unique)" value={name} onChange={(e) => setName(e.target.value)} required className="flex-1" />
                <Input placeholder="Language" value={lang} onChange={(e) => setLang(e.target.value)} className="w-32 font-mono" />
              </div>
              <Textarea placeholder="Snippet body…" value={body} onChange={(e) => setBody(e.target.value)} className="font-mono min-h-[120px] text-xs" required />
              <div className="flex gap-2">
                <Button type="submit" size="sm" disabled={createSnippet.isPending}>Save</Button>
                <Button type="button" size="sm" variant="secondary" onClick={() => setOpen(false)}>Cancel</Button>
              </div>
            </form>
          </CardContent>
        </Card>
      )}
      {isLoading ? (
        <div className="grid sm:grid-cols-2 gap-3">{Array.from({length:4}).map((_,i)=><SkeletonCard key={i} rows={4}/>)}</div>
      ) : isError ? (
        <ErrorState compact title="Failed to load snippets" message={(error as Error)?.message} onRetry={refetch} />
      ) : !snippets?.length ? (
        <EmptyState icon={<Code className="w-8 h-8 text-muted-foreground"/>} title="No snippets saved" description="Store reusable code with: omni workspace snippet save 'name' 'body'" />
      ) : (
        <div className="grid sm:grid-cols-2 gap-3">
          {snippets.map((s) => (
            <Card key={s.id} className="border-border bg-card hover:border-primary/30 transition-colors">
              <CardHeader className="pb-2">
                <div className="flex items-center justify-between gap-2">
                  <CardTitle className="text-sm font-mono truncate">{s.name}</CardTitle>
                  {s.language && <Badge variant="secondary" className="text-[10px] flex-shrink-0">{s.language}</Badge>}
                </div>
              </CardHeader>
              <CardContent>
                <pre className="text-[11px] bg-background border border-border p-2.5 rounded-sm overflow-x-auto text-muted-foreground max-h-[120px]">{s.body}</pre>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

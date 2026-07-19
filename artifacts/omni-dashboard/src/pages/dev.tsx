import { useState } from "react";
import { useDevHash, useDevJson, useDevBase64, useDevUuid, useDevRegex, useDevJwt } from "@workspace/api-client-react";
import { DevHashInputAlgo, DevJsonInputAction } from "@workspace/api-zod";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { Wrench, Copy, Check, RefreshCw, Shuffle } from "lucide-react";
import { ErrorState } from "@/components/ui/states";

export default function DevPage() {
  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
          <Wrench className="w-6 h-6 sm:w-8 sm:h-8" />
          Dev Toolkit
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Hash · JSON · Base64 · UUID · Regex · JWT — real computations, zero mocking.
        </p>
      </div>

      <Tabs defaultValue="hash">
        <div className="overflow-x-auto">
          <TabsList className="bg-muted/30 border border-border w-max min-w-full sm:w-auto">
            {["hash", "json", "base64", "uuid", "regex", "jwt"].map((t) => (
              <TabsTrigger key={t} value={t} className="font-mono text-xs uppercase tracking-widest">
                {t}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>

        <TabsContent value="hash"  className="mt-4"><HashTool /></TabsContent>
        <TabsContent value="json"  className="mt-4"><JsonTool /></TabsContent>
        <TabsContent value="base64" className="mt-4"><Base64Tool /></TabsContent>
        <TabsContent value="uuid"  className="mt-4"><UuidTool /></TabsContent>
        <TabsContent value="regex" className="mt-4"><RegexTool /></TabsContent>
        <TabsContent value="jwt"   className="mt-4"><JwtTool /></TabsContent>
      </Tabs>
    </div>
  );
}

// ── shared helpers ─────────────────────────────────────────────────────────────

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const doCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  return (
    <button onClick={doCopy} className="p-1.5 rounded-sm text-muted-foreground hover:text-primary transition-colors" title="Copy">
      {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
    </button>
  );
}

function OutputBox({ value, label }: { value: string; label?: string }) {
  return (
    <div>
      {label && <div className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">{label}</div>}
      <div className="relative">
        <pre className="font-mono text-xs bg-background border border-border rounded-sm p-3 overflow-x-auto text-primary break-all pr-8">
          {value}
        </pre>
        <div className="absolute top-1.5 right-1.5">
          <CopyButton text={value} />
        </div>
      </div>
    </div>
  );
}

// ── Hash ──────────────────────────────────────────────────────────────────────

function HashTool() {
  const [input, setInput] = useState("");
  const [algo, setAlgo] = useState<DevHashInputAlgo>(DevHashInputAlgo.blake3);
  const ALGOS = Object.values(DevHashInputAlgo);
  const hash = useDevHash();

  const run = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!input) return;
    hash.mutate({ data: { input, algo } });
  };

  return (
    <ToolCard title="Hash" description="Compute SHA-256, SHA-1, MD5, or BLAKE3 digest.">
      <form onSubmit={run} className="space-y-3">
        <Textarea value={input} onChange={e => setInput(e.target.value)} placeholder="Input text to hash…" className="font-mono min-h-[80px]" />
        <div className="flex gap-2 flex-wrap">
          {ALGOS.map(a => (
            <button key={a} type="button" onClick={() => setAlgo(a)}
              className={`px-3 py-1 text-xs font-mono border rounded-sm transition-colors ${algo === a ? "border-primary bg-primary/10 text-primary" : "border-border text-muted-foreground hover:border-primary/50"}`}>
              {a}
            </button>
          ))}
          <Button type="submit" size="sm" disabled={hash.isPending || !input} className="ml-auto">
            {hash.isPending ? <RefreshCw className="w-4 h-4 animate-spin" /> : "Hash"}
          </Button>
        </div>
        {hash.isError && <ErrorState compact title="Hash failed" message={(hash.error as Error)?.message} onRetry={() => run()} />}
        {hash.data && <OutputBox value={hash.data.digest} label={`${hash.data.algo} digest`} />}
      </form>
    </ToolCard>
  );
}

// ── JSON ──────────────────────────────────────────────────────────────────────

function JsonTool() {
  const [input, setInput] = useState("");
  const [action, setAction] = useState<DevJsonInputAction>(DevJsonInputAction.pretty);
  const ACTIONS = Object.values(DevJsonInputAction);
  const json = useDevJson();

  const run = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!input) return;
    json.mutate({ data: { input, action } });
  };

  return (
    <ToolCard title="JSON" description="Pretty-print, minify, or validate JSON.">
      <form onSubmit={run} className="space-y-3">
        <Textarea value={input} onChange={e => setInput(e.target.value)} placeholder='{"key": "value"}' className="font-mono min-h-[120px] text-xs" />
        <div className="flex gap-2 flex-wrap">
          {ACTIONS.map(a => (
            <button key={a} type="button" onClick={() => setAction(a)}
              className={`px-3 py-1 text-xs font-mono border rounded-sm transition-colors ${action === a ? "border-primary bg-primary/10 text-primary" : "border-border text-muted-foreground hover:border-primary/50"}`}>
              {a}
            </button>
          ))}
          <Button type="submit" size="sm" disabled={json.isPending || !input} className="ml-auto">
            {json.isPending ? <RefreshCw className="w-4 h-4 animate-spin" /> : "Run"}
          </Button>
        </div>
        {json.isError && <ErrorState compact title="JSON failed" message={(json.error as Error)?.message} />}
        {json.data && (
          json.data.valid
            ? <OutputBox value={json.data.output} label="Output" />
            : <p className="text-xs text-destructive font-mono">{json.data.error}</p>
        )}
      </form>
    </ToolCard>
  );
}

// ── Base64 ────────────────────────────────────────────────────────────────────

function Base64Tool() {
  const [input, setInput] = useState("");
  const [decode, setDecode] = useState(false);
  const b64 = useDevBase64();

  const run = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!input) return;
    b64.mutate({ data: { input, decode } });
  };

  return (
    <ToolCard title="Base64" description="Encode or decode Base64.">
      <form onSubmit={run} className="space-y-3">
        <Textarea value={input} onChange={e => setInput(e.target.value)} placeholder="Input text…" className="font-mono min-h-[80px]" />
        <div className="flex gap-3">
          <label className="flex items-center gap-2 text-sm cursor-pointer select-none">
            <input type="checkbox" checked={decode} onChange={e => setDecode(e.target.checked)} className="accent-primary" />
            Decode
          </label>
          <Button type="submit" size="sm" disabled={b64.isPending || !input} className="ml-auto">
            {b64.isPending ? <RefreshCw className="w-4 h-4 animate-spin" /> : decode ? "Decode" : "Encode"}
          </Button>
        </div>
        {b64.isError && <ErrorState compact title="Base64 failed" message={(b64.error as Error)?.message} />}
        {b64.data && <OutputBox value={b64.data.output} label={b64.data.decoded ? "Decoded" : "Encoded"} />}
      </form>
    </ToolCard>
  );
}

// ── UUID ──────────────────────────────────────────────────────────────────────

function UuidTool() {
  const [count, setCount] = useState(1);
  // useDevUuid is a query hook — re-fetches when params change
  const uuid = useDevUuid({ count });

  return (
    <ToolCard title="UUID Generator" description="Generate v4 (time-seeded random) UUIDs.">
      <div className="space-y-3">
        <div className="flex gap-3 items-center flex-wrap">
          <div className="flex items-center gap-2">
            <label className="text-xs text-muted-foreground">Count:</label>
            <Input
              type="number" min={1} max={100} value={count}
              onChange={e => setCount(Math.min(100, Math.max(1, Number(e.target.value))))}
              className="w-20 font-mono"
            />
          </div>
          <Button
            size="sm"
            onClick={() => uuid.refetch()}
            disabled={uuid.isFetching}
            className="ml-auto gap-2"
          >
            <Shuffle className={`w-3.5 h-3.5 ${uuid.isFetching ? "animate-spin" : ""}`} />
            Generate
          </Button>
        </div>
        {uuid.isError && <ErrorState compact title="UUID failed" message={(uuid.error as Error)?.message} />}
        {uuid.data && (
          <div className="relative">
            <pre className="font-mono text-xs bg-background border border-border rounded-sm p-3 overflow-x-auto text-primary pr-8">
              {uuid.data.uuids.join("\n")}
            </pre>
            <div className="absolute top-1.5 right-1.5">
              <CopyButton text={uuid.data.uuids.join("\n")} />
            </div>
          </div>
        )}
      </div>
    </ToolCard>
  );
}

// ── Regex ─────────────────────────────────────────────────────────────────────

function RegexTool() {
  const [pattern, setPattern] = useState("");
  const [text, setText] = useState("");
  const [flags, setFlags] = useState("i");
  const regex = useDevRegex();

  const run = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!pattern || !text) return;
    regex.mutate({ data: { pattern, text, flags } });
  };

  return (
    <ToolCard title="Regex Tester" description="Test a regex pattern against input text. Supports flags: i (case), m (multiline), s (dot-all).">
      <form onSubmit={run} className="space-y-3">
        <div className="flex gap-2">
          <Input value={pattern} onChange={e => setPattern(e.target.value)} placeholder="Pattern (e.g. \d+)" className="font-mono flex-1" />
          <Input value={flags} onChange={e => setFlags(e.target.value)} placeholder="flags" className="font-mono w-24" />
        </div>
        <Textarea value={text} onChange={e => setText(e.target.value)} placeholder="Test text…" className="font-mono min-h-[80px]" />
        <Button type="submit" size="sm" disabled={regex.isPending || !pattern} className="w-full">
          Test
        </Button>
        {regex.isError && <ErrorState compact title="Regex error" message={(regex.error as Error)?.message} />}
        {regex.data && (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <Badge variant={regex.data.matched ? "default" : "destructive"} className="text-xs">
                {regex.data.matched ? `${regex.data.matchCount} match(es)` : "No match"}
              </Badge>
              {regex.data.error && <span className="text-xs text-destructive font-mono">{regex.data.error}</span>}
            </div>
            {regex.data.matches.map((m, i) => (
              <div key={i} className="flex gap-3 text-xs font-mono border-l-2 border-primary/40 pl-3">
                <span className="text-muted-foreground">[{m.start}–{m.end}]</span>
                <span className="text-yellow-400">{m.match}</span>
                {(m.groups?.length ?? 0) > 0 && <span className="text-muted-foreground">groups: {JSON.stringify(m.groups)}</span>}
              </div>
            ))}
          </div>
        )}
      </form>
    </ToolCard>
  );
}

// ── JWT ───────────────────────────────────────────────────────────────────────

function JwtTool() {
  const [token, setToken] = useState("");
  const jwt = useDevJwt();

  const run = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!token) return;
    jwt.mutate({ data: { token } });
  };

  return (
    <ToolCard title="JWT Decoder" description="Decode and inspect JWT tokens without signature verification.">
      <form onSubmit={run} className="space-y-3">
        <Textarea value={token} onChange={e => setToken(e.target.value)} placeholder="eyJ…" className="font-mono min-h-[80px] text-xs break-all" />
        <Button type="submit" size="sm" disabled={jwt.isPending || !token} className="w-full">
          Decode
        </Button>
        {jwt.isError && <ErrorState compact title="JWT decode failed" message={(jwt.error as Error)?.message} />}
        {jwt.data && jwt.data.valid && (
          <div className="space-y-3">
            {jwt.data.isExpired !== null && (
              <div className={`p-2 rounded-sm text-xs font-mono border ${jwt.data.isExpired ? "border-destructive/40 bg-destructive/10 text-destructive" : "border-emerald-500/30 bg-emerald-500/10 text-emerald-400"}`}>
                {jwt.data.isExpired ? `⚠ EXPIRED — ${jwt.data.expiresAt}` : `✓ Valid until ${jwt.data.expiresAt}`}
              </div>
            )}
            <div className="grid sm:grid-cols-2 gap-3">
              <OutputBox value={JSON.stringify(jwt.data.header, null, 2)} label="Header" />
              <OutputBox value={JSON.stringify(jwt.data.payload, null, 2)} label="Payload" />
            </div>
          </div>
        )}
      </form>
    </ToolCard>
  );
}

function ToolCard({ title, description, children }: { title: string; description: string; children: React.ReactNode }) {
  return (
    <Card className="border-border bg-card/80">
      <CardHeader className="pb-3">
        <CardTitle className="text-base">{title}</CardTitle>
        <p className="text-xs text-muted-foreground">{description}</p>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  );
}

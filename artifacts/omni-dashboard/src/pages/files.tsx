import { useState } from "react";
import { useFindFiles } from "@workspace/api-client-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { File, Folder, HardDrive, Clock, FolderSearch } from "lucide-react";
import { formatBytes, formatEpochOrDate } from "@/lib/utils";
import { LoadingSpinner, ErrorState, EmptyState, SkeletonBlock } from "@/components/ui/states";

const TYPE_FILTERS = ["any", "file", "dir"] as const;

export default function FilesPage() {
  const [pattern, setPattern] = useState("");
  const [typeFilter, setTypeFilter] = useState<string>("any");
  const [queryParams, setQueryParams] = useState<{ pattern?: string; type?: string } | null>(null);

  const { data, isLoading, isError, error, refetch } = useFindFiles(
    queryParams
      ? {
          pattern: queryParams.pattern || undefined,
          type: queryParams.type === "any" ? undefined : (queryParams.type === "file" ? "f" : queryParams.type === "dir" ? "d" : undefined),
        }
      : undefined,
    { query: { enabled: queryParams !== null } as never }
  );

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setQueryParams({ pattern: pattern.trim() || undefined, type: typeFilter });
  };

  const files = data?.entries ?? [];

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-5 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
          <FolderSearch className="w-6 h-6 sm:w-8 sm:h-8" />
          File Finder
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">Real-time filesystem walk — no stale index.</p>
      </div>

      <Card className="border-primary/30 glow-border bg-card/80">
        <CardContent className="p-4 sm:p-6">
          <form onSubmit={handleSearch} className="flex flex-col sm:flex-row gap-3">
            <Input
              placeholder="Name pattern — e.g. *.rs  or  config"
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
              className="font-mono h-10 sm:h-11 flex-1"
            />
            <div className="flex gap-2">
              {TYPE_FILTERS.map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setTypeFilter(t)}
                  className={`px-3 h-10 sm:h-11 text-xs font-mono uppercase border rounded-sm transition-colors ${
                    typeFilter === t
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:border-primary/50"
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
            <Button type="submit" className="h-10 sm:h-11 font-bold tracking-wider">FIND</Button>
          </form>
        </CardContent>
      </Card>

      {!isLoading && !isError && files.length > 0 && (
        <div className="flex items-center justify-between text-xs text-muted-foreground font-mono px-1">
          <span><span className="text-primary font-bold">{files.length}</span> entries</span>
          {data?.durationMs != null && (
            <span className="flex items-center gap-1.5">
              <HardDrive className="w-3 h-3" />in {data.durationMs}ms
            </span>
          )}
        </div>
      )}

      {isLoading && <LoadingSpinner label="Walking filesystem…" />}

      {isError && (
        <ErrorState title="File listing failed" message={(error as Error)?.message} onRetry={refetch} />
      )}

      {!isLoading && !isError && queryParams !== null && files.length === 0 && (
        <EmptyState
          icon={<FolderSearch className="w-8 h-8 text-muted-foreground" />}
          title="No files found"
          description="Try a different pattern or remove the type filter."
        />
      )}

      {queryParams === null && (
        <EmptyState
          icon={<FolderSearch className="w-8 h-8 text-muted-foreground" />}
          title="Enter a pattern and press FIND"
          description="Searches the filesystem in real time — no index required."
        />
      )}

      {files.length > 0 && (
        <div className="border border-border rounded-sm overflow-hidden">
          <div className="hidden sm:grid grid-cols-[auto_1fr_auto_auto] gap-4 px-4 py-2.5 bg-muted/30 text-xs uppercase tracking-wider text-muted-foreground font-semibold">
            <span>Type</span><span>Path</span>
            <span className="text-right">Size</span>
            <span className="text-right">Modified</span>
          </div>
          <div className="divide-y divide-border">
            {files.map((f, i) => (
              <div key={i} className="flex sm:grid sm:grid-cols-[auto_1fr_auto_auto] items-start sm:items-center gap-2 sm:gap-4 px-3 sm:px-4 py-2.5 hover:bg-secondary/30 transition-colors flex-wrap">
                <span className="flex-shrink-0">
                  {f.fileType === "dir"
                    ? <Folder className="w-4 h-4 text-yellow-400" />
                    : <File className="w-4 h-4 text-muted-foreground" />}
                </span>
                <span className="font-mono text-xs sm:text-sm text-foreground break-all flex-1 min-w-0 sm:truncate">{f.path}</span>
                <span className="text-xs text-muted-foreground font-mono sm:text-right">{formatBytes(f.sizeBytes ?? 0)}</span>
                <span className="text-xs text-muted-foreground font-mono sm:text-right">{formatEpochOrDate(f.modified)}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

import { useState } from "react";
import { useSearchFiles, useTriggerSearchIndex, useGetSearchIndexInfo } from "@workspace/api-client-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Search as SearchIcon, Database, RefreshCw, FileText, Clock, HardDrive } from "lucide-react";
import { formatBytes, formatEpochOrDate, formatMs } from "@/lib/utils";
import { LoadingSpinner, ErrorState, SearchEmptyState, EmptyState } from "@/components/ui/states";

export default function SearchPage() {
  const [query, setQuery] = useState("");
  const [types, setTypes] = useState("");
  const [searchParams, setSearchParams] = useState({ q: "", types: "" });

  const { data: indexInfo, refetch: refetchIndex, isLoading: indexLoading } = useGetSearchIndexInfo();
  const triggerIndex = useTriggerSearchIndex();

  const { data: results, isLoading, isError, error, refetch } = useSearchFiles(
    searchParams,
    { query: { enabled: searchParams.q.length > 0 } as never }
  );

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;
    setSearchParams({ q: query.trim(), types });
  };

  const handleRebuild = () => {
    triggerIndex.mutate({ data: { paths: ["/home"], rebuild: true } }, {
      onSuccess: () => refetchIndex(),
    });
  };

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-5 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div className="flex flex-col sm:flex-row sm:justify-between sm:items-start gap-4">
        <div>
          <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
            <SearchIcon className="w-6 h-6 sm:w-8 sm:h-8" />
            Global Search
          </h1>
          <p className="text-muted-foreground mt-1 text-sm sm:text-base">
            Full-text query across indexed files and documents.
          </p>
        </div>

        {!indexLoading && indexInfo && (
          <div className="flex items-center gap-3 bg-muted/30 p-3 border border-border rounded-sm self-start flex-wrap">
            <div className="flex items-center gap-2">
              <Database className="w-4 h-4 text-primary flex-shrink-0" />
              <div>
                <div className="text-xs text-muted-foreground uppercase tracking-wider">Index</div>
                <div className="font-mono text-sm">
                  <span className="text-primary">{indexInfo.totalEntries.toLocaleString()}</span>
                  <span className="text-muted-foreground text-xs ml-1">entries</span>
                </div>
              </div>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={handleRebuild}
              disabled={triggerIndex.isPending}
              className="text-xs"
            >
              <RefreshCw className={`w-3 h-3 mr-1.5 ${triggerIndex.isPending ? "animate-spin" : ""}`} />
              {triggerIndex.isPending ? "Indexing…" : "Rebuild"}
            </Button>
          </div>
        )}
      </div>

      <Card className="border-primary/30 glow-border bg-card/80">
        <CardContent className="p-4 sm:p-6">
          <form onSubmit={handleSearch} className="flex flex-col sm:flex-row gap-3">
            <div className="flex-1">
              <Input
                placeholder='Query — e.g. "error AND timeout" or CVE-2026'
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="font-mono h-11 sm:h-12 text-sm sm:text-base"
                autoFocus
              />
            </div>
            <Input
              placeholder="Types: code,pdf,logs"
              value={types}
              onChange={(e) => setTypes(e.target.value)}
              className="font-mono h-11 sm:h-12 sm:w-48"
            />
            <Button type="submit" size="lg" className="h-11 sm:h-12 font-bold tracking-wider">
              EXECUTE
            </Button>
          </form>
        </CardContent>
      </Card>

      {isLoading && <LoadingSpinner label="Searching index…" />}

      {isError && (
        <ErrorState
          title="Search failed"
          message={(error as Error)?.message}
          onRetry={refetch}
        />
      )}

      {!isLoading && !isError && searchParams.q && results?.results?.length === 0 && (
        <SearchEmptyState query={searchParams.q} />
      )}

      {results && results.results.length > 0 && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <p className="text-sm text-muted-foreground font-mono">
              <span className="text-primary font-bold">{results.total}</span> result(s) for{" "}
              <span className="text-foreground">"{searchParams.q}"</span>{" "}
              in <span className="text-primary">{formatMs(results.durationMs)}</span>
            </p>
          </div>
          <div className="space-y-2">
            {results.results.map((r, i) => (
              <Card key={i} className="border-border bg-card/70 hover:border-primary/40 transition-colors">
                <CardContent className="p-3 sm:p-4">
                  <div className="flex items-start justify-between gap-3 flex-wrap">
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-mono text-primary truncate">{r.path}</p>
                      {r.snippet && (
                        <p className="text-xs text-muted-foreground mt-1.5 line-clamp-2 font-mono">
                          {r.snippet}
                        </p>
                      )}
                    </div>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      {r.fileType && <Badge variant="secondary" className="text-[10px] uppercase">{r.fileType}</Badge>}
                      {r.rank != null && (
                        <span className="text-[10px] text-muted-foreground font-mono">score: {r.rank.toFixed(1)}</span>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-4 mt-2 text-[11px] text-muted-foreground font-mono flex-wrap">
                    {r.sizeBytes != null && (
                      <span className="flex items-center gap-1">
                        <HardDrive className="w-3 h-3" />{formatBytes(r.sizeBytes)}
                      </span>
                    )}
                    {r.modified != null && (
                      <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />{formatEpochOrDate(r.modified)}
                      </span>
                    )}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      )}

      {!searchParams.q && (
        <EmptyState
          icon={<SearchIcon className="w-8 h-8 text-muted-foreground" />}
          title="Ready to search"
          description='Enter a query above and press EXECUTE. Supports AND, OR, NOT, and "phrase" search.'
        />
      )}
    </div>
  );
}

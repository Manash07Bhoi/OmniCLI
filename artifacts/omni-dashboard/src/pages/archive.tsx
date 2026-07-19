import { useState } from "react";
import { useInspectArchive } from "@workspace/api-client-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Archive, File, Folder, HardDrive, Layers } from "lucide-react";
import { formatBytes, formatEpochOrDate } from "@/lib/utils";
import { LoadingSpinner, ErrorState, EmptyState } from "@/components/ui/states";

export default function ArchivePage() {
  const [archivePath, setArchivePath] = useState("");
  const [queriedPath, setQueriedPath] = useState("");

  const { data: archive, isLoading, isError, error, refetch } = useInspectArchive(
    queriedPath,
    { query: { enabled: queriedPath.length > 0 } }
  );

  const handleInspect = (e: React.FormEvent) => {
    e.preventDefault();
    if (!archivePath.trim()) return;
    setQueriedPath(archivePath.trim());
  };

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-5 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
          <Archive className="w-6 h-6 sm:w-8 sm:h-8" />
          Archive Inspector
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Inspect ZIP, TAR, TAR.GZ, TAR.XZ, TAR.BZ2 archives without extracting.
        </p>
      </div>

      {/* Path form */}
      <Card className="border-primary/30 glow-border bg-card/80">
        <CardContent className="p-4 sm:p-6">
          <form onSubmit={handleInspect} className="flex gap-3">
            <Input
              value={archivePath}
              onChange={(e) => setArchivePath(e.target.value)}
              placeholder="/path/to/archive.tar.gz"
              className="font-mono h-11 flex-1"
              autoFocus
            />
            <Button type="submit" className="h-11 font-bold tracking-wider" disabled={isLoading || !archivePath.trim()}>
              INSPECT
            </Button>
          </form>
        </CardContent>
      </Card>

      {/* Results */}
      {isLoading && <LoadingSpinner label="Reading archive…" />}

      {isError && (
        <ErrorState
          title="Archive inspection failed"
          message={(error as Error)?.message}
          onRetry={refetch}
        />
      )}

      {!queriedPath && (
        <EmptyState
          icon={<Archive className="w-8 h-8 text-muted-foreground" />}
          title="Enter an archive path"
          description="Supports ZIP, TAR, TAR.GZ (.tgz), TAR.XZ, and TAR.BZ2 — format detected from magic bytes."
        />
      )}

      {archive && (
        <div className="space-y-5">
          {/* Summary */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <SummaryTile
              label="Format"
              value={(archive.format ?? "Unknown").toUpperCase()}
              icon={<Archive className="w-4 h-4 text-primary" />}
            />
            <SummaryTile
              label="Total Entries"
              value={(archive.entries?.length ?? 0).toLocaleString()}
              icon={<Layers className="w-4 h-4 text-primary" />}
            />
            <SummaryTile
              label="Uncompressed"
              value={formatBytes(archive.totalSizeBytes ?? 0)}
              icon={<HardDrive className="w-4 h-4 text-primary" />}
            />
            <SummaryTile
              label="Compressed"
              value={archive.totalCompressedBytes ? formatBytes(archive.totalCompressedBytes) : "N/A"}
              icon={<Layers className="w-4 h-4 text-muted-foreground" />}
            />
          </div>

          {/* Entry table */}
          <Card className="border-border overflow-hidden">
            <CardHeader className="pb-2">
              <CardTitle className="text-base font-mono">{queriedPath}</CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead className="w-8 pl-4"></TableHead>
                      <TableHead>Name</TableHead>
                      <TableHead className="text-right w-24">Size</TableHead>
                      <TableHead className="text-right w-24 hidden sm:table-cell">Compressed</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {(archive.entries ?? []).length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={4} className="text-center text-muted-foreground text-sm py-8">
                          Archive is empty
                        </TableCell>
                      </TableRow>
                    ) : (
                      (archive.entries ?? []).map((entry, i) => (
                        <TableRow key={i} className="border-border hover:bg-secondary/20">
                          <TableCell className="pl-4">
                            {entry.isDir
                              ? <Folder className="w-4 h-4 text-yellow-400" />
                              : <File className="w-4 h-4 text-muted-foreground" />}
                          </TableCell>
                          <TableCell className="font-mono text-xs sm:text-sm truncate max-w-[200px] sm:max-w-none">
                            {entry.name}
                          </TableCell>
                          <TableCell className="text-right text-muted-foreground text-xs font-mono">
                            {entry.isDir ? "—" : formatBytes(entry.sizeBytes)}
                          </TableCell>
                          <TableCell className="text-right text-muted-foreground text-xs font-mono hidden sm:table-cell">
                            {entry.isDir || !entry.compressedSizeBytes ? "—" : formatBytes(entry.compressedSizeBytes)}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}

function SummaryTile({ label, value, icon }: { label: string; value: string; icon: React.ReactNode }) {
  return (
    <div className="p-3 sm:p-4 border border-border rounded-sm bg-background hover:border-primary/40 transition-colors">
      <div className="flex items-center gap-2 mb-2">{icon}</div>
      <div className="text-lg sm:text-xl font-bold font-mono text-foreground">{value}</div>
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground mt-0.5">{label}</div>
    </div>
  );
}

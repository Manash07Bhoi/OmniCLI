import { useState } from "react";
import { useListBackupJobs, useListBackupSnapshots, useCreateBackupJob } from "@workspace/api-client-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { DatabaseBackup, Plus, CheckCircle2, Clock, RefreshCw } from "lucide-react";
import { formatBytes, formatRelative } from "@/lib/utils";
import { SkeletonCard, ErrorState, EmptyState } from "@/components/ui/states";

export default function BackupPage() {
  const [newJobOpen, setNewJobOpen] = useState(false);
  const [jobName, setJobName] = useState("");
  const [sourcePath, setSourcePath] = useState("");
  const [destPath, setDestPath] = useState("");

  const { data: jobs, isLoading, isError, error, refetch } = useListBackupJobs();
  const createJob = useCreateBackupJob();

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!jobName || !sourcePath || !destPath) return;
    createJob.mutate(
      { data: { name: jobName, sourcePath, destPath } },
      {
        onSuccess: () => {
          setNewJobOpen(false);
          setJobName(""); setSourcePath(""); setDestPath("");
          refetch();
        },
      }
    );
  };

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
            <DatabaseBackup className="w-6 h-6 sm:w-8 sm:h-8" />
            Backup Ops
          </h1>
          <p className="text-muted-foreground mt-1 text-sm">
            BLAKE3-deduplicated incremental backups — content-addressed object store.
          </p>
        </div>
        <Button onClick={() => setNewJobOpen(!newJobOpen)} className="gap-2 self-start" variant={newJobOpen ? "secondary" : "default"}>
          <Plus className="w-4 h-4" />New Job
        </Button>
      </div>

      {newJobOpen && (
        <Card className="border-primary/30 glow-border bg-card/80">
          <CardHeader><CardTitle className="text-base">Create Backup Job</CardTitle></CardHeader>
          <CardContent>
            <form onSubmit={handleCreate} className="grid sm:grid-cols-3 gap-3">
              <Input placeholder="Job name" value={jobName} onChange={(e) => setJobName(e.target.value)} className="font-mono" required />
              <Input placeholder="Source path" value={sourcePath} onChange={(e) => setSourcePath(e.target.value)} className="font-mono" required />
              <Input placeholder="Destination path" value={destPath} onChange={(e) => setDestPath(e.target.value)} className="font-mono" required />
              <div className="sm:col-span-3 flex gap-3">
                <Button type="submit" disabled={createJob.isPending} className="gap-2">
                  {createJob.isPending ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                  Create
                </Button>
                <Button type="button" variant="secondary" onClick={() => setNewJobOpen(false)}>Cancel</Button>
              </div>
            </form>
          </CardContent>
        </Card>
      )}

      {isLoading ? (
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {Array.from({ length: 3 }).map((_, i) => <SkeletonCard key={i} rows={4} />)}
        </div>
      ) : isError ? (
        <ErrorState title="Failed to load backup jobs" message={(error as Error)?.message} onRetry={refetch} />
      ) : !jobs?.length ? (
        <EmptyState
          icon={<DatabaseBackup className="w-8 h-8 text-muted-foreground" />}
          title="No backup jobs yet"
          description="Create your first job to start protecting your data with BLAKE3-deduplicated snapshots."
          action={<Button onClick={() => setNewJobOpen(true)} size="sm" className="gap-2"><Plus className="w-4 h-4" />Create Job</Button>}
        />
      ) : (
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {jobs.map((job) => <BackupJobCard key={job.id} job={job} />)}
        </div>
      )}
    </div>
  );
}

type Job = { id: number; name: string; sourcePath: string; destPath: string; snapshotCount: number; lastSnapshotAt?: string | null; lastStatus?: string | null };

function BackupJobCard({ job }: { job: Job }) {
  const { data: snaps, isLoading } = useListBackupSnapshots(job.id);

  const statusColor: Record<string, string> = {
    active: "text-emerald-400", running: "text-yellow-400",
    failed: "text-red-400", idle: "text-muted-foreground",
  };

  return (
    <Card className="border-border bg-card hover:border-primary/40 transition-colors">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="text-base font-mono truncate">{job.name}</CardTitle>
          <Badge variant="outline" className={`text-[10px] uppercase flex-shrink-0 ${statusColor[job.lastStatus ?? ""] ?? "text-muted-foreground"}`}>
            {job.lastStatus ?? "idle"}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 text-xs text-muted-foreground">
        <div>
          <div className="text-[10px] uppercase tracking-wider mb-0.5">Source</div>
          <div className="font-mono text-foreground truncate">{job.sourcePath}</div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-wider mb-0.5">Destination</div>
          <div className="font-mono text-foreground truncate">{job.destPath}</div>
        </div>
        <div className="flex items-center justify-between pt-1 border-t border-border">
          <div className="flex items-center gap-1.5">
            <CheckCircle2 className="w-3 h-3 text-primary" />
            {isLoading ? "…" : `${snaps?.length ?? 0} snapshots`}
          </div>
          {job.lastSnapshotAt && (
            <div className="flex items-center gap-1.5">
              <Clock className="w-3 h-3" />{formatRelative(job.lastSnapshotAt)}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

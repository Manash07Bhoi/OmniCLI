import { useGetDashboardStats, useGetDashboardActivity, useGetModuleStatus } from "@workspace/api-client-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Activity, HardDrive, Search, Archive, FileText, CheckSquare, Code, Layers, RefreshCw } from "lucide-react";
import { formatBytes } from "@/lib/utils";
import { formatDistanceToNow } from "date-fns";
import { SkeletonBlock, SkeletonCard, ErrorState, EmptyState } from "@/components/ui/states";

export default function Dashboard() {
  const { data: stats, isLoading: statsLoading, isError: statsError, error: statsErr, refetch: refetchStats } = useGetDashboardStats();
  const { data: activity, isLoading: actLoading, isError: actError, refetch: refetchAct } = useGetDashboardActivity({ limit: 10 });
  const { data: modules, isLoading: modLoading, isError: modError } = useGetModuleStatus();

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 lg:space-y-8 max-w-[1600px] mx-auto w-full animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
            <Activity className="w-6 h-6 sm:w-8 sm:h-8" />
            Command Center
          </h1>
          <p className="text-muted-foreground mt-1 sm:mt-2 text-sm sm:text-base">
            System overview and real-time operations telemetry.
          </p>
        </div>
        <button
          onClick={() => { refetchStats(); refetchAct(); }}
          className="p-2 rounded-sm text-muted-foreground hover:text-primary hover:bg-secondary transition-colors flex-shrink-0"
          title="Refresh"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
      </div>

      {/* Stats grid */}
      {statsError ? (
        <ErrorState
          compact
          title="Could not load stats"
          message={(statsErr as Error)?.message}
          onRetry={refetchStats}
        />
      ) : statsLoading ? (
        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-8 gap-3 sm:gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="h-24 rounded-sm bg-muted/40 animate-pulse border border-border" />
          ))}
        </div>
      ) : stats ? (
        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-8 gap-3 sm:gap-4">
          <StatTile title="Total Files" value={stats.totalFiles.toLocaleString()} icon={HardDrive} />
          <StatTile title="Storage"     value={formatBytes(stats.totalSizeBytes)}  icon={Layers} />
          <StatTile title="Indexed"     value={stats.indexedFiles.toLocaleString()} icon={Search} />
          <StatTile title="Backups"     value={stats.backupJobs}                   icon={Archive} />
          <StatTile title="Notes"       value={stats.notes}                        icon={FileText} />
          <StatTile title="Todos"       value={stats.todos}                        icon={CheckSquare} />
          <StatTile title="Snippets"    value={stats.snippets}                     icon={Code} />
          <StatTile title="Modules"     value={`${stats.modulesActive}`}           icon={Activity} />
        </div>
      ) : null}

      <div className="grid lg:grid-cols-3 gap-6 lg:gap-8">
        {/* Activity feed */}
        <div className="lg:col-span-2 space-y-4 sm:space-y-6">
          <Card className="border-primary/20 glow-border bg-card/50 backdrop-blur">
            <CardHeader>
              <CardTitle>Recent Telemetry</CardTitle>
              <CardDescription>Latest system operations and activity.</CardDescription>
            </CardHeader>
            <CardContent>
              {actLoading ? (
                <div className="space-y-4">
                  {Array.from({ length: 5 }).map((_, i) => (
                    <div key={i} className="flex gap-3">
                      <div className="w-3 h-3 mt-1.5 rounded-full bg-muted animate-pulse flex-shrink-0" />
                      <div className="flex-1 space-y-2">
                        <SkeletonBlock className="h-3 w-24" />
                        <SkeletonBlock className="h-3 w-full" />
                      </div>
                    </div>
                  ))}
                </div>
              ) : actError ? (
                <ErrorState compact title="Activity feed unavailable" onRetry={refetchAct} />
              ) : activity?.length ? (
                <div className="space-y-4 relative before:absolute before:inset-y-0 before:left-3 before:w-px before:bg-border">
                  {activity.map((item) => (
                    <div key={item.id} className="relative pl-8 sm:pl-10 flex flex-col gap-1 group">
                      <div className="absolute left-1.5 top-1.5 w-3 h-3 rounded-full bg-primary/20 border border-primary group-hover:bg-primary transition-colors" />
                      <div className="flex items-baseline justify-between gap-2 flex-wrap">
                        <span className="font-semibold text-xs sm:text-sm text-card-foreground">
                          {item.type.toUpperCase()}
                        </span>
                        <span className="text-xs text-muted-foreground font-mono">
                          {formatDistanceToNow(new Date(item.timestamp), { addSuffix: true })}
                        </span>
                      </div>
                      <p className="text-xs sm:text-sm text-muted-foreground">{item.description}</p>
                      {item.metadata && (
                        <pre className="text-[10px] mt-1 p-2 bg-background border border-border text-muted-foreground overflow-x-auto rounded-sm">
                          {item.metadata}
                        </pre>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <EmptyState
                  title="No recent activity"
                  description="Operations you run will appear here as telemetry events."
                />
              )}
            </CardContent>
          </Card>
        </div>

        {/* Module status */}
        <div className="space-y-4 sm:space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Module Subsystems</CardTitle>
            </CardHeader>
            <CardContent>
              {modLoading ? (
                <div className="space-y-3">
                  {Array.from({ length: 4 }).map((_, i) => (
                    <SkeletonCard key={i} rows={2} />
                  ))}
                </div>
              ) : modError ? (
                <ErrorState compact title="Module status unavailable" />
              ) : modules ? (
                <div className="space-y-2.5 max-h-[500px] overflow-y-auto pr-1">
                  {modules.map((mod) => (
                    <div
                      key={mod.name}
                      className="flex flex-col p-3 border border-border bg-background rounded-sm hover:border-primary/30 transition-colors"
                      style={{ borderLeftColor: mod.color, borderLeftWidth: 2 }}
                    >
                      <div className="flex justify-between items-center mb-1.5">
                        <span className="font-bold text-xs sm:text-sm font-mono" style={{ color: mod.color }}>
                          {mod.name}
                        </span>
                        <Badge
                          variant={mod.status === "active" ? "default" : "secondary"}
                          className="text-[10px] uppercase h-5"
                        >
                          {mod.status === "active" ? "ACTIVE" : `PH:${mod.phase}`}
                        </Badge>
                      </div>
                      <span className="text-[11px] text-muted-foreground leading-snug">{mod.description}</span>
                    </div>
                  ))}
                </div>
              ) : null}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

function StatTile({ title, value, icon: Icon }: { title: string; value: string | number; icon: React.ComponentType<{ className?: string }> }) {
  return (
    <div className="p-3 sm:p-4 bg-background border border-border rounded-sm flex flex-col justify-between hover:border-primary/50 transition-colors group min-h-[80px] sm:min-h-[96px]">
      <div className="flex justify-between items-start mb-2 sm:mb-4">
        <Icon className="w-4 h-4 sm:w-5 sm:h-5 text-muted-foreground group-hover:text-primary transition-colors" />
      </div>
      <div>
        <div className="text-lg sm:text-2xl font-bold text-foreground group-hover:text-primary transition-colors leading-none">
          {value}
        </div>
        <div className="text-[10px] sm:text-xs text-muted-foreground mt-1 uppercase tracking-wider">{title}</div>
      </div>
    </div>
  );
}

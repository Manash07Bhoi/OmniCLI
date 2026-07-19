import { AlertCircle, RefreshCw, Inbox, Search, ServerOff } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

// ── Loading skeleton ──────────────────────────────────────────────────────────

export function SkeletonBlock({ className }: { className?: string }) {
  return (
    <div className={cn("animate-pulse rounded-sm bg-muted/60", className)} />
  );
}

export function SkeletonCard({ rows = 3 }: { rows?: number }) {
  return (
    <div className="border border-border rounded-sm p-4 space-y-3 bg-card/50">
      <SkeletonBlock className="h-4 w-1/3" />
      {Array.from({ length: rows }).map((_, i) => (
        <SkeletonBlock key={i} className={cn("h-3", i % 3 === 0 ? "w-full" : i % 3 === 1 ? "w-3/4" : "w-2/3")} />
      ))}
    </div>
  );
}

export function SkeletonTable({ rows = 5, cols = 4 }: { rows?: number; cols?: number }) {
  return (
    <div className="border border-border rounded-sm overflow-hidden">
      <div className="grid bg-muted/30 p-3 gap-4" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
        {Array.from({ length: cols }).map((_, i) => (
          <SkeletonBlock key={i} className="h-3 w-16" />
        ))}
      </div>
      {Array.from({ length: rows }).map((_, r) => (
        <div key={r} className="grid border-t border-border p-3 gap-4" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
          {Array.from({ length: cols }).map((_, c) => (
            <SkeletonBlock key={c} className={cn("h-3", c === 0 ? "w-full" : "w-2/3")} />
          ))}
        </div>
      ))}
    </div>
  );
}

export function LoadingSpinner({ size = "md", label }: { size?: "sm" | "md" | "lg"; label?: string }) {
  const sizes = { sm: "w-4 h-4", md: "w-8 h-8", lg: "w-12 h-12" };
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-12">
      <RefreshCw className={cn("animate-spin text-primary", sizes[size])} />
      {label && <p className="text-sm text-muted-foreground font-mono">{label}</p>}
    </div>
  );
}

// ── Error state ───────────────────────────────────────────────────────────────

interface ErrorStateProps {
  title?: string;
  message?: string;
  onRetry?: () => void;
  compact?: boolean;
}

export function ErrorState({ title = "Something went wrong", message, onRetry, compact }: ErrorStateProps) {
  if (compact) {
    return (
      <div className="flex items-center gap-3 p-4 border border-destructive/40 bg-destructive/10 rounded-sm">
        <AlertCircle className="w-4 h-4 text-destructive flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-destructive">{title}</p>
          {message && <p className="text-xs text-muted-foreground mt-0.5 truncate">{message}</p>}
        </div>
        {onRetry && (
          <Button variant="outline" size="sm" onClick={onRetry} className="flex-shrink-0 text-xs border-destructive/40 hover:border-destructive">
            Retry
          </Button>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center justify-center py-16 gap-5 text-center px-6">
      <div className="p-4 rounded-full bg-destructive/10 border border-destructive/30">
        <ServerOff className="w-8 h-8 text-destructive" />
      </div>
      <div>
        <h3 className="font-bold text-lg text-foreground">{title}</h3>
        {message && (
          <p className="text-sm text-muted-foreground mt-1 font-mono max-w-sm">{message}</p>
        )}
      </div>
      {onRetry && (
        <Button variant="outline" onClick={onRetry} className="gap-2">
          <RefreshCw className="w-4 h-4" />
          Retry
        </Button>
      )}
    </div>
  );
}

// ── Empty state ───────────────────────────────────────────────────────────────

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 gap-4 text-center px-6">
      <div className="p-4 rounded-full bg-muted/50 border border-border">
        {icon ?? <Inbox className="w-8 h-8 text-muted-foreground" />}
      </div>
      <div>
        <h3 className="font-semibold text-base text-foreground font-mono">{title}</h3>
        {description && (
          <p className="text-sm text-muted-foreground mt-1 max-w-xs">{description}</p>
        )}
      </div>
      {action && <div>{action}</div>}
    </div>
  );
}

export function SearchEmptyState({ query }: { query: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 gap-4 text-center px-6">
      <div className="p-4 rounded-full bg-muted/50 border border-border">
        <Search className="w-8 h-8 text-muted-foreground" />
      </div>
      <div>
        <h3 className="font-semibold text-base font-mono">No results for "{query}"</h3>
        <p className="text-sm text-muted-foreground mt-1">
          Try a different query, or run <code className="text-primary">omni search index .</code> to index files first.
        </p>
      </div>
    </div>
  );
}

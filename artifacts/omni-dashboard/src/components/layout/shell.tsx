import { useEffect, useState } from "react";
import { Link, useLocation } from "wouter";
import { getGetModuleStatusQueryKey, useGetModuleStatus } from "@workspace/api-client-react";
import {
  TerminalSquare,
  Search,
  FolderSearch,
  Archive,
  DatabaseBackup,
  BookOpen,
  ArrowRightLeft,
  Wrench,
  Activity,
  Menu,
  X,
  TriangleAlert,
  CircleOff,
} from "lucide-react";
import { cn } from "@/lib/utils";

const navItems = [
  { path: "/",          label: "Dashboard",       icon: Activity },
  { path: "/search",    label: "Global Search",   icon: Search },
  { path: "/files",     label: "File Finder",     icon: FolderSearch },
  { path: "/archive",   label: "Archive Inspector", icon: Archive },
  { path: "/backup",    label: "Backup Ops",      icon: DatabaseBackup },
  { path: "/workspace", label: "Workspace",       icon: BookOpen },
  { path: "/convert",   label: "Format Converter", icon: ArrowRightLeft },
  { path: "/dev",       label: "Dev Toolkit",     icon: Wrench },
];

interface SidebarContentProps {
  onNavClick?: () => void;
}

type CliState =
  | { kind: "checking" }
  | { kind: "online"; modules: number }
  | { kind: "cli-missing"; message: string }
  | { kind: "api-offline" };

/**
 * Real readiness indicator. It reports what the last /api/dashboard/module-status
 * call actually returned: the CLI's module count when reachable, the API's
 * "CLI not found" message when the binary is missing, or an offline notice when
 * the API itself cannot be reached. Nothing here is hardcoded to a happy path.
 */
function CliStatus() {
  const { data, isError, error, isLoading } = useGetModuleStatus({
    query: {
      queryKey: getGetModuleStatusQueryKey(),
      retry: false,
      refetchInterval: 30_000,
      staleTime: 15_000,
    },
  });

  const state: CliState = isLoading
    ? { kind: "checking" }
    : isError
      ? (error as { status?: number })?.status === 503
        ? { kind: "cli-missing", message: (error as Error)?.message ?? "omnicli binary not found" }
        : { kind: "api-offline" }
      : { kind: "online", modules: data?.length ?? 0 };

  const dot =
    state.kind === "online"
      ? "bg-primary"
      : state.kind === "checking"
        ? "bg-muted-foreground"
        : "bg-destructive";

  const label =
    state.kind === "online"
      ? `CLI online · ${state.modules} modules`
      : state.kind === "checking"
        ? "Checking CLI…"
        : state.kind === "cli-missing"
          ? "CLI not found"
          : "API offline";

  return (
    <div
      className="flex items-start gap-2 text-xs text-muted-foreground"
      role="status"
      aria-live="polite"
      title={state.kind === "cli-missing" ? state.message : undefined}
    >
      <span
        className={cn(
          "mt-1 w-2 h-2 rounded-full flex-shrink-0",
          dot,
          state.kind === "online" && "motion-safe:animate-pulse"
        )}
      />
      <span className="flex items-center gap-1">
        {state.kind === "cli-missing" && <TriangleAlert className="w-3 h-3 text-destructive" />}
        {state.kind === "api-offline" && <CircleOff className="w-3 h-3 text-destructive" />}
        {label}
      </span>
    </div>
  );
}

function SidebarContent({ onNavClick }: SidebarContentProps) {
  const [location] = useLocation();
  return (
    <>
      <div className="h-16 flex items-center px-6 border-b border-border flex-shrink-0">
        <Link href="/" className="flex items-center gap-2 group outline-none" onClick={onNavClick}>
          <TerminalSquare className="w-6 h-6 text-primary group-hover:text-primary-foreground transition-colors group-hover:bg-primary rounded-sm p-0.5" />
          <span className="font-bold text-lg tracking-tight text-sidebar-foreground group-hover:text-primary transition-colors">
            OmniCLI<span className="text-primary motion-safe:animate-pulse">_</span>
          </span>
        </Link>
      </div>

      <div className="p-4 flex-1 overflow-y-auto">
        <div className="text-xs font-semibold text-muted-foreground mb-4 uppercase tracking-wider">
          Operations Core
        </div>
        <nav className="space-y-1">
          {navItems.map((item) => {
            const isActive = location === item.path;
            const Icon = item.icon;
            return (
              <Link
                key={item.path}
                href={item.path}
                onClick={onNavClick}
                className={cn(
                  "flex items-center gap-3 px-3 py-2 text-sm transition-all outline-none focus-visible:ring-1 focus-visible:ring-primary rounded-sm",
                  isActive
                    ? "bg-accent text-accent-foreground border-l-2 border-primary pl-2.5"
                    : "text-sidebar-foreground hover:bg-secondary hover:text-secondary-foreground border-l-2 border-transparent pl-2.5"
                )}
              >
                <Icon className={cn("w-4 h-4 flex-shrink-0", isActive ? "text-primary" : "text-muted-foreground")} />
                {item.label}
              </Link>
            );
          })}
        </nav>
      </div>

      <div className="p-4 border-t border-border bg-muted/20 flex-shrink-0">
        <CliStatus />
      </div>
    </>
  );
}

export function Sidebar() {
  return (
    <aside className="w-64 border-r border-border bg-sidebar h-screen flex flex-col fixed left-0 top-0 z-40">
      <SidebarContent />
    </aside>
  );
}

export function MobileHeader({ onOpen }: { onOpen: () => void }) {
  return (
    <header className="h-14 border-b border-border bg-sidebar flex items-center px-4 gap-3 lg:hidden">
      <button
        onClick={onOpen}
        className="p-2 rounded-sm text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
        aria-label="Open navigation"
      >
        <Menu className="w-5 h-5" />
      </button>
      <Link href="/" className="flex items-center gap-2 group outline-none">
        <TerminalSquare className="w-5 h-5 text-primary" />
        <span className="font-bold tracking-tight text-sidebar-foreground">
          OmniCLI<span className="text-primary motion-safe:animate-pulse">_</span>
        </span>
      </Link>
    </header>
  );
}

export function MobileDrawer({ open, onClose }: { open: boolean; onClose: () => void }) {
  // Escape closes the drawer, matching the backdrop click.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return (
    <>
      {/* Backdrop */}
      <div
        className={cn(
          "fixed inset-0 z-50 bg-black/60 backdrop-blur-sm transition-opacity duration-300 motion-reduce:transition-none lg:hidden",
          open ? "opacity-100 pointer-events-auto" : "opacity-0 pointer-events-none"
        )}
        onClick={onClose}
        aria-hidden="true"
      />
      {/* Drawer */}
      <aside
        role="dialog"
        aria-modal="true"
        aria-label="Navigation"
        className={cn(
          "fixed top-0 left-0 z-50 h-screen w-72 bg-sidebar border-r border-border flex flex-col transition-transform duration-300 motion-reduce:transition-none lg:hidden",
          open ? "translate-x-0" : "-translate-x-full"
        )}
      >
        <div className="absolute top-3 right-3">
          <button
            onClick={onClose}
            className="p-1.5 rounded-sm text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
            aria-label="Close navigation"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
        <SidebarContent onNavClick={onClose} />
      </aside>
    </>
  );
}

export function Shell({ children }: { children: React.ReactNode }) {
  const [drawerOpen, setDrawerOpen] = useState(false);

  return (
    <div className="flex min-h-screen w-full bg-background text-foreground">
      {/* Keyboard users can jump straight past the navigation. */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-3 focus:py-2 focus:bg-primary focus:text-primary-foreground focus:rounded-sm focus:text-sm"
      >
        Skip to content
      </a>

      {/* Desktop sidebar */}
      <div className="hidden lg:block">
        <Sidebar />
      </div>

      {/* Mobile drawer */}
      <MobileDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />

      {/* Main content */}
      <main id="main-content" className="flex-1 lg:ml-64 min-w-0 flex flex-col">
        <MobileHeader onOpen={() => setDrawerOpen(true)} />
        {children}
      </main>
    </div>
  );
}

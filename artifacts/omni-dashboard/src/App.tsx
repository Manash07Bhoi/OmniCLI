import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Toaster } from '@/components/ui/toaster';
import { TooltipProvider } from '@/components/ui/tooltip';
import NotFound from '@/pages/not-found';
import { Route, Switch, Router as WouterRouter } from 'wouter';
import { Shell } from '@/components/layout/shell';

import Dashboard from '@/pages/dashboard';
import SearchPage from '@/pages/search';
import FilesPage from '@/pages/files';
import ArchivePage from '@/pages/archive';
import BackupPage from '@/pages/backup';
import WorkspacePage from '@/pages/workspace';
import ConvertPage from '@/pages/convert';
import DevToolkitPage from '@/pages/dev';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function Router() {
  return (
    <Shell>
      <Switch>
        <Route path="/" component={Dashboard} />
        <Route path="/search" component={SearchPage} />
        <Route path="/files" component={FilesPage} />
        <Route path="/archive" component={ArchivePage} />
        <Route path="/backup" component={BackupPage} />
        <Route path="/workspace" component={WorkspacePage} />
        <Route path="/convert" component={ConvertPage} />
        <Route path="/dev" component={DevToolkitPage} />
        <Route component={NotFound} />
      </Switch>
    </Shell>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <WouterRouter base={import.meta.env.BASE_URL.replace(/\/$/, '')}>
          <Router />
        </WouterRouter>
        <Toaster />
      </TooltipProvider>
    </QueryClientProvider>
  );
}

export default App;

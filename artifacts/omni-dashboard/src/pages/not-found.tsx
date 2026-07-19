import { Terminal } from "lucide-react";
import { Link } from "wouter";

export default function NotFound() {
  return (
    <div className="flex items-center justify-center min-h-screen bg-background text-foreground animate-in fade-in zoom-in-95">
      <div className="max-w-md w-full p-8 border border-destructive/50 bg-destructive/5 relative overflow-hidden">
        <div className="absolute top-0 left-0 w-full h-1 bg-destructive/50"></div>
        <div className="flex flex-col items-center text-center space-y-4">
          <div className="p-4 bg-background rounded-full border border-destructive/30">
            <Terminal className="w-12 h-12 text-destructive" />
          </div>
          <h1 className="text-4xl font-bold font-mono tracking-tighter text-destructive">404_ERR</h1>
          <p className="text-muted-foreground font-mono">
            Path not found in operations registry. The requested module or resource does not exist or has been decommissioned.
          </p>
          <div className="mt-8 pt-6 border-t border-border w-full">
            <Link href="/" className="inline-flex items-center justify-center whitespace-nowrap text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 border border-input bg-background hover:bg-accent hover:text-accent-foreground h-10 px-8 font-mono w-full">
              RETURN TO DASHBOARD
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}

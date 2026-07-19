import { useState } from "react";
import { useListConvertFormats, useRunConversion } from "@workspace/api-client-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ArrowRightLeft, RefreshCw, Upload, Download, ChevronRight } from "lucide-react";
import { formatBytes, formatMs } from "@/lib/utils";
import { SkeletonCard, ErrorState, EmptyState, LoadingSpinner } from "@/components/ui/states";

export default function ConvertPage() {
  const [inputPath, setInputPath] = useState("");
  const [outputPath, setOutputPath] = useState("");

  const { data: formats, isLoading: formatsLoading, isError: formatsError } = useListConvertFormats();
  const convert = useRunConversion();

  const handleConvert = (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputPath || !outputPath) return;
    convert.mutate({ data: { inputPath, outputPath } });
  };

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 max-w-[1600px] mx-auto w-full animate-in fade-in">
      <div>
        <h1 className="text-2xl sm:text-3xl font-bold text-primary terminal-text-glow flex items-center gap-3">
          <ArrowRightLeft className="w-6 h-6 sm:w-8 sm:h-8" />
          Format Converter
        </h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Schema-preserving file conversions — format inferred from extension.
        </p>
      </div>

      {/* Converter form */}
      <Card className="border-primary/30 glow-border bg-card/80">
        <CardContent className="p-4 sm:p-6">
          <form onSubmit={handleConvert} className="space-y-4">
            <div className="grid sm:grid-cols-[1fr_auto_1fr] gap-3 items-center">
              <div>
                <label className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1 block">
                  Input path
                </label>
                <div className="relative">
                  <Upload className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <Input
                    value={inputPath}
                    onChange={(e) => setInputPath(e.target.value)}
                    placeholder="/path/to/data.csv"
                    className="pl-9 font-mono h-11"
                  />
                </div>
              </div>
              <div className="flex justify-center pt-4 sm:pt-0">
                <div className="p-2 rounded-full bg-primary/10 border border-primary/30">
                  <ChevronRight className="w-5 h-5 text-primary" />
                </div>
              </div>
              <div>
                <label className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1 block">
                  Output path
                </label>
                <div className="relative">
                  <Download className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <Input
                    value={outputPath}
                    onChange={(e) => setOutputPath(e.target.value)}
                    placeholder="/path/to/data.json"
                    className="pl-9 font-mono h-11"
                  />
                </div>
              </div>
            </div>
            <Button
              type="submit"
              className="w-full h-11 font-bold tracking-wider"
              disabled={convert.isPending || !inputPath || !outputPath}
            >
              {convert.isPending
                ? <><RefreshCw className="w-4 h-4 animate-spin mr-2" />Converting…</>
                : <><ArrowRightLeft className="w-4 h-4 mr-2" />CONVERT</>}
            </Button>
          </form>

          {/* Result */}
          {convert.isError && (
            <div className="mt-4">
              <ErrorState compact title="Conversion failed" message={(convert.error as Error)?.message} />
            </div>
          )}

          {convert.isSuccess && convert.data && (
            <div className="mt-4 p-4 border border-emerald-500/30 bg-emerald-500/10 rounded-sm space-y-2">
              <div className="flex items-center gap-2 text-emerald-400 font-semibold text-sm">
                ✓ Conversion complete
              </div>
              <div className="grid grid-cols-3 gap-4 text-xs font-mono">
                <div>
                  <div className="text-muted-foreground mb-1">Input</div>
                  <div className="text-foreground font-bold uppercase">{convert.data.fromFormat}</div>
                  {(convert.data.inputSizeBytes ?? 0) > 0 && (
                    <div className="text-muted-foreground">{formatBytes(convert.data.inputSizeBytes ?? 0)}</div>
                  )}
                </div>
                <div className="flex flex-col items-center justify-center">
                  <ChevronRight className="w-5 h-5 text-primary" />
                </div>
                <div>
                  <div className="text-muted-foreground mb-1">Output</div>
                  <div className="text-primary font-bold uppercase">{convert.data.toFormat}</div>
                  {(convert.data.outputSizeBytes ?? 0) > 0 && (
                    <div className="text-muted-foreground">{formatBytes(convert.data.outputSizeBytes ?? 0)}</div>
                  )}
                </div>
              </div>
              {convert.data.durationMs > 0 && (
                <div className="text-xs text-muted-foreground">
                  Completed in {formatMs(convert.data.durationMs)}
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Supported format pairs */}
      <div>
        <h2 className="text-base font-bold mb-3 text-muted-foreground uppercase tracking-wider text-xs">
          Supported Pairs
        </h2>
        {formatsLoading ? (
          <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <SkeletonCard key={i} rows={2} />
            ))}
          </div>
        ) : formatsError ? (
          <ErrorState compact title="Could not load format pairs" />
        ) : !formats?.length ? (
          <EmptyState title="No conversion pairs loaded" />
        ) : (
          <div className="grid sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            {formats.map((pair, i) => (
              <button
                key={i}
                type="button"
                onClick={() => {
                  setOutputPath((p) => {
                    const base = p.split(".").slice(0, -1).join(".");
                    return base ? `${base}.${pair.to}` : `output.${pair.to}`;
                  });
                  if (!inputPath) setInputPath(`input.${pair.from}`);
                }}
                className="flex items-center gap-3 p-3 border border-border rounded-sm text-left hover:border-primary/50 hover:bg-secondary/20 transition-colors group"
              >
                <Badge variant="secondary" className="font-mono text-[11px] flex-shrink-0 group-hover:bg-primary group-hover:text-primary-foreground transition-colors">
                  {pair.from.toUpperCase()}
                </Badge>
                <ChevronRight className="w-3.5 h-3.5 text-muted-foreground flex-shrink-0" />
                <Badge variant="outline" className="font-mono text-[11px] flex-shrink-0 border-primary/40 text-primary">
                  {pair.to.toUpperCase()}
                </Badge>
                <span className="text-xs text-muted-foreground truncate">{pair.description}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

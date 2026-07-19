import { execFile } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

/** Resolve the omni binary path — prefers release build, falls back to debug. */
function omniPath(): string {
  const workspace = process.env["WORKSPACE_ROOT"] ?? path.resolve(__dirname, "..", "..", "..", "..");
  const release = path.join(workspace, "omnicli", "target", "release", "omni");
  const debug   = path.join(workspace, "omnicli", "target", "debug", "omni");
  // Synchronous existence check via require — acceptable once at startup
  try {
    require("node:fs").accessSync(release);
    return release;
  } catch {
    try {
      require("node:fs").accessSync(debug);
      return debug;
    } catch {
      return "omni"; // hope it's on PATH
    }
  }
}

export interface ExecResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  exitCode: number;
}

/**
 * Run `omni --json <args>` and return the parsed JSON output.
 * Throws if the binary is not found or exits non-zero and producess no JSON.
 */
export async function omniJson<T>(args: string[], timeoutMs = 30_000): Promise<T> {
  const bin = omniPath();
  try {
    const { stdout } = await execFileAsync(bin, ["--json", ...args], {
      timeout: timeoutMs,
      maxBuffer: 10 * 1024 * 1024,
    });
    return JSON.parse(stdout) as T;
  } catch (err: unknown) {
    if (err && typeof err === "object" && "stdout" in err) {
      const e = err as { stdout?: string; stderr?: string; code?: number };
      const raw = (e.stdout ?? "").trim();
      if (raw) {
        try {
          return JSON.parse(raw) as T;
        } catch {
          // fall through
        }
      }
      throw new Error(`omni ${args[0]} failed (exit ${e.code ?? "?"}): ${e.stderr ?? raw}`);
    }
    throw err;
  }
}

/**
 * Quick check: does the omni binary exist and respond to --version?
 */
export async function omniAvailable(): Promise<boolean> {
  try {
    await execFileAsync(omniPath(), ["--version"], { timeout: 5_000 });
    return true;
  } catch {
    return false;
  }
}

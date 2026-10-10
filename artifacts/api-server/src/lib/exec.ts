import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

/**
 * Thrown when the `omnicli` binary cannot be found or executed. Routes turn this
 * into an HTTP 503 so the dashboard can show an actionable "CLI not available"
 * state instead of fabricated data.
 */
export class OmniUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "OmniUnavailableError";
  }
}

export function isOmniUnavailable(err: unknown): err is OmniUnavailableError {
  return err instanceof OmniUnavailableError;
}

/** Names the binary has shipped under (both are valid today). */
const BINARY_NAMES = ["omnicli", "omni"];

function moduleDir(): string {
  try {
    return path.dirname(fileURLToPath(import.meta.url));
  } catch {
    return process.cwd();
  }
}

/**
 * Directories to search for a built `omnicli/target/{release,debug}/omnicli`.
 *
 * `OMNICLI_ROOT` (or `WORKSPACE_ROOT`) pins the search to exactly those roots —
 * that is what deployments and CI set, and it makes the resolver deterministic
 * (and testable) instead of depending on where the server happens to live.
 * Without it, the walk starts at the module and the CWD, which works both when
 * the server runs from source and from the bundled `dist/index.mjs`.
 */
function searchRoots(): string[] {
  const roots = new Set<string>();
  const envRoots = [process.env["OMNICLI_ROOT"], process.env["WORKSPACE_ROOT"]].filter(
    (r): r is string => Boolean(r)
  );
  for (const r of envRoots) roots.add(path.resolve(r));
  if (envRoots.length > 0) return [...roots];

  for (const start of [moduleDir(), process.cwd()]) {
    let dir = path.resolve(start);
    for (let depth = 0; depth < 6; depth++) {
      roots.add(dir);
      const parent = path.dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }
  return [...roots];
}

function builtBinaryCandidates(): string[] {
  const out: string[] = [];
  for (const root of searchRoots()) {
    for (const profile of ["release", "debug"]) {
      for (const name of BINARY_NAMES) {
        out.push(path.join(root, "omnicli", "target", profile, name));
      }
    }
  }
  return out;
}

/** Candidate executables in priority order: explicit override, build, then PATH. */
export function omniCandidates(): string[] {
  const candidates: string[] = [];
  const override = process.env["OMNICLI_BIN"];
  if (override) candidates.push(override);
  candidates.push(...builtBinaryCandidates().filter((p) => existsSync(p)));
  candidates.push(...BINARY_NAMES); // resolved through PATH by execFile
  return [...new Set(candidates)];
}

function isEnoent(err: unknown): boolean {
  return (
    typeof err === "object" &&
    err !== null &&
    (err as { code?: string }).code === "ENOENT"
  );
}

/** The binary that most recently executed successfully, if any. */
let resolvedBinary: string | undefined;

function unavailableMessage(): string {
  return [
    "Could not find the omnicli binary.",
    "Build it with `cd omnicli && cargo build --release`, install it on PATH,",
    "or point OMNICLI_BIN at the executable.",
  ].join(" ");
}

export interface ExecResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  exitCode: number;
}

/**
 * Run the CLI with `args`. Tries each candidate binary until one executes
 * (ENOENT means "not this one"); the first that works is remembered.
 * Non-zero exits are returned rather than thrown, so callers can inspect stderr.
 */
export async function omniExec(
  args: string[],
  timeoutMs = 30_000
): Promise<ExecResult> {
  const candidates = resolvedBinary
    ? [resolvedBinary, ...omniCandidates().filter((c) => c !== resolvedBinary)]
    : omniCandidates();

  let lastEnoent: unknown;
  for (const bin of candidates) {
    try {
      const { stdout, stderr } = await execFileAsync(bin, args, {
        timeout: timeoutMs,
        maxBuffer: 10 * 1024 * 1024,
      });
      resolvedBinary = bin;
      return { ok: true, stdout, stderr, exitCode: 0 };
    } catch (err: unknown) {
      if (isEnoent(err)) {
        lastEnoent = err;
        continue;
      }
      const e = err as {
        stdout?: string;
        stderr?: string;
        code?: number | string;
      };
      resolvedBinary = bin;
      return {
        ok: false,
        stdout: e.stdout ?? "",
        stderr: e.stderr ?? "",
        exitCode: typeof e.code === "number" ? e.code : 1,
      };
    }
  }
  throw new OmniUnavailableError(
    `${unavailableMessage()} (last error: ${String(lastEnoent)})`
  );
}

/** Structured failure reported by the CLI in `--json` mode. */
interface CliJsonError {
  error?: string;
  exit_code?: number;
}

/** Extract the CLI's own error message from a JSON error payload. */
function cliErrorMessage(text: string): string | null {
  const trimmed = text.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed) as { error?: unknown };
    if (typeof parsed.error === "string" && parsed.error) return parsed.error;
  } catch {
    // not JSON
  }
  return null;
}

/**
 * Run `omnicli --json <args>` and parse stdout as JSON.
 *
 * In `--json` mode the CLI writes success payloads to stdout and error payloads
 * (`{"error": …}`) to stderr, so both are inspected: a non-zero exit whose
 * stdout still holds valid JSON is returned to the caller, and otherwise the
 * CLI's own message is surfaced without the surrounding wire format.
 */
export async function omniJson<T>(args: string[], timeoutMs = 30_000): Promise<T> {
  const result = await omniExec(["--json", ...args], timeoutMs);
  const raw = result.stdout.trim();
  if (raw) {
    try {
      return JSON.parse(raw) as T;
    } catch {
      if (!result.ok) {
        throw new Error(
          cliErrorMessage(result.stderr) ??
            `omnicli ${args.join(" ")} failed (exit ${result.exitCode}): ${result.stderr.trim() || raw}`
        );
      }
      throw new Error(`omnicli ${args.join(" ")} returned invalid JSON: ${raw.slice(0, 200)}`);
    }
  }
  if (!result.ok) {
    throw new Error(
      cliErrorMessage(result.stderr) ??
        `omnicli ${args.join(" ")} failed (exit ${result.exitCode}): ${result.stderr.trim() || "no output"}`
    );
  }
  throw new Error(`omnicli ${args.join(" ")} produced no output`);
}

/**
 * Run a `--json` command that is expected to succeed, returning the parsed
 * payload or throwing the CLI's own error message.
 */
export async function omniJsonOk<T>(args: string[], timeoutMs = 30_000): Promise<T> {
  const payload = await omniJson<T & CliJsonError>(args, timeoutMs);
  if (payload && typeof payload === "object" && "error" in payload && payload.error) {
    throw new Error(payload.error);
  }
  return payload as T;
}

/** Quick check: does an omnicli binary exist and respond to --version? */
export async function omniAvailable(): Promise<boolean> {
  try {
    const result = await omniExec(["--version"], 5_000);
    return result.ok;
  } catch {
    return false;
  }
}

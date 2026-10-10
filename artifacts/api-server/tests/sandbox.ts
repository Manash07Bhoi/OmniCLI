import { existsSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

/**
 * Sandbox handoff between vitest's global setup (a separate module context) and
 * the test workers. The paths are written to a JSON file whose location is
 * announced through the environment, so both sides see exactly the same
 * temporary HOME/XDG/DB sandbox.
 */
export interface Sandbox {
  root: string;
  home: string;
  data: string;
  work: string;
  dbPath: string;
  searchDbDir: string;
  repoRoot: string;
  cliBinary: string;
}

export const SANDBOX_ENV = "OMNI_TEST_SANDBOX_FILE";

function sandboxFile(): string {
  return process.env[SANDBOX_ENV] ?? path.join(os.tmpdir(), "omnicli-api-test-sandbox.json");
}

export function writeSandbox(sandbox: Sandbox): void {
  writeFileSync(sandboxFile(), JSON.stringify(sandbox, null, 2));
}

let cached: Sandbox | null = null;

export function readSandbox(): Sandbox {
  if (cached) return cached;
  const file = sandboxFile();
  if (!existsSync(file)) {
    throw new Error(
      `Test sandbox metadata not found at ${file}. globalSetup must run before the suites.`
    );
  }
  cached = JSON.parse(readFileSync(file, "utf8")) as Sandbox;
  return cached;
}

import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import type { Express } from "express";
import { readSandbox, type Sandbox } from "./sandbox";

const execFileAsync = promisify(execFile);

/** Sandbox paths created by the global setup. */
export function sandbox(): Sandbox {
  return readSandbox();
}

/** The real Express app, imported after the sandbox environment is in place. */
export async function loadApp(): Promise<Express> {
  const sb = sandbox();
  // Workers get the sandbox through the file, so re-assert the environment the
  // app reads at import time.
  process.env["DB_PATH"] = sb.dbPath;
  process.env["XDG_DATA_HOME"] = sb.data;
  process.env["HOME"] = sb.home;
  process.env["OMNICLI_ROOT"] ??= sb.repoRoot;

  const mod = await import("../src/app");
  return mod.default;
}

export function cliBinary(): string {
  return sandbox().cliBinary;
}

/** Run the real CLI and return parsed JSON (used to cross-check API output). */
export async function cliJson<T>(args: string[]): Promise<T> {
  const { stdout } = await execFileAsync(cliBinary(), ["--json", ...args], {
    env: { ...process.env },
    timeout: 60_000,
    maxBuffer: 16 * 1024 * 1024,
  });
  return JSON.parse(stdout) as T;
}

export function fixturesDir(): string {
  return path.join(sandbox().work, "fixtures");
}

/** A unique temp directory under the sandbox work dir. */
export async function makeTempDir(name: string): Promise<string> {
  const dir = path.join(sandbox().work, name);
  await fs.rm(dir, { recursive: true, force: true });
  await fs.mkdir(dir, { recursive: true });
  return dir;
}

export async function makeTree(name: string, files: Record<string, string>): Promise<string> {
  const dir = await makeTempDir(name);
  for (const [rel, content] of Object.entries(files)) {
    const full = path.join(dir, rel);
    await fs.mkdir(path.dirname(full), { recursive: true });
    await fs.writeFile(full, content);
  }
  return dir;
}

export async function fileExists(p: string): Promise<boolean> {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

/** Distinct-ish names keep unique-constrained tables independent per test. */
let counter = 0;
export function uniqueName(prefix: string): string {
  counter += 1;
  return `${prefix}-${Date.now().toString(36)}-${counter}`;
}

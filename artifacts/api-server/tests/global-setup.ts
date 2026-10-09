import { execFile } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { SANDBOX_ENV, writeSandbox, type Sandbox } from "./sandbox";

const execFileAsync = promisify(execFile);

const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");

/**
 * Create the database schema with the documented setup command
 * (`pnpm --filter @workspace/db run push`) instead of hand-written DDL, so a
 * regression in that step fails the suite.
 */
async function pushSchema(dbPath: string): Promise<void> {
  const env = { ...process.env, DB_PATH: dbPath };
  try {
    await execFileAsync("pnpm", ["--filter", "@workspace/db", "run", "push"], {
      cwd: REPO_ROOT,
      env,
      timeout: 180_000,
    });
  } catch (err) {
    const e = err as { code?: string; stderr?: string; message?: string };
    if (e.code === "ENOENT") {
      // pnpm supplied by corepack when it is not on PATH.
      await execFileAsync("corepack", ["pnpm", "--filter", "@workspace/db", "run", "push"], {
        cwd: REPO_ROOT,
        env,
        timeout: 180_000,
      });
      return;
    }
    throw new Error(`drizzle-kit push failed: ${e.stderr ?? e.message}`);
  }
}

export default async function setup(): Promise<() => Promise<void>> {
  const root = mkdtempSync(path.join(os.tmpdir(), "omnicli-api-test-"));
  const home = path.join(root, "home");
  const data = path.join(root, "data");
  const work = path.join(root, "work");
  for (const dir of [home, data, work]) mkdirSync(dir, { recursive: true });

  const cliBinary = path.join(REPO_ROOT, "omnicli", "target", "debug", "omnicli");
  const sandbox: Sandbox = {
    root,
    home,
    data,
    work,
    dbPath: path.join(root, "omni.db"),
    searchDbDir: path.join(data, "omni"),
    repoRoot: REPO_ROOT,
    cliBinary,
  };

  // Hand the sandbox to the workers through a file whose location is announced
  // in the environment (env vars alone are not reliably shared with the fork).
  const sandboxFile = path.join(root, "sandbox.json");
  process.env[SANDBOX_ENV] = sandboxFile;
  writeSandbox(sandbox);

  // The database singleton and the CLI resolver both read the environment at
  // import time, so it must be in place before any suite imports the app.
  process.env["DB_PATH"] = sandbox.dbPath;
  process.env["XDG_DATA_HOME"] = data;
  process.env["HOME"] = home;
  process.env["OMNICLI_ROOT"] = REPO_ROOT;
  // Keep the suite output readable: request logs are not the subject of the
  // tests, but warnings and errors still surface.
  process.env["LOG_LEVEL"] ??= "warn";
  delete process.env["OMNICLI_BIN"];

  if (!existsSync(cliBinary)) {
    throw new Error(
      `The omnicli binary is required for these tests but was not found at ${cliBinary}. ` +
        `Run: cd omnicli && cargo build`
    );
  }

  await pushSchema(sandbox.dbPath);

  // A small fixture tree shared by the search, archive, backup and convert suites.
  const fixtures = path.join(work, "fixtures");
  mkdirSync(path.join(fixtures, "sub"), { recursive: true });
  writeFileSync(path.join(fixtures, "notes.txt"), "alpha bravo charlie\nsecond line\n");
  writeFileSync(path.join(fixtures, "people.csv"), "name,age\nalice,30\nbob,25\n");
  writeFileSync(path.join(fixtures, "sub", "deep.txt"), "charlie delta echo\n");
  writeFileSync(path.join(fixtures, "unicode-日本語-😀.txt"), "unicode content\n");

  // Remove the sandbox when the run ends: each run creates a database, an index
  // and fixture copies, so leaving them behind fills up /tmp on repeated runs.
  return async () => {
    try {
      rmSync(root, { recursive: true, force: true });
    } catch {
      // A leftover temp directory must never fail a test run.
    }
  };
}

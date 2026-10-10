#!/usr/bin/env node
/**
 * Runs `orval --config ./orval.config.ts`.
 *
 * orval declares `engines.node >= 22.18.0` and uses `Map.groupBy`, so on Node 20
 * it crashes with an unhelpful "Map.groupBy is not a function" *after* deleting
 * the generated output. This wrapper checks the version first and explains how
 * to proceed, and it leaves the existing generated files untouched.
 */
import { spawnSync } from "node:child_process";

const [major, minor] = process.versions.node.split(".").map(Number);
const REQUIRED_MAJOR = 22;
const REQUIRED_MINOR = 18;

if (major < REQUIRED_MAJOR || (major === REQUIRED_MAJOR && minor < REQUIRED_MINOR)) {
  console.error(
    `orval requires Node >= ${REQUIRED_MAJOR}.${REQUIRED_MINOR}.0 but this is Node ${process.versions.node}.\n` +
      "Regenerating the API clients would fail and would first delete lib/api-zod/src/generated.\n" +
      "Nothing was changed. Switch to Node 22.18+ (e.g. `nvm use 22`) and run `pnpm codegen` again."
  );
  process.exit(1);
}

const result = spawnSync("orval", ["--config", "./orval.config.ts"], {
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);

import { defineConfig } from "vitest/config";

/**
 * API-server test suite.
 *
 * The tests are integration tests on purpose: they boot the real Express app
 * with the real SQLite schema, spawn the real `omnicli` binary, and use real
 * files in a throwaway sandbox. Nothing is stubbed with fake data, so a passing
 * run means the endpoint genuinely worked.
 */
export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    globalSetup: ["tests/global-setup.ts"],
    // The CLI is a child process; give it room but keep failures bounded.
    testTimeout: 30_000,
    hookTimeout: 60_000,
    // The database singleton is per-process, so keep suites in one worker.
    pool: "forks",
    maxWorkers: 1,
    reporters: ["default"],
  },
});

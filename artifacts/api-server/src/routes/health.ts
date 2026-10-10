import { Router, type IRouter } from "express";
import { HealthCheckResponse } from "@workspace/api-zod";
import { omniAvailable } from "../lib/exec";
import { workspaceStore } from "../lib/workspace";

const router: IRouter = Router();

/**
 * Liveness: the process is up and can serve HTTP. Deliberately cheap and
 * dependency-free so a supervisor's health check never fails because a
 * downstream component is slow.
 */
router.get("/healthz", (_req, res) => {
  const data = HealthCheckResponse.parse({ status: "ok" });
  res.json(data);
});

/**
 * Readiness: can this instance actually do its job right now?
 *
 * Two real checks, each answering a question the dashboard would otherwise
 * discover one broken request at a time:
 *   - the workspace database opens and answers a query
 *   - an `omnicli` binary is present and responds to `--version`
 *
 * The response always lists both checks so a client can distinguish "the DB is
 * fine but the CLI is missing" from "nothing is up". A not-ready instance
 * returns 503 so a load balancer stops sending it traffic; nothing is reported
 * as OK that was not measured.
 */
router.get("/readyz", async (_req, res): Promise<void> => {
  const checks: Record<string, string> = {};
  let ready = true;

  try {
    const store = await workspaceStore();
    // A real read, not a connection test: it fails if the file is corrupt, the
    // schema is missing, or the database is locked by another writer.
    store.sqlite.prepare("SELECT COUNT(*) AS n FROM notes").get();
    checks["database"] = "ok";
  } catch (err) {
    ready = false;
    checks["database"] = `error: ${(err as Error).message}`;
  }

  try {
    checks["cli"] = (await omniAvailable()) ? "ok" : "unavailable";
    if (checks["cli"] !== "ok") ready = false;
  } catch (err) {
    ready = false;
    checks["cli"] = `error: ${(err as Error).message}`;
  }

  res.status(ready ? 200 : 503).json({
    status: ready ? "ready" : "not-ready",
    checks,
  });
});

export default router;

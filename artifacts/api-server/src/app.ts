import { randomUUID } from "node:crypto";
import express, { type Express } from "express";
import cors from "cors";
import pinoHttp from "pino-http";
import router from "./routes";
import { logger } from "./lib/logger";

const app: Express = express();

// The framework banner is free reconnaissance for anyone probing the service and
// buys nothing, so it is not sent.
app.disable("x-powered-by");

/**
 * Client-supplied correlation ids are honoured only when they look like an id.
 * Anything else (newlines, log-injection payloads, unbounded strings) is
 * replaced with a generated UUID, so the value is always safe to log and echo.
 */
export function requestIdFrom(header: unknown): string {
  if (typeof header === "string" && /^[A-Za-z0-9._-]{1,128}$/.test(header)) return header;
  return randomUUID();
}

app.use(
  pinoHttp({
    logger,
    genReqId: (req, res) => {
      const id = requestIdFrom(req.headers["x-request-id"]);
      // Echo it so a user can quote the id from a support report or a curl -i
      // response and find the matching log line.
      res.setHeader("X-Request-Id", id);
      return id;
    },
    serializers: {
      req(req) {
        return {
          id: req.id,
          method: req.method,
          url: req.url?.split("?")[0],
        };
      },
      res(res) {
        return {
          statusCode: res.statusCode,
        };
      },
    },
  }),
);
// Security headers. This API only ever returns JSON, so the policy can be
// strict: nothing may load resources, be framed, or be sniffed as another type.
// (The dashboard is a separate origin and sets its own headers.)
app.use((_req, res, next): void => {
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("X-Frame-Options", "DENY");
  res.setHeader("Referrer-Policy", "no-referrer");
  res.setHeader("Cross-Origin-Resource-Policy", "same-origin");
  res.setHeader(
    "Content-Security-Policy",
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
  );
  next();
});

/**
 * CORS is deny-by-default for browsers. Same-origin deployments (the
 * documented setup: the built dashboard and this API behind one origin, or the
 * Vite dev proxy) need no CORS at all. A development server on another port is
 * allowed automatically; a real deployment opts in explicitly with
 * CORS_ORIGIN="https://dash.example.com,https://other.example.com" or "*".
 */
export function corsOptions(env: NodeJS.ProcessEnv = process.env) {
  const configured = (env["CORS_ORIGIN"] ?? "")
    .split(",")
    .map((o) => o.trim())
    .filter(Boolean);

  if (env["NODE_ENV"] === "production") {
    if (configured.length === 0) return { origin: false } as const;
    if (configured.includes("*")) return { origin: true } as const;
    return { origin: configured } as const;
  }

  // Development: the Vite dev/preview servers, plus anything explicitly listed.
  const devOrigins = [
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:4173",
    "http://127.0.0.1:4173",
    ...configured.filter((o) => o !== "*"),
  ];
  if (configured.includes("*")) return { origin: true } as const;
  return { origin: devOrigins } as const;
}

app.use(cors(corsOptions()));
// Bound the body size: the endpoints accept small JSON documents, and an
// unbounded parser is a trivial memory-exhaustion vector.
app.use(express.json({ limit: process.env["MAX_BODY"] ?? "1mb" }));
app.use(express.urlencoded({ extended: true, limit: process.env["MAX_BODY"] ?? "1mb" }));

app.use("/api", router);

// Unknown paths answer with JSON, never an HTML error page: the dashboard and
// any scripted client parse this API.
app.use((_req, res): void => {
  res.status(404).json({ error: "Not found" });
});

// Terminal error handler: log the failure, answer with JSON, and never leak
// stack traces or internals to the client.
app.use(
  (err: Error, req: express.Request, res: express.Response, _next: express.NextFunction): void => {
    if (res.headersSent) return;

    // Body-parser and friends attach their own status (malformed JSON 400,
    // payload too large 413). Honour it instead of flattening everything to 500.
    const status = (err as { status?: number; statusCode?: number }).status ??
      (err as { statusCode?: number }).statusCode;
    if (typeof status === "number" && status >= 400 && status < 500) {
      res.status(status).json({ error: err.message });
      return;
    }

    req.log?.error({ err }, "unhandled request error");
    res.status(500).json({
      error: "Internal server error",
      detail: process.env["NODE_ENV"] === "production" ? undefined : err.message,
    });
  },
);

export default app;

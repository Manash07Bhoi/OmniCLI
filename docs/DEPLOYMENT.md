# Deployment

OmniCLI has two deployable pieces and one that is optional:

| Piece | What it is | Needs |
|-------|-----------|-------|
| **CLI** (`omnicli`) | A single static-ish Rust binary. No runtimes, no services. | Nothing but the binary |
| **API server** (`artifacts/api-server`) | A Node/Express process that exposes the CLI's capabilities over HTTP for the dashboard | Node ≥ 20, an `omnicli` binary, a writable data directory |
| **Dashboard** (`artifacts/omni-dashboard`) | Static files (HTML/CSS/JS) that talk to the API | Any web server |

There is deliberately no database server, message broker, or container
orchestration requirement: **the simplest reliable architecture that works** is
one Node process plus static files. The CLI itself is the source of truth for
every operation — the API shells out to it rather than reimplementing it, so an
API deployment cannot drift away from the CLI's behaviour.

---

## 1. Build

```bash
# CLI (produces omnicli/target/release/omnicli)
cd omnicli && cargo build --release

# Node workspace (installs and builds API + dashboard)
cd .. && pnpm install --frozen-lockfile
pnpm build                       # typechecks, then builds every workspace project
```

Build outputs:

| Artifact | Path |
|----------|------|
| CLI binary | `omnicli/target/release/omnicli` |
| API bundle | `artifacts/api-server/dist/index.mjs` |
| Dashboard static site | `artifacts/omni-dashboard/dist/public/` |

## 2. Configuration model

Four environments, one mechanism: **environment variables**, with a `.env` file
as a developer convenience only. The API reads `.env` from
`artifacts/api-server/.env` or the repository root (`ENV_FILE` overrides the
path), never overriding variables that are already set.

| Environment | How it is set up | Notes |
|-------------|------------------|-------|
| **development** | `pnpm --filter @workspace/api-server dev`, `NODE_ENV` unset | Pretty logs, CORS allowed from the local Vite dev servers, `.env` loaded |
| **testing** | `pnpm --filter @workspace/api-server test` | Vitest builds a sandbox (`HOME`/`XDG_DATA_HOME`/`DB_PATH` redirected), so tests never touch real user data |
| **staging** | Same as production, with its own data directory and `NODE_ENV=production` | No `.env`-dependent defaults; everything explicit |
| **production** | `NODE_ENV=production node artifacts/api-server/dist/index.mjs` | JSON logs, CORS deny-by-default, no stack traces in responses |

**Production fails clearly when it cannot work.** The server validates `PORT`,
and logs a clear error when it cannot resolve a binary. It never starts
"successfully" in a state where every request would fail.

**Schema creation is automatic and non-destructive.** Opening the database
applies an idempotent DDL (`lib/db/src/schema-ddl.ts`: six tables plus two
indexes, `CREATE TABLE IF NOT EXISTS` only), so a fresh deployment works with
runtime dependencies alone — no build tools or `drizzle-kit push` needed. The
startup schema check then acts as a tripwire: if any required table is still
missing after that automatic creation, the environment is broken (usually an
unwritable `DB_PATH` or a corrupt file) and the server exits with an error
naming the missing tables. The DDL is drift-checked by
`artifacts/api-server/tests/schema.test.ts`.

### Environment variables (API)

| Variable | Default | Purpose |
|----------|---------|---------|
| `PORT` | `8080` | Listen port. Validated; an invalid value stops startup. |
| `HOST` | `0.0.0.0` | Bind address. Use `127.0.0.1` when a reverse proxy is on the same host. |
| `NODE_ENV` | `development` | `production` switches to JSON logs, hides error details, and tightens CORS. |
| `LOG_LEVEL` | `info` | `debug`, `info`, `warn`, `error`. |
| `MAX_BODY` | `1mb` | Request body limit. |
| `CORS_ORIGIN` | *(unset)* | Comma-separated allowed origins, or `*`. Deny-by-default in production; the local Vite dev origins are allowed in development. |
| `OMNICLI_BIN` | *(unset)* | Absolute path to the `omnicli` binary. Authoritative when set — the API will not silently run a different binary. |
| `OMNICLI_ROOT` | *(unset)* | Repository root used to find `omnicli/target/release/omnicli`. Pin it in deployments for deterministic resolution. |
| `OMNI_WORKSPACE_DB` | *(unset)* | Path to the workspace SQLite file. If unset the API asks the CLI where it lives. |
| `DB_PATH` | *(unset)* | Legacy activity database path (Phase-1 installs). |
| `ENV_FILE` | *(unset)* | Explicit `.env` path. |

### Environment variables (dashboard)

| Variable | Default | Purpose |
|----------|---------|---------|
| `PORT` | `3000` | Dev/preview server port (validated, fails fast). |
| `BASE_PATH` | `/` | Base path when the dashboard is served from a subdirectory. |
| `API_PORT` | `8080` | Target of the `/api` proxy used outside Replit. |

The dashboard build is **static**: it has no runtime environment of its own, so
nothing can be misconfigured at run time. It calls `/api` on its own origin, so
serving the built files and the API from one origin needs no CORS at all.

## 3. Run

```bash
# Production API process
NODE_ENV=production \
PORT=8080 \
HOST=127.0.0.1 \
OMNICLI_BIN=/usr/local/bin/omnicli \
OMNI_WORKSPACE_DB=/var/lib/omni/workspace.db \
node artifacts/api-server/dist/index.mjs
```

Serve `artifacts/omni-dashboard/dist/public/` with any web server on the same
origin, and route `/api/*` to the API process. The dashboard is a **local admin
tool**: it must not be exposed to the public internet (see Security below).

Under a process supervisor, use the health endpoints:

| Endpoint | Meaning | Suggested use |
|----------|---------|---------------|
| `GET /api/healthz` | Cheap liveness: the process serves HTTP | Restart policy |
| `GET /api/readyz` | Readiness: opens the workspace database **and** runs `omnicli --version`; `503` when either fails, with a `checks` object naming which one | Traffic routing / deploy gating |

The process handles `SIGTERM` and `SIGINT`: it stops accepting connections, lets
in-flight requests finish (max 5 s), and exits 0.

## 4. Data and backup

| Data | Default location | Notes |
|------|------------------|-------|
| Workspace (notes/todos/snippets/activity) | `$XDG_DATA_HOME/omni/workspace.db` (`~/.local/share/omni/` on Linux, `%LOCALAPPDATA%\omni\` on Windows) | One SQLite file, WAL mode, `busy_timeout` 5000 ms so the CLI and the API can write it from different processes |
| Search index | `…/omni/search.db` | Rebuildable with `omni search index --rebuild <PATH>` |
| Configuration | `~/.config/omni/omnicli.toml` (or `--config`) | Written atomically |

Back up the data directory (`…/omni/`) and the config file. `omni backup create`
verifies what it wrote before reporting success; `omni backup verify` re-checks
an existing archive later.

## 5. Security

- **Do not expose the dashboard publicly.** It is an admin surface for the
  machine it runs on. Put it behind the OS, a VPN, or an authenticating proxy.
- The API sets `X-Content-Type-Options`, `X-Frame-Options: DENY`,
  `Referrer-Policy: no-referrer`, `Cross-Origin-Resource-Policy`, and a
  `default-src 'none'` CSP on every response, and does not send
  `X-Powered-By`.
- CORS is deny-by-default in production; opt in with `CORS_ORIGIN`.
- Requests are correlated by `X-Request-Id` (an inbound id is used only if it is
  id-shaped, otherwise a UUID is generated and echoed).
- **No authentication is built in.** If you need to reach the API from another
  host, terminate TLS and authentication in front of it (reverse proxy), and bind
  it to `127.0.0.1`.
- Logs never contain secrets: pino redacts `authorization` and `cookie` headers,
  and the CLI redacts secret-shaped matches in its own output.

## 6. What has and has not been verified

Verified on Linux x86_64 (see the final report for commands and output):
building all three pieces, starting the API with explicit production-style
environment variables, `/api/healthz`, `/api/readyz`, security headers, request
id propagation, CORS behaviour for allowed and disallowed origins, and the
installer end-to-end against a locally generated release archive.

Not verified here: container images (none are published), systemd/Windows
service units, and macOS/Windows runtime behaviour. Nothing in the repository
claims otherwise.

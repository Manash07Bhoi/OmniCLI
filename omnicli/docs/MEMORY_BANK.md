# OmniCLI — Architecture Memory Bank

> For AI agents and contributors. Captures every non-obvious decision so future work stays consistent without re-deriving rationale from scratch.

---

## Project overview (current state)

OmniCLI is a hybrid Rust + TypeScript project:
- **Rust CLI** (`omnicli/crates/`) — the `omni` binary, Phase 1 complete
- **API Server** (`artifacts/api-server/`) — Express + TypeScript, Phase 1 routes live, Phase 2 stubs active
- **Web Dashboard** (`artifacts/omni-dashboard/`) — React 19 + Vite + Tailwind CSS 4, all panels live
- **Shared libs** (`lib/`) — Drizzle ORM/SQLite (`lib/db`), Zod schemas (`lib/api-zod`), React hooks (`lib/api-client-react`), OpenAPI spec (`lib/api-spec/openapi.yaml`)

Runs on Replit via pnpm workspaces. All three services start automatically.

---

## Crate map and responsibilities

```
omni-core      shared foundation — never depends on other omni-* crates
omni-file      filesystem operations — depends on omni-core, omni-archive
omni-search    FTS5 search engine — depends on omni-core
omni-archive   compression/archiving — depends on omni-core
omni-convert   format conversion codecs — depends on omni-core (via anyhow)
omni-cli       binary + clap CLI + dispatch — depends on all of the above
```

**Rule:** No circular deps. `omni-file` may depend on `omni-archive` (for the `Compress` verb) but `omni-archive` must never depend on `omni-file`.

---

## Dependency decisions

### Hash: BLAKE3 as default, not SHA256

**Decision:** Default hash is BLAKE3 (`blake3` crate). SHA256 and MD5 are provided for compatibility.  
**Why:** BLAKE3 is 3–10× faster than SHA256 on modern hardware, and output is fixed at 256 bits (64 hex chars). Termux devices benefit from the speed.  
**How to apply:** `omni-core::hash::HashAlgo` is the single enum for all three. Config `[file].default_hash` controls the default. The `--algo` CLI flag overrides it.

### age 0.10 encryption API

**Decision:** `age::Encryptor::with_recipients` returns `Option<Encryptor>`, not `Result`. Treat `None` as a domain error.  
**Why:** age 0.10 changed the API; `.unwrap()` on `None` panics at runtime with no message.  
**How to apply:** Always chain `.ok_or_else(|| FileError::Encryption("no recipients".into()))?`.

**Decryption identity:** `Decryptor::new(reader)` returns an enum (`Recipients` or `Passphrase`). Only `Recipients` variant is supported. Match exhaustively and return a descriptive error for `Passphrase`.

### SQLite: bundled feature flag

**Decision:** `rusqlite = { version = "0.32", features = ["bundled"] }` — SQLite compiled in, no system dependency.  
**Why:** Termux and embedded Linux environments may have old or missing `libsqlite3`. Bundled avoids runtime linking issues at the cost of ~600 KB binary size.

### FTS5 with Porter stemmer

**Decision:** `CREATE VIRTUAL TABLE search_fts USING fts5(content, tokenize="porter ascii")`.  
**Why:** Porter stemmer enables searching "encrypt" to find "encryption", "encrypted", etc. Critical for log/code search workflows.

### bzip2 support

**Decision:** `bzip2 = "0.4"` crate added for `.tar.bz2` / `.tbz2` support. Magic bytes `BZh` (0x42 0x5A 0x68).  
**Why:** tar.bz2 is common on older Kali/Parrot packages. Without proper bzip2 decoding, extraction silently reads garbage bytes.

---

## Error handling contract

```
Library crates:  Result<T, CrateError>  (typed thiserror enum)
CLI dispatch:    Result<(), anyhow::Error>  (maps CrateError via anyhow)
main():          if Err → eprintln! + exit(1)
```

**thiserror named-field trap:** `#[from]` on a field named `source` triggers auto-detection of the cause chain in thiserror, changing `Display` output. Rename such fields to `src`/`dest`/`detail` etc. to avoid the trap.

**No silent fallbacks:** Unimplemented Phase-2 features must return `Err(…UnsupportedFormat("…TODO phase-2…"))`, never an `Ok` with wrong data.

---

## CLI grammar decisions

### omni file compress

Thin wrapper over `omni archive create` — exists purely for discoverability ("omni file" is the natural entry point for most users).

```
omni file compress <output.tar.gz> <inputs...>
# == omni archive create <output.tar.gz> <inputs...>
```

Dispatch delegates directly to `omni_archive::create::create_archive`.

### omni search bare query

`omni search "term"` routes through `SearchCmd::External(Vec<OsString>)` (clap `external_subcommand`). The args are joined with spaces to form the query, then dispatched with all defaults (limit=100, no regex, no type filter).  
**Limitation:** bare form does not support `--in` / `--regex` / `--limit`. Use `omni search query "term" --flags` for full control.

### omni file hash — config-driven default algorithm

`FileCmd::Hash.algo` is `Option<String>`. When `None`, the dispatch layer reads `cfg.file.default_hash` (default: `"blake3"`). This means `--algo blake3` and omitting `--algo` produce the same result for the default config, but a user can set `default_hash = "sha256"` and get SHA256 without typing `--algo sha256` every time.

### omni config show

Emits the full resolved config (post-defaults + config file) as TOML (`toml::to_string_pretty`) or JSON (`--json`). The `OmniConfig` struct is `Serialize + Deserialize + Default`, which enables both.

---

## Output system

All print helpers live in `omni-core::output`. They check `cfg.quiet` and `cfg.is_json()` before emitting anything. Progress bars (indicatif) check `omni_core::is_tty()` and fall back to `ProgressBar::hidden()` when not on a TTY — prevents garbage in piped/redirected output.

`OutputMode::Plain` is activated from `config.core.output = "plain"` in `main.rs` (after config load, before dispatch).

---

## Archive convert — directory preservation

**Problem:** `ArchiveCmd::Convert` originally collected only top-level entries from the temp dir (via `read_dir`), losing subdirectory structure.  
**Fix:** Pass `tmp.path()` as a single directory input to `create_archive`. `create_archive` internally uses `WalkDir` to recurse, preserving the full directory tree.

---

## Platform: libc dependency

`omni-core/src/platform.rs` uses `libc::isatty(2)` to detect stderr TTY. This requires `libc = "0.2"` in `omni-core/Cargo.toml`. Do not replace with a pure-Rust approach — `libc` is already a transitive dependency of most crates in the tree and adds no measurable overhead.

---

## Workspace layout rules

- All shared dependency versions live in `[workspace.dependencies]` in `omnicli/Cargo.toml`. Per-crate `Cargo.toml` files use `{ workspace = true }` — never duplicate version numbers.
- `tempfile` is listed in workspace deps and every crate that uses it (including `omni-cli` for integration tests).
- `bzip2 = "0.4"` is a workspace dep; only `omni-archive` includes it.
- Run `cargo build` / `cargo test` from **inside `omnicli/`** — the repo root is a pnpm workspace, not a Cargo workspace.

---

## API server conventions

- **PORT:** Always read from `process.env["PORT"]`. Never hard-code. Replit assigns unique ports per artifact to avoid collisions.
- **Routes:** Registered under `/api` in `app.ts`. Each module gets its own router file in `src/routes/`.
- **Zod schemas:** Imported from `@workspace/api-zod` (generated from OpenAPI spec). Do not hand-write schemas — regenerate with `pnpm generate` from `lib/api-spec/openapi.yaml`.
- **Health check:** `GET /api/healthz` — the only route with no DB dependency. Use this for workflow liveness probes.
- **Build:** `node ./build.mjs` (esbuild) → `dist/index.mjs`. The `dev` script runs build then start; there is no hot-reload in the API server.

---

## Dashboard API calls

The dashboard fetches from `/api/*` (path-relative). Vite proxies `/api` to the API server in development. Do not use `localhost` or absolute URLs in frontend code — they break in the Replit proxy environment.

The dashboard panels that show 500 errors when no data exists yet (backup, workspace with empty DB) are expected — the panels handle empty state gracefully.

---

## Test strategy

| Level | Where | What |
|-------|-------|-------|
| Unit tests | `src/*.rs` inline `#[cfg(test)]` | Pure logic: hash vectors, error conditions, format detection |
| Integration | `crates/omni-cli/tests/integration.rs` | Cross-crate pipelines: hash→compare, CSV→JSON→YAML, archive roundtrips, encrypt→decrypt, search index+query |

**NIST SHA256 test vector:** `hash_bytes(b"", HashAlgo::Sha256)` must equal `e3b0c44298fc1c14...` (empty string). This is verified in `omni-core` unit tests.

---

## Phase 2 status

| Feature | Status |
|---------|--------|
| `omni backup` module (Rust) | Not implemented — returns `Err(NotYetImplemented)` |
| `omni workspace` module (Rust) | Not implemented — returns `Err(NotYetImplemented)` |
| `omni dev` module (Rust) | Not implemented — returns `Err(NotYetImplemented)` |
| `omni backup` API route | Stub active in `artifacts/api-server/src/routes/backup.ts` |
| `omni workspace` API route | Active — notes/todos/snippets in SQLite via Drizzle |
| `omni dev` API route | Active — hash, UUID, Base64, regex via `artifacts/api-server/src/routes/dev.ts` |
| `omni file sync --watch` | Dep present (`notify` crate), implementation pending |
| `ColorsConfig` | Config struct field exists, no effect at runtime |
| `trash_instead_of_delete` | Config field exists, always hard-deletes |
| `.7z` format | Not supported — needs external library or system `7z` |

---

## Extension points

To add a new conversion pair in `omni-convert`:
1. Add a match arm in `codec::convert()`.
2. Add a `FormatPair` entry in `list_supported_pairs()` — must be kept in sync with the match arm (enforced by the `test_list_supported_pairs_complete` integration test).

To add a new archive format:
1. Add a variant to `ArchiveFormat` in `create.rs`.
2. Implement detection in `detect_format_by_magic()` in `extract.rs` and `list.rs`.
3. Add create/extract/list functions.
4. Wire up in `ArchiveCmd` dispatch.

To add a new `omni file` verb:
1. Add variant to `FileCmd` in `cli.rs`.
2. Add a handler arm in `dispatch_file()` in `dispatch.rs`.
3. If the verb needs a new library function, add it to `omni-file/src/` with its own `CrateError` variant.

To add a new API route:
1. Create `artifacts/api-server/src/routes/<name>.ts`
2. Register it in `artifacts/api-server/src/routes/index.ts`
3. Add the path to `lib/api-spec/openapi.yaml`
4. Run `pnpm generate` to regenerate `lib/api-zod` and `lib/api-client-react`

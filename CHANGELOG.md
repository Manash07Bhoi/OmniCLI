# Changelog

All notable changes to OmniCLI are documented here.

This project uses [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Fixed
- **The release pipeline could never publish a tag.** The `verify-version` gate (and the pre-release `version-check` job) grepped the workspace-root `omnicli/Cargo.toml` for `^version`, but the version lives in `crates/omnicli-cli/Cargo.toml`; the grep returned an empty string, so every tag would be refused. Both gates now read the binary-crate manifest (verified locally: extracts `0.1.1`).
- **The weekly security workflow failed every run since 2026-08-31**: `chacha20 0.10.1` was yanked from crates.io and cargo-deny treats yanked crates as advisory failures. Lockfile updated to `chacha20 0.10.2` (`cargo-deny check licenses advisories` now passes).
- **`GET /api/search` without `q` searched for the literal word "undefined"** and returned real results; `GET /api/archive/inspect` without `path` answered `404 "Archive not found: undefined"`. Both now return the structured 400 (same root cause and fix as the earlier `/api/files/hash` case; 4 new tests, total 104).

- **`search --in` was parsed but never applied.** The filter existed in the CLI grammar and the docs, but `search_query` ignored it, so `--in code,logs` returned every type. It now filters both content and filename results by extension category (`files,pdf,code,sqlite,json,logs,zip`; `pdf`/`sqlite`/`zip` match by name — only text extensions get FTS content).
- **`search --regex` never searched content.** Regex mode skipped the full-text stage entirely and only filtered entry *names*, so `search query "TODO|FIXME" --regex` (the documented example) could not find anything inside files. The pattern is now applied to every indexed file's content (with snippet and line number) and to names.
- `omnicli … | head` no longer panics with exit 101 (`SIGPIPE` default disposition restored on Unix; completion output is buffered before writing so a closed stdout cannot panic on any platform).
- Generated completion scripts now complete the name of the installed binary (`omnicli`), not a hard-coded `omni`.
- The API `GET /api/files/hash` returns a structured 400 when `path` is missing instead of `404 File not found: undefined` (the generated `zod.coerce.string()` schema silently coerced the missing value).
- API startup: the schema tripwire message no longer recommends `drizzle-kit push` (a devDependencies-only command) — the schema is created automatically on open, and a missing table now means an unwritable `DB_PATH` or a corrupt file.
- Removed the unused `notify` dependency (no `--watch` flag exists).



### Added — Phase 2

- `omnicli search "<query>"` accepts the same flags as `search query`: `--limit`, `--in`, `--regex`, `--case-sensitive`.
- `omnicli search query --case-sensitive` — exact-case matching for content, filenames, and regex.

**New command groups (real work, no placeholders)**
- `omnicli doctor` — environment and data diagnostics. Checks the binary, data directory (write-probed), workspace DB and search index (`PRAGMA integrity_check`), configuration, required/optional external tools, and PATH installation. Statuses are `PASS`/`WARN`/`FAIL`/`INFO`; a check that cannot run never reports PASS, and any FAIL makes the command exit 1.
- `omnicli report [PATH] [--format terminal|json|markdown]` — one summary of environment, workspace, index, and file statistics. Values are read live; sections that cannot be read report the reason instead of zeros.
- `omnicli security permissions [PATH]` — setuid/setgid and world-writable entries (POSIX only; refuses on other platforms rather than guessing).
- `omnicli security secrets [PATH]` — credential scanner for your own files. Detects AWS/GitHub/Slack/Google/Stripe keys, JWTs and PEM private keys, plus high-entropy values in assignment context. Matches are always redacted in output; placeholder values are ignored.
- `omnicli security config` — audits configuration-file and data-directory permissions.

**One workspace store (CLI ↔ dashboard)**
- The dashboard API now reads and writes the CLI's `workspace.db` (notes/todos/snippets) instead of a second copy in `omni.db`, so `omnicli workspace note list` and the dashboard always agree. Timestamps are stored in the CLI's unit (Unix seconds) and conversions are covered by tests.
- `OMNI_WORKSPACE_DB` overrides the path; otherwise the CLI is asked (`workspace stats --json` now reports `db_path`), falling back to the platform default.
- One-time, non-destructive migration copies previously stored dashboard rows into the shared store (ms → s), records a marker, and never deletes the old database.

**Testing**
- `artifacts/api-server` has an integration test suite (vitest + supertest, 100 tests in 13 files) that boots the real Express app against a temporary sandbox and the real `omnicli` binary: hashing cross-checked with the CLI, archives created by the CLI, search indexing, backups, conversion byte counts, dashboard consistency, workspace CRUD, malformed input, missing-CLI 503, and the store/migration contract.
- New CI job `api-tests` runs that suite on every push.
- New CLI end-to-end tests for `doctor`, `report`, and `security`, including a corrupt-database doctor failure and a secret that must never be printed in full (65 black-box tests over the compiled binary).

**API correctness**
- Duplicate snippet names now return `409` instead of `500`; a malformed `limit` on `/api/dashboard/activity` returns `400`; `/api/files/find` and `/api/files/stats` return `404`/`400` for missing or invalid paths instead of an empty result set; `/api/dev/base64` rejects malformed input instead of returning garbage; unknown API paths return JSON `404`; body-parser failures keep their status (`400` malformed JSON, `413` too large) instead of becoming `500`.
- `GET /api/workspace/stats` and `GET /api/dashboard/stats` read the shared workspace store.

**Release engineering**
- `release.yml` refuses to publish when the tag does not match `omnicli/Cargo.toml`, and re-verifies that the packaged binary reports the tagged version. Added macOS (Intel/Apple Silicon) and Termux (ARM64/ARMv7 static musl) build targets.
- New `OMNI_WORKSPACE_DB`, an optional `.env` loader, clean SIGTERM/SIGINT shutdown, and startup validation that reports exactly which tables are missing if the database is unusable. (Since the schema is now created on open, that message only appears for a database that cannot be written at all.)

- **`GET /api/readyz`** reports whether an instance can actually work (workspace database readable **and** `omnicli` present), returning `503` with a `checks` object naming the failing dependency. `GET /api/healthz` remains the cheap liveness probe.
- **Request correlation**: a caller-supplied `X-Request-Id` is accepted when it is id-shaped and generated otherwise, echoed in every response and attached to every log line.
- **`docs/DEPLOYMENT.md`**: build, configuration (development/testing/staging/production), environment variables, process supervision, data locations and the security posture of an API deployment.
- **Installer end-to-end test** (`omnicli/scripts/test-installer.sh`), run in CI: a real release archive is installed by the real `install.sh`; tampered checksums, a missing checksum file and a path-traversal member are all refused.
- **Build provenance attestation** for release archives (`gh attestation verify`), alongside the existing SHA256SUMS.
- **Rendered accessibility audit** (`artifacts/omni-dashboard/scripts/a11y-audit.mjs`) plus current screenshots in `docs/assets/`.
- **Browser E2E smoke** (`artifacts/omni-dashboard/scripts/e2e-smoke.mjs`): a real Chromium run over the production bundle that checks the dashboard renders the API's real numbers, all routes reach loaded/empty states, the Files page shows entries the API returned, the offline state appears and recovers, and no console errors occur (15 checks).
- `OMNI_RELEASES_URL` / `OMNI_LATEST_API` let the installer use a mirror, a fork, or a local release directory (which is how the test above works).
- `omnicli/scripts/audit-rust.py` checks every crate in `Cargo.lock` against the OSV/RustSec advisory database without needing a local `cargo audit` build.

### Changed — Phase 2
- The activity/backup schema is created when the database is opened, so a production host that installs runtime dependencies only (`pnpm install --prod`) starts without a separate `drizzle-kit push` step. Non-destructive: `CREATE TABLE IF NOT EXISTS` only, verified against the Drizzle definitions by `tests/schema.test.ts`.
- CORS is deny-by-default in production (`CORS_ORIGIN` opt-in) instead of allowing every origin; the local dev servers are still allowed during development.
- Validation failures return `{error, issues[]}` instead of Zod's JSON-encoded issue array inside a string.
- `X-Powered-By` is no longer sent; `pnpm audit` is clean after pinned overrides and isolating the design-only `mockup-sandbox` package from the workspace.
- Accessibility fixes found by the rendered audit: AA-compliant secondary text, zoom no longer disabled, correct heading order, keyboard-reachable scroll regions, and no load-time autofocus stealing focus from the skip link.
- `indicatif` 0.18 (from 0.17) drops the unmaintained `number_prefix` crate, so RUSTSEC-2025-0119 is fixed rather than ignored; only RUSTSEC-2024-0370 (a build-time proc-macro inside `age` 0.10) remains ignored, with the reason recorded in `deny.toml` and `.cargo/audit.toml`.

### Fixed — Phase 2
- **Search: re-indexing no longer duplicates a file's content.** Every `omnicli search index` run appended another content row for each file, so one file could appear several times in `omnicli search` results and the index grew on every run (a 20,000-file corpus reached 80,000 content rows and a 43 MB index after four runs). Content is now replaced, not appended: the corpus stays at 20,000 rows / 23 MB, searches return each file once, and `omnicli doctor` reports `WARN` with the duplicate count plus the `--rebuild` command for indexes written by an older version. Regression tests cover both the indexer and the search results.
- `omnicli workspace stats --json` now includes `db_path`, so consumers (the dashboard) can open the same database instead of re-deriving the default path. A dead helper that computed a *different* default path was removed.
- README: the screenshot previously labelled "Compression & Encryption" is the File Finder panel; the platform matrix now states exactly which targets have actually been executed (only Linux x86_64) rather than claiming runtime support for untested platforms.
- CHANGELOG corrections below: `omnicli file compress` and `sync --delete-extraneous` do not exist in any released binary. The synchronous verb is `omnicli file sync --delete`; compression is `omnicli archive create`. Entries describing them in `[0.1.0]` are annotated.

### Versioning note (read before trusting a tag)
`omnicli --version` reports the version in `omnicli/Cargo.toml`, and the release workflow refuses to publish a tag that disagrees with it. Historically that gate did not exist:
- `v0.1.0` → binary reports `0.1.0` ✅ consistent
- `v0.1.1` → binary reports `0.1.1` ✅ consistent
- `v1.0.0` → binary reports **`0.1.0`** ❌ and uses the older config path `~/.config/omni/omni.toml`. Treat `v1.0.0` as an artifact of an older build mislabelled with a newer tag: it is *behind* `v0.1.1`, not ahead of it. History is not rewritten here; the gate above prevents a recurrence.

### Added
- `omnicli completions <shell>` — generate shell completion scripts for bash, zsh, fish, powershell, and elvish
- `omnicli file find --follow-symlinks` — follow symbolic links with inode-based circular-loop detection; emits `cycle_detected: true` entries instead of looping infinitely
- Real BLAKE3 hashing in the REST API `POST /api/dev/hash` via `@noble/hashes` (pure TypeScript, no native binaries)
- Automatic database migration on first boot — `pnpm run dev` now runs `drizzle-kit push` as a `predev` hook so fresh clones work without manual setup
- Live file count in `omnicli search index` progress display (e.g. `Indexing… 12,345 files  0:00:04`)
- Windows / WSL2 build support — replaced `libc::isatty` with `std::io::IsTerminal` (stable since Rust 1.70); `dirs` crate already handles `USERPROFILE`
- Additional indexed file extensions: `.lock`, `.mod`, `.sum`, `.tf`, `.hcl`, `.cs`, `.swift`, `.kt`, `.scala`, `.hs`
- `.github/ISSUE_TEMPLATE/` — structured bug report and feature request forms
- `.github/PULL_REQUEST_TEMPLATE.md` — PR checklist including Rust + TypeScript + docs gates
- `SECURITY.md` — vulnerability reporting process and security design documentation
- `CODE_OF_CONDUCT.md` — community standards (Contributor Covenant 2.1)
- `omni-search` exclude test for `node_modules` directories

### Changed
- `omni-core/platform.rs` — `is_tty()` now uses `std::io::IsTerminal` (cross-platform, no `unsafe`)
- `omnicli/crates/omni-core/Cargo.toml` — removed `libc` dependency
- `omnicli/docs/USAGE.md` — added installation section, web dashboard panel table, full exit-code table, link back to README
- `README.md` — complete professional rewrite; all phase labels removed; fixed clone URLs; added API route table, architecture diagram, performance notes
- `omnicli/README.md` — clean rewrite aligned with root README
- `omnicli/CONTRIBUTING.md` — updated clone URL, Code of Conduct, architecture rules

### Fixed
- Search index progress bar now shows running file count instead of last-indexed path
- `omnicli file find` symlink entries correctly reported as `file_type: "symlink"` (unchanged, `follow_links: false` by default)

---

## [0.1.0] — 2026-07-19

### Added — Rust CLI (`omnicli`)

**`omnicli file`** (11 verbs)
- `find` — recursive filesystem search with pattern, type, size, and mtime filters; `--regex`, `--long`, `--count`
- `copy` — file/directory copy with optional BLAKE3 `--verify`
- `move` — file/directory rename/move
- `compare` — byte-level or hash-only comparison; exits `0` (identical) / `1` (differ)
- `duplicate` — two-pass (size → BLAKE3) duplicate detection with `--delete-dupes` + `--dry-run`
- `clean` — remove files older than a duration or empty directories; `--dry-run`
- `hash` — BLAKE3 / SHA-256 / MD5 file hashing; `--json` output
- `encrypt` / `decrypt` — age X25519 asymmetric file encryption
- `compress` — shorthand for `omnicli archive create` *(corrected: no released binary exposes a `file compress` verb; use `omnicli archive create`)*
- `sync` — BLAKE3-powered one-way directory sync with `--delete` + `--dry-run` *(corrected: the flag is `--delete`)*
- `stats` — file size, mtime, optional hash

**`omnicli search`**
- `index` — SQLite FTS5 index with Porter stemmer, incremental upsert, `--rebuild`
- `query` — full-text search with content-type filters, regex, case-sensitive, limit
- `info` — show index path and entry count

**`omnicli archive`**
- `create` — zip / tar.gz / tar.xz / tar.bz2 / tar
- `extract` — format detected by magic bytes; zip-slip protected
- `list` — list archive contents without extracting
- `convert` — repack one format into another

**`omnicli convert`** (16 format pairs)
- CSV↔JSON · YAML↔TOML↔JSON · MD→HTML · PDF→TXT · PNG/JPG↔WebP

**`omnicli config`**
- `show` — dump active TOML config (or `--json`)
- `path` — print config file location
- `read` / `get` / `set` / `validate` — multi-format config management (JSON, YAML, TOML, XML, INI)

**`omnicli dev`** (CLI-only verbs)
- `hash` — hash text input with BLAKE3 / SHA-256 / MD5 / SHA-1
- `json` — pretty-print, minify, validate, dot-path query
- `base64` — encode/decode
- `uuid` — generate v4 / v7 UUIDs
- `regex` — test regex patterns; returns matches with index and groups
- `jwt` — decode header+payload; check expiration

**`omnicli backup`**
- `create` — incremental snapshot with BLAKE3 content-hash deduplication
- `restore` — restore from snapshot ID
- `verify` — re-hash all stored objects
- `list` — list snapshots

**`omnicli workspace`**
- Notes, todos, snippets — stored in SQLite

### Added — API Server

- Express + TypeScript REST API on configurable `$PORT`
- Typed contract enforced end-to-end via Zod + Orval-generated client
- Routes: `healthz`, `dashboard/stats`, `dashboard/activity`, `dashboard/module-status`, `files/*`, `search/query`, `convert/run`, `archive/list`, `workspace/*`, `backup/*`, `dev/*`

### Added — Web Dashboard

- React 19 + Vite + Tailwind CSS 4 web dashboard
- Panels: Command Center, Global Search, File Finder, Archive Inspector, Format Converter, Dev Toolkit, Workspace, Backup Ops
- OpenAPI-generated React Query hooks for type-safe data fetching

### Added — CI/CD

- `.github/workflows/rust.yml` — Clippy, test, build on push
- `.github/workflows/typescript.yml` — typecheck + build
- `.github/workflows/security.yml` — weekly cargo-audit + cargo-deny + pnpm audit
- `.github/workflows/pre-release.yml` — version consistency + cross-platform smoke tests
- `.github/workflows/release.yml` — cross-compiled binaries for Kali/ParrotOS x86_64/ARM64, Termux aarch64/armv7, macOS

### Added — Tooling

- `omnicli/deny.toml` — cargo-deny config: allow MIT/Apache-2/BSD/ISC, deny GPL/AGPL/LGPL and `openssl` crate
- Drizzle ORM + SQLite schema for workspace, backup, activity, notes, todos, snippets
- pnpm workspace monorepo with shared TypeScript libraries

[Unreleased]: https://github.com/Manash07Bhoi/OmniCLI/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Manash07Bhoi/OmniCLI/releases/tag/v0.1.0

## [0.1.1] - 2026-08-21
### Fixed
- Addressed stale `.github/workflows/` configurations breaking `v0.1.0` GitHub releases.
- Synchronized canonical artifact naming format to `omnicli-<target_triple>.<ext>` across scripts, CI, and docs.
- Prepared the TUR Termux publishing pipeline with canonical archives.

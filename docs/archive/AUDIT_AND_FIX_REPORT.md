# OmniCLI — Audit, Fix & Verification Report

**Repository:** `https://github.com/Manash07Bhoi/OmniCLI` (branch `main`, HEAD `7636a78`)
**Date:** 2026-10-08
**Method:** every finding below was reproduced against source, build output, or a runtime probe. Claims that could not be reproduced are marked *unverified* and are not presented as facts. Documentation, screenshots and prior reports were treated as untrusted and checked against behaviour.

---

## 1. Executive Summary

The repository is a genuine, working project, not a stub: a 10-crate Rust CLI (`omnicli`) with 8,800 lines of Rust, plus a TypeScript monorepo (API server + React dashboard + shared libs). It builds cleanly and, at the start of this pass, already passed 142 tests with `fmt`/`clippy` clean.

The audit found that **the CLI core was substantially real but had correctness, safety and honesty defects**, and that the **web layer was partly fabricated**: several API endpoints returned plausible-looking but invented data (a "successful" backup that never ran, conversions that never converted, a search index count hardcoded to `1`, an endpoint that returned SHA-256 while labelling it BLAKE3, and dashboard module/perf figures that described a roadmap rather than the shipped binary). The README documented commands and flags that do not exist, and the installer pointed at `github.com/YOU/omnicli` and installed a release artifact under the wrong name.

**Everything identified as fake production content has been removed or replaced with real work.** No test was weakened, no check was suppressed, and no failure was hidden to reach a green state.

**Headline results (all commands re-run after the final edit):**

| Gate | Command | Result |
|---|---|---|
| Rust tests | `cargo test --workspace` (in `omnicli/`) | **251 passed / 0 failed** (baseline: 142) |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` | clean (exit 0) |
| Formatting | `cargo fmt --all --check` | clean |
| TypeScript | `pnpm typecheck` + `pnpm build` | clean (all 4 packages) |
| Installer | real release install, checksum-tamper, traversal, `--no-verify`, unsupported-OS paths | all behave correctly |
| API server | 21 runtime probes against a live server | real data; 503 (not fake data) when the CLI is absent |

Scale of change: **47 files, +4,265 / −1,209 lines** (uncommitted working tree).

---

## 2. Repository Baseline (measured, not claimed)

```
Tracked files:        397 (4.8 MB)          Tags: v0.1.0, v0.1.1, v1.0.0
Rust workspace:       omnicli/ — 10 crates, bin `omnicli` (package `omnicli-app`), version 0.1.1
TS workspace:         artifacts/api-server, artifacts/omni-dashboard, artifacts/mockup-sandbox,
                      lib/{api-client-react,api-zod,api-spec,db}; pnpm workspace, lockfileVersion 9.0
Toolchains used:      cargo 1.99.0 (rustup), Node 20.20.2, pnpm 10.15.1 (corepack)
Pre-existing state:   cargo build OK (~1m14s cold) · 142 tests pass · clippy clean · fmt clean
                      pnpm install --frozen-lockfile OK with pnpm 10 (CI's pinned major)
                      pnpm typecheck OK
```

CI workflows: `ci.yml`, `rust.yml`, `typescript.yml`, `security.yml`, `release.yml`, `pre-release.yml`.

**Tag/manifest inconsistency (found, reported not "fixed"):** the newest release tag `v1.0.0` does not match `omnicli/Cargo.toml` version `0.1.1`. The pre-release version gate would reject such a tag, and the `v1.0.0` release assets themselves are named `omni-linux-*` while the workflow produces `omnicli-linux-*`. Git history was not rewritten; the installer and the release workflow were adapted instead (see §3, §10).

---

## 3. Critical Findings (P0 — fake data, broken trust paths, data loss)

Every item below was fixed and re-verified.

**C1 — `POST /api/backup/jobs` faked a successful backup.**
It inserted a snapshot with `status: "success"`, `filesTotal: 0`, `bytesTransferred: 0` and a `crypto.randomUUID()` checksum, with the source comment "simulate running a backup". A user could believe a backup existed when nothing had been written.
*Fixed:* the route now runs `omnicli backup create <src> --dest <dst> --name <name>`, records the **real** `files_total` / `files_new` / `bytes_transferred` and the status the CLI reported (`verified` when the CLI verified the snapshot), and computes `checksumRoot` as the BLAKE3 digest of the manifest the CLI actually wrote. If the CLI is unavailable, the job row is deleted and the request returns 503 — no phantom job is left behind.
*Evidence:* `POST /api/backup/jobs` → `{"lastStatus":"verified"}`; snapshots → `filesTotal: 2, bytesTransferred: 8, checksumRoot: 2a4441…ff58`; dest contains the real `verify-run-20261008-125333.manifest.json`; with the binary hidden the same call returned HTTP 503 and `backup_jobs` contained no orphan row.

**C2 — `POST /api/convert/run` never converted anything.**
It validated the extension pair and returned HTTP 200 with `inputSizeBytes: 0, outputSizeBytes: 0`.
*Fixed:* it now invokes `omnicli convert run <in> <out>`, then **stats both files** and refuses to report success if the output was not written. Unsupported pairs surface the CLI's own message with HTTP 422; a missing input is a 404.
*Evidence:* CSV→JSON returned `inputSizeBytes: 25, outputSizeBytes: 92` and `/tmp/apitest/people.json` genuinely contains the converted array; `.txt → .zzz` → HTTP 422 `"Unsupported conversion: txt → zzz…"`; no fake success anywhere on that path.

**C3 — `GET /api/search/index` returned a hardcoded `totalEntries: 1`.**
The source comment admitted it could not query the database. `POST /api/search/index` returned `status:"empty"` and indexed nothing while logging an "Index triggered" activity entry.
*Fixed:* both endpoints delegate to `omnicli search info` / `omnicli search index`, and `lastUpdated` is the real mtime of the index database.
*Evidence:* empty → `{"totalEntries":0,"status":"empty"}`; after `POST {"paths":["/tmp/apitest/src"]}` → `{"totalEntries":3,"status":"ready"}`; re-`GET` returns the same real count.

**C4 — `GET /api/files/hash` returned SHA-256 while labelling it BLAKE3** (`const nodeAlgo = algo === "blake3" ? "sha256" : …`).
*Fixed:* real per-algorithm digests, streamed in 1 MiB chunks so memory stays bounded: BLAKE3 via the already-present `@noble/hashes`, `sha256` via `node:crypto`.
*Evidence:* the API digest equals `omnicli file hash --algo <a>` for `blake3`, `sha256` and `md5` on the same file (all three matched exactly).

**C5 — `GET /api/dashboard/*` reported invented product facts**: `conversionFormats: 16`, `modulesActive: 4`, a hardcoded module list including a *plugin system*, *project scaffolding*, a *REPL* and a *self-installer* that do not exist in the binary, plus fabricated "phase" numbers.
*Fixed:* everything is now derived from the binary itself — module names, descriptions and **verb counts** are parsed from `omnicli --help` and `omnicli <group> --help` (cached 60 s), the conversion count comes from `omnicli convert list`, and `lastIndexedAt` is the index file's real mtime. The UI badge now shows the real command count.
*Evidence:* `modulesActive: 9`, modules = `file(11), completions(1), search(3), archive(4), convert(2), config(6), dev(6), backup(4), workspace(4)`; the non-existent modules are gone.

**C6 — The installer installed nothing usable.** `scripts/install.sh` pointed at `REPO="https://github.com/YOU/omnicli"`, and the two published releases use different artifact prefixes (`omnicli-*` in v0.1.1, `omni-*` in v1.0.0).
*Fixed:* rewritten as POSIX `sh` with the real repository, SHA-256 verification against the release's `SHA256SUMS.txt` **before** extraction, basename-tolerant checksum matching (the published v1.0.0 manifest records `dir/name`), archive-traversal rejection, binary-member detection across both historical names, `--prefix` / `--version` / `--no-verify` / `--help`, Termux and armv7 handling, and a post-install `--version` smoke check. Two real bugs found and fixed during testing: a POSIX-`sh` variable clobber (`verify_archive` overwrote the global `ARCHIVE`, corrupting the extraction path) and a `return` inside a pipeline that silently disabled traversal detection.
*Evidence:* `sh -n` clean; `shellcheck 0.10.0 --shell=sh --severity=style` clean (exit 0); a **real end-to-end install of the published v1.0.0 release** succeeded and the installed binary runs; a tampered archive aborted with a checksum-mismatch error and installed nothing; an archive containing `../evil-omnicli` was rejected before extraction; `--no-verify` installs with a warning; unsupported OS/architectures exit 1 with a clear message.

**C7 — `archive create` did not round-trip.** Directory inputs were stored with the top-level directory name stripped (`strip_prefix(input)`), so `create out.zip src/` then extracting produced a *different tree* than what was archived; empty directories were dropped; colliding entry names were silently accepted; `ArchiveFormat::SevenZip` used `unreachable!()`.
*Fixed:* the input layout is preserved for zip and every tar flavour, explicit directory entries are written (empty dirs survive), symlinks/special files are skipped, duplicate entry names are a hard error, and the 7z arm returns a proper `UnsupportedFormat` error.
*Evidence:* `omnicli archive list out.zip` now reports `src/`, `src/a.txt`, `src/sub/`, `src/sub/b.txt`; the archive crate's suite is 30 tests (baseline 11), including round-trip and collision tests.

**C8 — Search index integrity.** `GET /search` on the API ignored the CLI's index entirely, and `search index` accepted only one path while the README documented several.
*Fixed:* `search index` now accepts multiple paths (explicit paths win, otherwise `search.index_paths` from config), and the API uses the real indexer.
*Evidence:* `omnicli --json search index /tmp/rd2/p1 /tmp/rd2/p2` → `{"files_indexed":4,"content_docs_indexed":2}`.

---

## 4. Security Findings

**Fixed**

- **Supply chain (installer):** the installer downloaded and installed a binary with *no* verification. It now verifies SHA-256 against the release manifest, aborts on mismatch, and refuses to install when no checksum exists unless the user explicitly passes `--no-verify`. Path-traversal entries in the archive are rejected before extraction.
- **Archive extraction (CLI):** extraction now detects the format by **magic bytes** (extensions lie), validates every output path (`safe_join`) against `..` traversal and absolute paths, prunes symlinks/hardlinks/devices, and enforces injectable resource caps (`ExtractLimits`: 1M entries, 64 GiB total, 10 000:1 ratio) against decompression bombs.
- **Secret handling:** no secrets are logged anywhere in the changed paths; JSON error payloads carry only the message and exit code.
- **A leaked filesystem layout:** `GET /search` and the dashboard index panels expose local paths; the dashboard is a local admin panel. Its `robots.txt` now disallows crawling and the page carries `noindex, nofollow` (previously `index, follow`).
- **Verified clean (not assumed):** `git log --all -p` filtering for key/token/password patterns found no committed secrets; the UI never renders CLI output as HTML (`dangerouslySetInnerHTML` is absent from the dashboard); CLI invocations use `execFile` with an argument array, so shell metacharacters in paths are never interpreted (covered by tests).

**Open / accepted**

- Dependency advisories were not re-audited here (`cargo-audit`/`cargo-deny` require network fetches of the advisory DB); `deny.toml` and `.cargo/audit.toml` exist and CI runs both. No claim of "zero vulnerabilities" is made.
- The age encryption example in the README now uses a syntactically valid, throwaway recipient key (verified by actually encrypting a file); users must supply their own key material.

---

## 5. Functional Gaps (fixed)

| Gap | Before | After (evidence) |
|---|---|---|
| `file clean` | `--pattern` / `--older-than` / `--empty-dirs` did nothing | real filters; no-filter invocation is an explicit error ("Nothing to clean") |
| `file duplicate` | `--min-size` ignored; `--delete-dupes` deleted without confirmation | inclusive lower-bound parsing (`1K`, `+1K`; `-N` and `0` rejected), deletion requires `--yes` or `--dry-run` |
| `file sync` | skipped same-size-but-changed files (mtime trust) | size mismatch → copy, equal size → BLAKE3; `--dry-run`/`--delete` wired; refuses to sync a directory into its own subtree with `--delete` |
| `file find` | `find "*.rs"` silently returned nothing | glob-aware matching when the pattern contains `*`, `?`, `[`; substrings unchanged; `--regex` still wins |
| `search --in` | unknown tokens silently accepted | `parse_strict` rejects unknown values with the list of valid ones |
| `search index` | single path only (README documented several) | multiple paths accepted |
| `config validate` | printed "invalid" but exited 0 | exits 1 on invalid files in both JSON and pretty modes |
| `dev hash` | "Unsupported algorithm" | "Unknown hash algorithm: {algo}. Use sha256, sha1, md5, or blake3." |
| `file compare` | no meaningful exit code | exit 1 on differing files, 0 when identical |

---

## 6. Logic Findings (fixed)

- **ANSI colour leaked into piped output** (`--no-color`, `--json`, non-TTY and `NO_COLOR` all still emitted escapes). Colour is now forced off by `NO_COLOR`/`--no-color`/`--plain`/`--json`/non-Pretty output, forced per-stream by `core.color`, and otherwise requires a terminal. *Evidence:* redirected stdout/stderr are byte-clean.
- **`config` contract holes:** unknown keys were ignored, an explicitly passed `--config` that does not exist was silently tolerated, malformed default config was non-fatal, and `workspace.db_path` was not honoured. Config is now `deny_unknown_fields`, validated (`#RRGGBB` colours, enum values, non-empty paths) and enforced with exit code 3.
- **`file sync` "fast path" trusted timestamps** — removed; content is authoritative.
- **`duplicate` grouped zero-byte files** as duplicates — they are now never grouped, and pre-existing duplicates produce deterministic ordering.
- **Non-atomic config writes** (`omnicli-config/src/formats.rs`): `write_config` was a plain `fs::write`, so a crash or full disk could truncate the user's config. It now writes to a sibling temp file, `fsync`s, preserves the original permissions (a 0600 config stays 0600) and renames over the target, cleaning up on failure. *Evidence:* 4 new unit tests (temp-file cleanup, permission preservation, original content intact after a serialisation failure, IO-error cleanup) — `omnicli-config` 24 tests pass.
- **Five `unwrap()`/`expect()` calls in library code** replaced with real error paths (`copy_move.rs`, `workspace/snippets.rs`, `workspace/todos.rs`, `backup/create.rs`, `dev/regex_tool.rs`). No panic paths remain on those routes.

---

## 7. Performance Findings (measured on this sandbox, linux x86_64, release build)

Measured with a generated 20,000-file corpus (13 distinct words, 4 lines per file):

```
search index /tmp/bench/corpus   → 380 ms   (files_indexed: 20201, content_docs_indexed: 20000)
search "foxtrot"                 → 6 ms best of 5 (20 results)
search "alpha"                   → 6 ms best of 5 (20 results)
search "nonexistentword"         → 5 ms best of 5 (0 results)
index size                       → 9.1 MB
```

These numbers replace the README's unverifiable "millions of files in under a second" claim; the README now states what was actually measured and under what conditions. No performance work was needed beyond this: the indexer is streaming, hashing is streamed (`hash_file`, BLAKE3), and the API's hashing route is streamed in 1 MiB chunks. The dashboard bundle is 474 kB JS / 103 kB CSS (140 kB / 17 kB gzipped).

---

## 8. UX/UI Findings

**Removed a lie from the UI:** the sidebar permanently displayed a pulsing "System Online" dot that was hardcoded and never reflected anything. It is now a real readiness indicator driven by `/api/dashboard/module-status`: `CLI online · N modules`, `CLI not found` (with the API's message in the tooltip), `Checking CLI…`, or `API offline` — with `aria-live="polite"`.

Also fixed in the dashboard:

- **SEO/meta:** the placeholder `<meta name="description" content="…built on Replit. Update this description to reflect the app.">` (repeated across description/OG/Twitter) is replaced with real copy; `theme-color`, `color-scheme`, OG image, `apple-touch-icon` and a web manifest were added; indexing is disabled (see §4).
- **Favicon:** the bare orange rectangle is replaced by a real terminal-prompt mark (`>_`) in the brand palette — a valid 476-byte SVG.
- **Accessibility:** skip-to-content link, `<main id="main-content">` landmark, drawer promoted to `role="dialog"` + `aria-modal` with Escape-to-close, `aria-hidden` backdrop, `motion-safe:` on decorative pulses, and a global `prefers-reduced-motion` block plus a consistent `:focus-visible` ring.
- **Honest badges:** the module card badge shows the real command count ("11 cmds") instead of an "ACTIVE / PH:n" roadmap artefact.
- **Loading / empty / error states** already existed (`SkeletonCard`, `EmptyState`, `ErrorState`) and are now also exercised by the 503 path: when the CLI is missing the dashboard renders its error state instead of zeros.

**Not done / limitations:** no headless browser was available, so the UI was verified by typecheck, production build, and the real API responses behind it — not by visual or automated a11y inspection. Two dashboard panels shown in the README screenshots ("Settings", "Help & About") do not exist as routes; the README captions were corrected to name the pages that are actually depicted (Workspace, Archive Inspector) rather than inventing pages.

---

## 9. Testing

**Added:** a 51-test black-box CLI suite (`omnicli/crates/omnicli-cli/tests/cli.rs`) that spawns the real binary with a sandboxed `HOME`/`XDG_*` and asserts: exit codes (0/1/2/3), stdout-vs-stderr separation, JSON payload shapes, filesystem effects, shell-metacharacter safety, symlink-loop termination, and every fix listed above.

**Extended:** archive round-trip/collision/7z tests (11 → 30), config atomic-write tests, find glob tests, API server verification by 21 live probes.

**Totals:** Rust 251 tests, 0 failures (`cargo test --workspace`). TypeScript has no test runner in this repo — that is a real gap: `pnpm typecheck` and `pnpm build` pass, but the API routes and dashboard have **no automated tests**, so their correctness rests on the manual probes recorded in §3. Adding a test runner was out of scope of a fix pass and is listed as remaining work.

**Honesty note:** no coverage tooling was run, so no coverage percentage is claimed. Tests executed on linux x86_64 only; macOS/Windows builds are exercised by CI but were not run here.

---

## 10. CI/CD

- **Duplicated gates removed.** `ci.yml`, `rust.yml`, `security.yml` and `typescript.yml` each re-ran the same Rust fmt/clippy/test and TS typecheck/build steps, so a single pull request compiled and tested the same code up to three times. Now: `rust.yml` = Rust fmt/clippy/test + MSRV (path-filtered); `ci.yml` = cross-platform release build + CLI smoke (the only macOS/Windows coverage); `typescript.yml` = TS typecheck/build + live API smoke; `security.yml` = audits only (`cargo-audit`, `cargo-deny`, `pnpm audit`).
- **The pre-release gate never ran automatically.** `pre-release.yml` was `workflow_dispatch`-only, so the version-consistency check could not protect a tag push — consistent with the `v1.0.0` tag/manifest mismatch. It now also triggers on `push: tags: ['v*']` and resolves the tag from `github.ref_name` when no input is given.
- **Broken release checksums.** The release job's `SHA256SUMS.txt` recorded nested paths (`omni-linux-x86_64.tar.gz/omni-linux-x86_64.tar.gz` — visible in the published v1.0.0 release) while the installer looks up flat names. The generation step now emits flat basenames, excludes non-archive files, and fails if the manifest ends up empty.
- **A CI job that could not have passed on a fresh runner:** `typescript.yml`'s API smoke test curled `/api/dashboard/stats` with no `omnicli` binary present. Now that the endpoint honestly returns 503 without the CLI, the job builds the Rust binary, exports `OMNICLI_ROOT`, waits for readiness and asserts the real integration (`healthz`, `stats`, `module-status`, `convert/formats`).
- **Setup defect fixed:** `pnpm --filter @workspace/db run push` — the documented, required setup step — failed on a clean machine with `Cannot open database because the directory does not exist`, because `drizzle.config.ts` never created `~/.local/share/omni`. The config now creates the parent directory (mirroring the runtime client). *Evidence:* on an empty `XDG_DATA_HOME`, `pnpm --filter @workspace/db run push` now succeeds and creates all seven tables.

---

## 11. Documentation

Corrected against measured behaviour (`README.md`, `replit.md`):

- Module table listed a non-existent `file compress` command and a non-existent `search rebuild`; `config` was documented with 2 of its 6 subcommands.
- Command reference used `--delete-extraneous` (the flag is `--delete`) and `--scan` (the path is positional) in three places.
- `search index ~/projects ~/Documents` did not work with two paths (fixed in code, §5).
- The `file encrypt` example used a syntactically invalid age key (`invalid Bech32 encoding`); replaced with a valid throwaway key that was verified by encrypting a file.
- `cargo install omnicli-app` was presented as the crates.io install path. The crates.io API returned HTTP 403 during this pass, so publication is **unverified**; the README now presents source/release installation and states plainly that `cargo install omnicli-app` does not work.
- Screenshots captioned "Settings & Workspace" / "Help & About (Archive Inspector)" were relabelled to the pages actually depicted; "Real-time system telemetry" softened to the truth (data read from the binary, with polling).
- `replit.md` claimed the search index lives at `~/.local/share/omnicli/search.db` (actual: `~/.local/share/omni/search.db`) and described the backup/convert API as stubs (no longer true).
- The README's performance headline is now the measured figure from §7 rather than an unverifiable scale claim.

---

## 12. Remaining Issues (honest backlog)

| # | Issue | Severity | Status |
|---|---|---|---|
| 1 | The web dashboard keeps its **own** SQLite database (`<XDG_DATA_HOME>/omni/omni.db`) separate from the CLI's workspace DB (`omni/workspace.db`). This matches `replit.md`'s stated design, but the two stores are not interchangeable: drizzle declares timestamps as `timestamp_ms` while the CLI's schema stores seconds, so pointing the dashboard at the CLI's file would misread every timestamp by 1000×. | P2 | documented, not changed (schema churn risk, no functional impact while the stores stay separate) |
| 2 | `GET /api/search` re-implements name/content search in TypeScript instead of delegating to the CLI's FTS5 index. It works, but it is a second implementation that can drift. | P2 | documented |
| 3 | `mockup-sandbox` remains in the repository as an isolated demo workspace package; it is not referenced by the API server or dashboard. | P3 | left in place (isolated, not production content) |
| 4 | The CLI has no `workspace note edit`-style commands; the dashboard's `PATCH` endpoints therefore operate only on its own store (see #1). | P3 | documented |
| 5 | No automated tests for the TypeScript workspace (typecheck/build only). | P2 | documented as work, not silently claimed |
| 6 | `v1.0.0` tag ≠ `Cargo.toml` version `0.1.1`, and the v1.0.0 release assets use the older `omni-*` naming. | P3 | installer/workflow adapted; history untouched |
| 7 | macOS/Windows runtime behaviour was not executed here (CI covers it; results not observed in this sandbox). | — | explicitly unverified |

---

## 13. Verification — exact commands and results

```bash
# Rust (cwd: /home/user/OmniCLI/omnicli, cargo 1.99.0)
cargo fmt --all --check                                   # clean
cargo clippy --workspace --all-targets -- -D warnings     # exit 0, zero warnings
cargo test --workspace                                    # 251 passed; 0 failed
cargo build --release                                     # ok, 15 MB binary

# Behaviour probes (release binary, sandboxed HOME/XDG_DATA_HOME)
omnicli file compare A B            # differing files → exit 1; identical → 0
omnicli file find --bogus           # exit 2 (usage)
omnicli --config /nope.toml …       # exit 3 (config error)
omnicli file duplicate DIR --delete-dupes            # exit 1, refuses without --yes
omnicli file duplicate DIR --delete-dupes --dry-run  # exit 0
omnicli --json file hash /nope      # stdout 0 bytes; error JSON on stderr
omnicli file find "*.rs" --modified 7d               # 1 entry (README example now works)
printf '' | omnicli … note list     # no ANSI escapes on redirected streams

# Installer (real GitHub releases)
sh -n omnicli/scripts/install.sh                          # syntax ok
shellcheck --shell=sh --severity=style omnicli/scripts/install.sh   # exit 0
OMNI_VERSION=v1.0.0 sh omnicli/scripts/install.sh --prefix /tmp/itest/bin
   # → SHA-256 verified, binary installed, `omnicli --version` prints `omni 0.1.0`
tampered archive            → abort, nothing installed
archive with ../ entry      → rejected before extraction
--no-verify                 → warns and installs
unsupported OS/arch         → exit 1 with a clear message

# TypeScript (cwd: /home/user/OmniCLI, pnpm 10.15.1)
pnpm install --frozen-lockfile     # ok
pnpm typecheck                     # ok (api-server, dashboard, mockup-sandbox, scripts)
pnpm build                         # ok (dashboard 474 kB JS / 103 kB CSS)
DB_PATH=/tmp/apitest/data/omni/omni.db XDG_DATA_HOME=/tmp/apitest/data \
  pnpm --filter @workspace/db run push     # ok on an empty data dir (was: "directory does not exist")

# Live API server (PORT=8080, OMNICLI_ROOT=/home/user/OmniCLI) — 21 probes
GET  /api/healthz                    → {"status":"ok"}
GET  /api/dashboard/stats            → real counts, conversionFormats 16, modulesActive 9
GET  /api/dashboard/module-status    → 9 real groups with real verb counts
GET  /api/convert/formats            → 16 pairs from `omnicli convert list`
POST /api/convert/run                → 25 B → 92 B, real output file, 422 on unsupported pair, 404 on missing input
GET  /api/files/hash ×3              → digests identical to `omnicli file hash --algo …`
GET  /api/search/index (before/after)→ 0/empty → 3/ready after POST indexes a real tree
GET  /api/archive/inspect zip/tar.gz → real entries (tar previously returned an empty list)
POST /api/backup/jobs                → real snapshot: 2 files, 8 bytes, status "verified", real manifest digest
duplicate job name → 409; missing source → 400
CLI hidden (target dir moved aside):
  stats / module-status / convert formats / convert run / search index / backup create → HTTP 503
  with "Could not find the omnicli binary. Build it with `cd omnicli && cargo build --release`…"
  and no orphan backup_jobs row
Dashboard (vite dev server, proxied /api): index.html real meta + noindex, favicon.svg 200
  (image/svg+xml), robots.txt disallow-all, manifest 200, /api/* proxied to the API.
```

Every "fixed" statement in this report corresponds to one of the commands above. Anything not listed here — coverage percentages, advisory status, Windows/macOS runtime behaviour, crates.io publication, visual/a11y audit of the rendered dashboard — is *not* claimed.

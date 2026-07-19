# Changelog

All notable changes to OmniCLI are documented here.

This project uses [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- `omni completions <shell>` — generate shell completion scripts for bash, zsh, fish, powershell, and elvish
- `omni file find --follow-symlinks` — follow symbolic links with inode-based circular-loop detection; emits `cycle_detected: true` entries instead of looping infinitely
- Real BLAKE3 hashing in the REST API `POST /api/dev/hash` via `@noble/hashes` (pure TypeScript, no native binaries)
- Automatic database migration on first boot — `pnpm run dev` now runs `drizzle-kit push` as a `predev` hook so fresh clones work without manual setup
- Live file count in `omni search index` progress display (e.g. `Indexing… 12,345 files  0:00:04`)
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
- `omni file find` symlink entries correctly reported as `file_type: "symlink"` (unchanged, `follow_links: false` by default)

---

## [0.1.0] — 2026-07-19

### Added — Rust CLI (`omni`)

**`omni file`** (11 verbs)
- `find` — recursive filesystem search with pattern, type, size, and mtime filters; `--regex`, `--long`, `--count`
- `copy` — file/directory copy with optional BLAKE3 `--verify`
- `move` — file/directory rename/move
- `compare` — byte-level or hash-only comparison; exits `0` (identical) / `1` (differ)
- `duplicate` — two-pass (size → BLAKE3) duplicate detection with `--delete-dupes` + `--dry-run`
- `clean` — remove files older than a duration or empty directories; `--dry-run`
- `hash` — BLAKE3 / SHA-256 / MD5 file hashing; `--json` output
- `encrypt` / `decrypt` — age X25519 asymmetric file encryption
- `compress` — shorthand for `omni archive create`
- `sync` — BLAKE3-powered one-way directory sync with `--delete-extraneous` + `--dry-run`
- `stats` — file size, mtime, optional hash

**`omni search`**
- `index` — SQLite FTS5 index with Porter stemmer, incremental upsert, `--rebuild`
- `query` — full-text search with content-type filters, regex, case-sensitive, limit
- `info` — show index path and entry count

**`omni archive`**
- `create` — zip / tar.gz / tar.xz / tar.bz2 / tar
- `extract` — format detected by magic bytes; zip-slip protected
- `list` — list archive contents without extracting
- `convert` — repack one format into another

**`omni convert`** (16 format pairs)
- CSV↔JSON · YAML↔TOML↔JSON · MD→HTML · PDF→TXT · PNG/JPG↔WebP

**`omni config`**
- `show` — dump active TOML config (or `--json`)
- `path` — print config file location
- `read` / `get` / `set` / `validate` — multi-format config management (JSON, YAML, TOML, XML, INI)

**`omni dev`** (CLI-only verbs)
- `hash` — hash text input with BLAKE3 / SHA-256 / MD5 / SHA-1
- `json` — pretty-print, minify, validate, dot-path query
- `base64` — encode/decode
- `uuid` — generate v4 / v7 UUIDs
- `regex` — test regex patterns; returns matches with index and groups
- `jwt` — decode header+payload; check expiration

**`omni backup`**
- `create` — incremental snapshot with BLAKE3 content-hash deduplication
- `restore` — restore from snapshot ID
- `verify` — re-hash all stored objects
- `list` — list snapshots

**`omni workspace`**
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

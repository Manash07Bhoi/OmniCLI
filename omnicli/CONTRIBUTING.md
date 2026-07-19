# Contributing to OmniCLI

Welcome! Before diving in, please read [README.md](../README.md) for the project overview and architecture.

---

## Getting Started

1. Fork the repo and clone: `git clone https://github.com/Manash07Bhoi/OmniCLI`
2. Build: `cd OmniCLI/omnicli && cargo build`
3. Pick a crate from `crates/` — each is self-contained
4. Run tests: `cargo test -p omni-<module>`
5. Check lints: `cargo clippy -p omni-<module> -- -D warnings`
6. Format before committing: `cargo fmt`

Look for issues labelled **`good first issue`** — these are scoped to a single module and verb.

---

## Definition of Done

A contribution is mergeable when:

- All existing tests pass: `cargo test`
- Clippy is clean: `cargo clippy -- -D warnings`
- No `unwrap()` / `expect()` outside `main.rs` or `#[cfg(test)]`
- Every new public function has at least one unit test
- Every new command has `--json` output
- `cargo build --release` succeeds
- `cargo fmt --check` passes

---

## PR Description Format

```
## What
Brief description of the change.

## Tests
What fixture data was used. Which platforms were tested.

## Notes
Any design decisions or tradeoffs worth documenting.
```

---

## Repository Layout

```
OmniCLI/
├── omnicli/                    ← Rust workspace (work here for CLI)
│   ├── crates/                 ← One crate per module
│   ├── tests/fixtures/         ← Real files for integration tests
│   └── docs/
│       ├── USAGE.md            ← Practical command reference with examples
│       └── MEMORY_BANK.md      ← Architecture decisions and gotchas
├── artifacts/
│   ├── api-server/             ← Express + TypeScript REST API
│   └── omni-dashboard/         ← React 19 + Vite dashboard
└── lib/
    ├── api-spec/               ← OpenAPI 3.1 specification
    ├── api-client-react/       ← Generated React Query hooks
    ├── api-zod/                ← Generated Zod schemas
    └── db/                     ← Drizzle ORM + SQLite schema
```

---

## Adding a New CLI Module

1. Create `crates/omni-<name>/` with its own `Cargo.toml` using `{ workspace = true }` deps
2. Add a `CrateError` enum via `thiserror`
3. Wire up a new variant in `omni-cli/src/cli.rs` and `dispatch.rs`
4. Add an API route in `artifacts/api-server/src/routes/<name>.ts`
5. Add a panel in `artifacts/omni-dashboard/src/pages/` (if UI-facing)
6. Update `lib/api-spec/openapi.yaml` and regenerate the client (`pnpm generate`)

## Adding a New Format Pair (`omni convert`)

1. Add a match arm in `omni-convert/src/codec.rs::convert()`
2. Add a `FormatPair` entry in `list_supported_pairs()` — the `test_list_supported_pairs_complete` integration test will catch any mismatch

## Adding a New Archive Format (`omni archive`)

1. Add a variant to `ArchiveFormat` in `create.rs`
2. Implement detection in `detect_format_by_magic()` in `extract.rs` and `list.rs`
3. Add create/extract/list functions and wire up in dispatch

---

## Rust Workspace Notes

- All shared dependency versions live in `[workspace.dependencies]` in `omnicli/Cargo.toml`
- Per-crate `Cargo.toml` files use `{ workspace = true }` — never duplicate version numbers
- Run `cargo build` / `cargo test` from inside `omnicli/`, not from the repo root

## Architecture Rules

1. No function may return mock/hardcoded data — use `Err(...)` for unimplemented paths
2. No `unwrap()` / `expect()` in library code — propagate errors with `?`
3. `omni-core` has zero module dependencies — keep it lean
4. `omni-cli` is the only crate that imports all modules
5. Circular dependencies between crates are forbidden

---

## Code of Conduct

Be respectful, specific, and constructive. Focus on the code, not the person.

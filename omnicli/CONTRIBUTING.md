# Contributing to OmniCLI

Welcome! Before diving in, read both `PRD.md` and `AGENT.md` — they define the product spec and the **no-mock-data rule** that governs every contribution.

## Getting Started

1. Pick a crate from `crates/` — each crate is self-contained
2. Run tests for your crate: `cargo test -p omni-<module>`
3. Check lints: `cargo clippy -p omni-<module> -- -D warnings`
4. Format before committing: `cargo fmt`

Good-first-issue label is reserved for single-module, single-verb tasks.

## Phase Gate

A module is not mergeable unless its phase's exit criteria (PRD §2.2) are met:
- All verbs in the PRD pass integration tests on real fixture data
- `cargo clippy -- -D warnings` is clean
- No `unwrap()`/`expect()` outside `main.rs` or `#[cfg(test)]`

## PR Description Format

```
## What
Brief description of the implemented verb(s).

## Tests
What fixture data was used. Which platforms were tested.

## Assumptions
Any PRD ambiguities resolved per AGENT.md guidelines.
```

## Repository Layout

```
omnicli/
├── crates/          # One crate per module
├── scripts/         # install.sh (Bash, POSIX-compatible)
├── tests/fixtures/  # Real files used in integration tests (no synthetic data)
└── docs/
    ├── USAGE.md         # Practical command reference with examples
    └── MEMORY_BANK.md   # Architecture decisions and gotchas
```

## Adding a New Module (Phase 2+)

1. Create `crates/omni-<name>/` with its own `Cargo.toml` using `{ workspace = true }` deps
2. Add a `CrateError` enum via `thiserror`
3. Wire up a new variant in `omni-cli/src/cli.rs` and `dispatch.rs`
4. Add an API route in `artifacts/api-server/src/routes/<name>.ts`
5. Add a panel in `artifacts/omni-dashboard/src/` (if UI-facing)
6. Update `lib/api-spec/openapi.yaml` and regenerate the client (`pnpm generate`)

## Adding a New Format Pair (`omni convert`)

1. Add a match arm in `omni-convert/src/codec.rs::convert()`
2. Add a `FormatPair` entry in `list_supported_pairs()` — the `test_list_supported_pairs_complete` integration test will catch any mismatch

## Adding a New Archive Format (`omni archive`)

1. Add a variant to `ArchiveFormat` in `create.rs`
2. Implement detection in `detect_format_by_magic()` in `extract.rs` and `list.rs`
3. Add create/extract/list functions
4. Wire up in `ArchiveCmd` dispatch

## Rust Workspace Notes

- All shared dependency versions live in `[workspace.dependencies]` in `omnicli/Cargo.toml`. Per-crate `Cargo.toml` files use `{ workspace = true }` — never duplicate version numbers.
- Run `cargo build` / `cargo test` from inside `omnicli/`, not from the repo root (which is a pnpm workspace, not a Cargo workspace).

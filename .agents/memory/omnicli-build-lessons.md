---
name: OmniCLI build lessons
description: Rust workspace compilation fixes, API quirks, and architecture decisions for the OmniCLI project.
---

# OmniCLI Build Lessons

## Workspace root
The Rust workspace root is `omnicli/`, **not** the repo root. Always `cd omnicli && cargo build`, not `cargo build` from repo root.

## Key API quirks (age 0.10)
- `Encryptor::with_recipients(...)` returns `Option<Encryptor>`, not `Result`. Use `.ok_or_else(|| ...)` to convert.
- `x25519::Identity::to_string()` returns `Secret<String>` — must call `.expose_secret()` in tests.

## thiserror named-field trap
`thiserror` v2 requires named fields in error variants to be referenced with their field name in `#[error("...")]`, not positional. e.g. `#[error("Hash mismatch: src={src} dest={dest}")]` not `{0}`.

## libc dep
`omni-core` needs `libc = "0.2"` in its own `[dependencies]` (not workspace) for the `isatty()` TTY probe. It uses `std::os::unix::io::AsRawFd`.

## tempfile in library deps
`tempfile` must be in `[dependencies]` of each crate (not just `[dev-dependencies]`) because integration tests in the workspace run with all features enabled.

## Dead SevenZip variant
`ArchiveFormat::SevenZip` exists but is never constructed (from_path returns Err for .7z). Suppress with `#[allow(dead_code)]` on the variant.

## Find --type default
CLI default changed from "f" (file only) to "any". `EntryType::parse()` now accepts "any", "a", and "*" in addition to "f"/"file", "d"/"dir", "l"/"symlink".

## Zip-slip protection
`extract_zip` validates all entry names against `..` components and absolute paths via `safe_join()` helper. Tar extraction uses `entry.unpack_in(dest)` which the `tar` crate handles safely.

## config_file_path()
Added `config_file_path()` to `omni-core/src/platform.rs` (uses `dirs::config_dir()`). Previously `dispatch_config` hardcoded `~/.config/omni/omni.toml`. Now uses the platform-aware helper.

## clippy -D warnings pattern
`chrono::Local.timestamp_opt(...).single().unwrap_or_else(Local::now)` — must use `Local::now` (fn pointer), not `|| Local::now()` (redundant closure), to satisfy clippy.

**Why:** `-D warnings` as part of the DoD gate in AGENT.md means any clippy warning is a build failure.

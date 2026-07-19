<div align="center">

```
 ██████╗ ███╗   ███╗███╗   ██╗██╗
██╔═══██╗████╗ ████║████╗  ██║██║
██║   ██║██╔████╔██║██╔██╗ ██║██║
██║   ██║██║╚██╔╝██║██║╚██╗██║██║
╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║
 ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝
```

**One binary. One grammar. Full-stack power.**

[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Clippy](https://img.shields.io/badge/clippy-clean-brightgreen?logo=rust)](#)
[![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Kali%20%7C%20ParrotOS-0d1117?logo=linux&logoColor=white)](#)

*File ops · Full-text search · Format conversion · Archive management · Configuration*

</div>

---

## What is OmniCLI?

OmniCLI (`omni`) is a **professional-grade command-line toolkit** that replaces a scattered collection of utilities with a single, coherent binary. Whether you're managing files on Android via Termux, doing security research on Kali Linux, or writing automation scripts on ParrotOS — `omni` speaks one grammar across every platform.

```bash
omni file find "*.rs" --modified 7d
omni file hash firmware.bin --json | jq .digest
omni search "CVE-2026-1234"
omni convert run data.csv data.json
omni archive create release.tar.gz ./dist
omni file encrypt secrets.toml --recipient age1ql3z7...
```

---

## Modules

| Module | Commands | Highlights |
|--------|----------|------------|
| **`omni file`** | find, copy, move, compare, duplicate, clean, hash, encrypt, decrypt, compress, sync | BLAKE3/SHA256/MD5 · age X25519 encryption · verified copy · BLAKE3-powered sync |
| **`omni search`** | index, query, rebuild | SQLite FTS5 · Porter stemmer · regex · per-type filters · sub-second queries |
| **`omni archive`** | create, extract, list, convert | zip · tar.gz · tar.xz · tar.bz2 · magic-byte detection · zip-slip protected |
| **`omni convert`** | run, list | 16 format pairs: CSV↔JSON · YAML↔TOML↔JSON · MD→HTML · PNG/JPG↔WebP · PDF→TXT |
| **`omni config`** | show, path | Full TOML config with live defaults |

---

## Installation

### Termux (Android)

```bash
pkg update && pkg install rust git
git clone https://github.com/Manash07Bhoi/OmniCLI
cd OmniCLI/omnicli && cargo build --release
cp target/release/omni $PREFIX/bin/
omni --version
```

### Kali Linux / ParrotOS / Debian

```bash
sudo apt install build-essential libssl-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

git clone https://github.com/Manash07Bhoi/OmniCLI
cd OmniCLI/omnicli && cargo build --release
sudo cp target/release/omni /usr/local/bin/
omni --version
```

### Build from source

```bash
cd omnicli

cargo build --release        # optimised binary (~12 MB, bundled SQLite)
cargo test                   # run all tests
cargo clippy -- -D warnings  # must stay clean
```

---

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | | Structured JSON output — no ANSI codes, pipe-friendly |
| `--no-color` | | Disable colour (also honours `NO_COLOR` env var) |
| `--quiet` | `-q` | Suppress non-error output |
| `--verbose` | `-v` | Show debug traces to stderr |
| `--dry-run` | | Show plan without executing (destructive ops only) |
| `--config <PATH>` | | Override `~/.config/omni/omni.toml` |

---

## Full Usage Guide

See **[docs/USAGE.md](docs/USAGE.md)** for the complete command reference with real examples and expected output for every module.

---

## Architecture

```
omnicli/
├── Cargo.toml                  ← Workspace root — all shared dependency versions
└── crates/
    ├── omni-cli/               ← Binary entry-point: clap parse → dispatch
    │   ├── src/cli.rs          ← Full CLI definition (all modules, all verbs)
    │   └── src/dispatch.rs     ← Routes every command to the right module
    ├── omni-core/              ← Shared: hashing, output styling, config, platform
    │   ├── src/hash.rs         ← hash_file(path, algo) → hex string
    │   ├── src/output.rs       ← OutputConfig, print_success/error/info/warn
    │   ├── src/config.rs       ← OmniConfig loaded from ~/.config/omni/omni.toml
    │   └── src/platform.rs     ← expand_tilde, format_bytes, data_dir
    ├── omni-file/              ← File operations (11 verbs)
    ├── omni-search/            ← SQLite FTS5 index + full-text search
    ├── omni-archive/           ← zip/tar/* — zip-slip protected
    ├── omni-convert/           ← 16 format conversion codecs
    └── omni-config/            ← Config format handling
```

**Dependency rules (enforced by crate graph):**
- `omni-core` has zero module dependencies
- All other crates may depend on `omni-core` only
- `omni-file` may depend on `omni-archive` (compress delegates to it)
- `omni-cli` is the only crate that imports all modules

---

## Error Handling

```
Library crate    →  Result<T, CrateError>   (typed, via thiserror)
dispatch.rs      →  anyhow::Error            (human-readable, with context)
main.rs          →  exit(1) + stderr         (never silently swallows)
```

- Each crate defines its own `Error` enum — no stringly-typed errors in library code
- `unwrap()` and `expect()` are banned in library code (allowed only in tests)
- All errors include context: file path, operation, parameters

---

## Security

| Concern | Mitigation |
|---------|-----------|
| **Zip-slip** | Archive extraction rejects `..` path components and absolute entry paths |
| **Encryption** | age X25519 asymmetric encryption via the audited [`age`](https://crates.io/crates/age) crate |
| **Key exposure** | Identity keys via `--identity` are visible in `ps` — use shell substitution in production |
| **BLAKE3 integrity** | `--verify` on copy re-hashes the destination to confirm byte-identical transfer |
| **Dependency auditing** | Weekly `cargo audit` + `cargo deny` via GitHub Actions |

---

## Testing

```bash
# All tests across all crates
cargo test

# Specific crate
cargo test -p omni-file
cargo test -p omni-archive
cargo test -p omni-search
cargo test -p omni-convert

# With stdout
cargo test -- --nocapture
```

- Unit tests for every public function
- Integration tests: encrypt→decrypt roundtrips, archive format roundtrips, CSV→JSON→YAML chain
- No synthetic data — fixtures use real files
- All tests are hermetic: tempdir-isolated, no global state

---

## Performance

- **BLAKE3** is ~3× faster than SHA-256 on modern hardware (AVX2-accelerated)
- **FTS5 queries** return results in under 10 ms on a 100k-file index
- **Archive creation** streams through the encoder — no full in-memory buffer
- **Sync** only copies changed files (BLAKE3 comparison), making reruns cheap
- **Duplicate scan** uses a two-pass strategy: size-group first, hash only collisions

---

## Platform Notes

| Platform | Notes |
|----------|-------|
| **Termux (Android)** | `isatty()` probe works; colour auto-detected; path expansion handles Termux prefix |
| **Kali Linux** | `rusqlite` compiled with bundled SQLite — no system lib required |
| **ParrotOS** | Static SQLite avoids version conflicts |
| **macOS** | Compiles; `libc::isatty` supported via Unix trait |

---

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

**Every PR must pass:**

```bash
cargo clippy -- -D warnings   # zero diagnostics
cargo test                     # all tests green
cargo build --release          # release build succeeds
cargo fmt --check              # formatting clean
```

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Built with ❤️ in Rust · Targets Termux, Kali, and ParrotOS

[Usage Guide](docs/USAGE.md) · [Contributing](CONTRIBUTING.md) · [Full Docs](../README.md)

</div>

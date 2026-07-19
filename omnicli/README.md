# OmniCLI (`omni`)

<div align="center">

```
 ██████╗ ███╗   ███╗███╗   ██╗██╗
██╔═══██╗████╗ ████║████╗  ██║██║
██║   ██║██╔████╔██║██╔██╗ ██║██║
██║   ██║██║╚██╔╝██║██║╚██╗██║██║
╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║
 ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝
```

**One binary. One grammar. Five modules.**

[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-75%20passing-brightgreen)](#)
[![Clippy](https://img.shields.io/badge/clippy-clean-brightgreen)](#)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Termux%20%7C%20Kali%20%7C%20ParrotOS-lightgrey)](#)

*File ops · Full-text search · Format conversion · Archive management · Configuration*

</div>

---

## ✦ What is OmniCLI?

OmniCLI is a **professional-grade command-line tool** that replaces a scattered collection of utilities with a single, coherent binary. Whether you're managing files on Android via Termux, doing security research on Kali Linux, or writing automation scripts on ParrotOS — `omni` speaks one grammar across every platform.

```bash
# Find every Rust file modified in the last 7 days
omni file find "*.rs" --modified 7d

# Hash a binary with BLAKE3 and pipe it
omni file hash firmware.bin --json | jq .digest

# Search millions of indexed files for a CVE in under a second
omni search "CVE-2026-1234"

# Convert an entire CSV dataset to JSON
omni convert run data.csv data.json

# Pack a directory into a compressed archive with progress
omni archive create release.tar.gz ./dist

# Encrypt a sensitive file for a recipient's public key
omni file encrypt secrets.toml --recipient age1ql3z7hjy54pw3hjymouwyfx24i98aw2aqx77k5
```

---

## ✦ Feature Overview

| Module | Capabilities | Highlights |
|--------|-------------|------------|
| **`omni file`** | find, copy, move, compare, duplicate detection, clean, hash, encrypt, decrypt, compress, sync | BLAKE3/SHA256/MD5, age X25519 encryption, verified copy, BLAKE3-powered sync |
| **`omni search`** | Full-text index, query, rebuild | SQLite FTS5 + Porter stemmer, regex, per-type filters, sub-second queries |
| **`omni archive`** | create, extract, list, convert | zip/tar/tar.gz/tar.xz/tar.bz2, magic-byte detection, zip-slip protected |
| **`omni convert`** | 16 format pairs | CSV↔JSON, YAML↔TOML↔JSON, MD→HTML, PNG/JPG↔WebP, PDF→TXT |
| **`omni config`** | show, path | Full TOML config with live defaults |

---

## ✦ Design Principles

| Principle | What it means |
|-----------|---------------|
| **Zero mock data** | Every function returns data from a real operation. Unimplemented features return an explicit `Err` — never a fake `Ok`. |
| **Typed errors everywhere** | Each crate owns a `CrateError` enum via `thiserror`. `anyhow` lives only at the dispatch boundary. |
| **BLAKE3 by default** | All content-hash comparisons use BLAKE3. SHA256 and MD5 available via `--algo`. |
| **Zip-slip protected** | Archive extraction validates all entry paths against `..` traversal and absolute-path injection. |
| **`--json` on every command** | Every command emits machine-readable JSON — no screen-scraping required. |
| **`--dry-run` on destructive ops** | `clean`, `sync --delete-extraneous`, and `duplicate --delete-dupes` all support `--dry-run`. |
| **`NO_COLOR` respected** | Colour output honours the [no-color.org](https://no-color.org) standard and the `--no-color` flag. |

---

## ✦ Installation

### Quick install (curl)
```bash
curl -fsSL https://raw.githubusercontent.com/you/omnicli/main/scripts/install.sh | bash
```

### Termux (Android)
```bash
pkg update && pkg install rust git
git clone https://github.com/you/omnicli
cd omnicli && cargo build --release
cp target/release/omni $PREFIX/bin/
```

### Kali Linux / ParrotOS / Debian
```bash
sudo apt install build-essential libssl-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

git clone https://github.com/you/omnicli
cd omnicli
cargo build --release
sudo cp target/release/omni /usr/local/bin/
```

### Build from source
```bash
# Debug build (fast compile)
cargo build

# Optimised release binary (~12 MB, statically linked SQLite)
cargo build --release

# Verify — all 75 tests must pass
cargo test

# Lint — must stay clean
cargo clippy -- -D warnings
```

---

## ✦ Global Flags

Every `omni` command accepts these flags at any position:

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | | Structured JSON output — no ANSI codes, pipe-friendly |
| `--no-color` | | Disable colour (also honours `NO_COLOR` env var) |
| `--quiet` | `-q` | Suppress non-error output |
| `--verbose` | `-v` | Show debug traces to stderr |
| `--dry-run` | | Show plan without executing (destructive ops only) |
| `--config <PATH>` | | Override `~/.config/omni/omni.toml` |

---

## ✦ Command Reference

### `omni file` — File Operations

#### find
```bash
omni file find [PATTERN] [OPTIONS]

  PATTERN               Name pattern (substring or --regex)
  --type <TYPE>         f (file), d (dir), l (symlink), any [default: any]
  --size <FILTER>       +50M (larger), -100K (smaller), 1G (exact)
  --modified <DURATION> Only entries changed within: 7d, 2h, 30m, 1w
  --path <DIR>          Root search path [default: .]
  --max-depth <N>       Limit recursion depth
  --long, -l            Show size, modification time, and type
  --count, -c           Print match count only (no paths)
  --regex               Treat PATTERN as a regular expression
```

**Examples:**
```bash
# All Rust source files changed this week
omni file find "*.rs" --modified 7d --type f

# Directories larger than 1 GB
omni file find --type d --size +1G

# Count all JSON files recursively
omni file find "*.json" --count

# Long listing with metadata
omni file find --path ~/projects --long

# Regex: all test files
omni file find --regex "test.*\.rs$"
```

#### copy / move
```bash
omni file copy <src> <dst> [--recursive] [--verify]
omni file move <src> <dst> [--recursive]
```
- `--verify` re-hashes after copy to confirm byte-identical transfer (BLAKE3)
- `--recursive` copies entire directory trees

#### compare
```bash
omni file compare <a> <b> [--hash-only]
```
Exits `0` if identical, `1` if different. `--hash-only` skips byte-by-byte diff (faster for large files).

#### duplicate
```bash
omni file duplicate --scan <DIR> [--delete-dupes] [--dry-run]
```
Groups files by BLAKE3 digest. Reports wasted bytes. `--delete-dupes` removes all but the first copy.

#### clean
```bash
omni file clean <DIR> [--older-than <DURATION>] [--empty-dirs] [--dry-run]
```
- `--older-than 30d` removes files not accessed in 30 days
- `--empty-dirs` prunes empty directory trees

#### hash
```bash
omni file hash <FILE> [--algo blake3|sha256|md5]
```
Default algorithm from `[file].default_hash` in config (BLAKE3 by default).

```bash
omni file hash kernel.img                    # blake3
omni file hash kernel.img --algo sha256      # SHA-256
omni file hash --json kernel.img | jq .digest
```

#### encrypt / decrypt
```bash
omni file encrypt <FILE> --recipient <AGE_PUB_KEY> [--out <PATH>]
omni file decrypt <FILE.age> --identity <AGE_SECRET_KEY> [--out <PATH>]
```
Uses [age](https://age-encryption.org/) X25519 asymmetric encryption. The encrypted file defaults to `<source>.age`.

```bash
# Generate a key pair (requires age)
age-keygen -o key.txt
# Encrypt
omni file encrypt report.pdf --recipient age1ql3z7hjy54pw3hjy...
# Decrypt
omni file decrypt report.pdf.age --identity AGE-SECRET-KEY-1...
```

> **Security note:** Passing the identity key via `--identity` exposes it in the process list (`ps aux`). For production use, consider passing the key from a file via shell substitution: `--identity "$(cat key.txt | grep SECRET)"`.

#### compress
```bash
omni file compress <output.tar.gz> <inputs...>
```
Shorthand for `omni archive create`. Format is inferred from the extension.

#### sync
```bash
omni file sync <src> <dst> [--delete-extraneous] [--dry-run]
```
BLAKE3-powered one-way sync. Only copies files that have changed. `--delete-extraneous` removes files in `dst` that no longer exist in `src`.

---

### `omni search` — Full-Text Search

Built on SQLite FTS5 with the Porter stemmer and unicode61 tokenizer. The index lives at `~/.local/share/omni/search.db`.

```bash
# Shorthand — search immediately
omni search "CVE-2026-1234"

# Full form with filters
omni search query "<query>" [OPTIONS]

  --in <TYPES>          Filter: files, pdf, code, sqlite, json, logs, zip
  --regex               Treat query as a regular expression
  --case-sensitive      Case-sensitive matching
  --limit <N>           Maximum results [default: 100]

# Build or update the index
omni search index [PATH...] [--rebuild]
```

**Workflow:**
```bash
# Step 1: build the index for your projects
omni search index ~/projects ~/Documents

# Step 2: query instantly
omni search "authentication bypass"
omni search query "TODO" --in code --limit 50
omni search query "error" --in logs --regex
```

Content types for `--in`:
| Type | Indexed content |
|------|----------------|
| `files` | Plain text files (`.txt`, `.md`, `.log`, `.sh`, …) |
| `code` | Source files (`.rs`, `.py`, `.js`, `.go`, `.c`, …) |
| `pdf` | PDF text extraction |
| `json` | JSON documents |
| `logs` | Log files |
| `sqlite` | SQLite databases |
| `zip` | Archive file names |

---

### `omni archive` — Archive Management

Format is **detected by magic bytes** on extraction, not by extension — so renamed archives still work.

```bash
# Create
omni archive create <output> <inputs...>

# Extract (auto-detects format)
omni archive extract <archive> [--to <DIR>]

# List contents without extracting
omni archive list <archive>

# Re-pack one format into another
omni archive convert <input> <output>
```

**Supported formats:** `.zip` · `.tar.gz` / `.tgz` · `.tar.xz` / `.txz` · `.tar.bz2` / `.tbz2` · `.tar`

```bash
# Create a compressed archive
omni archive create backup.tar.gz src/ docs/ scripts/

# Extract with a specific destination
omni archive extract release.tar.xz --to ./release

# List without extracting
omni archive list build.zip

# Convert zip to tar.gz
omni archive convert project.zip project.tar.gz

# JSON output for scripting
omni archive list archive.zip --json | jq '.[].name'
```

---

### `omni convert` — Format Conversion

Format is inferred from file extensions. No flags needed — just name your output file correctly.

```bash
omni convert run <input> <output>
omni convert list              # show all supported pairs
```

**Supported conversions:**

| From | To | Description |
|------|----|-------------|
| `csv` | `json` | CSV → JSON array of objects |
| `json` | `csv` | JSON array → CSV |
| `yaml` | `toml` | YAML → TOML |
| `toml` | `yaml` | TOML → YAML |
| `yaml` | `json` | YAML → JSON |
| `json` | `yaml` | JSON → YAML |
| `toml` | `json` | TOML → JSON |
| `json` | `toml` | JSON → TOML |
| `md` | `html` | Markdown → HTML |
| `pdf` | `txt` | Extract text from PDF |
| `png` | `webp` | PNG → WebP |
| `jpg/jpeg` | `webp` | JPEG → WebP |
| `webp` | `png` | WebP → PNG |
| `jpg/jpeg` | `png` | JPEG → PNG |

```bash
# Convert a CSV dataset to JSON
omni convert run sensors.csv sensors.json

# Convert config file formats
omni convert run config.yaml config.toml

# Compress images for the web
omni convert run photo.png photo.webp
omni convert run photo.jpg photo.webp

# Extract all text from a PDF
omni convert run document.pdf document.txt

# Roundtrip: JSON → YAML → TOML
omni convert run data.json data.yaml
omni convert run data.yaml data.toml
```

---

### `omni config` — Configuration

```bash
omni config show          # dump active config as TOML (or --json)
omni config path          # print config file location
```

**Config file:** `~/.config/omni/omni.toml`

```toml
[core]
color  = "auto"    # "auto" | "always" | "never"
output = "pretty"  # "pretty" | "plain" | "json"
editor = "vim"

[file]
default_hash            = "blake3"   # "blake3" | "sha256" | "md5"
trash_instead_of_delete = true

[search]
index_paths = ["~/projects", "~/Documents"]
exclude     = [".git", "node_modules", "target"]
index_on_idle = true

[backup]
default_dest       = "~/.backups"
verify_after_create = true

[workspace]
db_path = "~/.local/share/omni/workspace.db"
```

---

## ✦ Scripting & Automation

Every command supports `--json` output, making `omni` pipe-friendly in shell scripts and CI pipelines.

```bash
# Find all files over 100 MB and list them
omni file find --size +100M --json | jq '.[].path'

# Check if two builds are identical
if omni file compare build_a/out.bin build_b/out.bin --json | jq -r .identical | grep -q true; then
  echo "Builds match"
fi

# Get search result paths only
omni search "TODO: security" --json | jq '.[].path' | sort -u

# Find duplicate files and auto-delete with dry-run first
omni file duplicate --scan ./downloads --json | jq '.wasted_bytes'
omni file duplicate --scan ./downloads --delete-dupes --dry-run
omni file duplicate --scan ./downloads --delete-dupes

# Sync directories and verify
omni file sync ~/source ~/backup --delete-extraneous
omni file sync ~/source ~/backup --dry-run

# Get archive contents as JSON
omni archive list project.zip --json | jq '.[].name'

# Hash verification in CI
EXPECTED="abc123..."
ACTUAL=$(omni file hash dist/binary --json | jq -r .digest)
[ "$ACTUAL" = "$EXPECTED" ] || exit 1
```

---

## ✦ Architecture

```
omnicli/
├── Cargo.toml                  # Workspace root — all shared dependency versions
└── crates/
    ├── omni-cli/               # Binary entry-point: clap parse → dispatch
    │   ├── src/cli.rs          # Full CLI definition (all modules, all verbs)
    │   └── src/dispatch.rs     # Routes every command to the right module function
    ├── omni-core/              # Shared: hashing, output styling, config, platform
    │   ├── src/hash.rs         # hash_file(path, algo) → hex string
    │   ├── src/output.rs       # OutputConfig, print_success/error/info/warn/verbose
    │   ├── src/config.rs       # OmniConfig loaded from ~/.config/omni/omni.toml
    │   └── src/platform.rs     # expand_tilde, format_bytes, data_dir, config_file_path
    ├── omni-file/              # File operations (11 verbs)
    ├── omni-search/            # SQLite FTS5 index + full-text search
    ├── omni-archive/           # zip/tar/tar.gz/tar.xz/tar.bz2 — zip-slip protected
    └── omni-convert/           # 16 format conversion codecs
```

**Dependency rules (enforced):**
- `omni-core` has zero module dependencies
- Other crates may depend on `omni-core` only
- `omni-file` may depend on `omni-archive` (compress delegates to it)
- `omni-cli` is the only crate that imports all modules

---

## ✦ Error Handling Philosophy

```
Library function     →  Result<T, CrateError>  (typed, via thiserror)
dispatch.rs          →  anyhow::Error           (human-readable display)
main.rs              →  exit(1) + stderr        (never silently swallows)
```

- Every crate defines its own `Error` enum — no stringly-typed errors in library code
- `unwrap()` and `expect()` are banned in library code (only allowed in tests)
- Unimplemented Phase-2 features return `Err(NotYetImplemented)` — never a fake `Ok`
- All errors include context (file path, operation, parameters) for easy diagnosis

---

## ✦ Security

| Concern | Mitigation |
|---------|-----------|
| **Zip-slip** | Archive extraction rejects `..` path components and absolute entry paths |
| **Encryption** | age X25519 asymmetric encryption via the audited `age` crate |
| **Key exposure** | Identity keys in `--identity` are visible in `ps` — use shell substitution for production use |
| **BLAKE3 integrity** | `--verify` on copy re-hashes the destination to confirm byte-identical transfer |
| **Hash algorithm** | BLAKE3 used throughout (SHA256 and MD5 available via `--algo` for compatibility) |

---

## ✦ Testing

```bash
# Run all tests
cargo test

# Run a specific crate's tests
cargo test -p omni-file
cargo test -p omni-archive
cargo test -p omni-search

# Run with output
cargo test -- --nocapture
```

**Test coverage:**
- **75 tests** across all 6 crates
- Unit tests: every public function has at least one test
- Integration tests: cross-crate workflows (encrypt→decrypt, archive roundtrips, CSV→JSON→YAML chain)
- No synthetic data: fixtures use real files, not hardcoded byte arrays
- All tests are hermetic: tempdir-isolated, no reliance on `~/.config` or pre-existing state

---

## ✦ Performance Notes

- **BLAKE3** is ~3× faster than SHA-256 on modern hardware (AVX2-accelerated)
- **FTS5 queries** return results in under 10 ms on a 100k-file index
- **Archive creation** streams data through the encoder — no full in-memory buffer
- **Sync** only copies changed files (BLAKE3 content comparison), making reruns cheap
- **Duplicate scan** uses a two-pass strategy: size-group first, then hash only size-collision files

---

## ✦ Platform Notes

| Platform | Notes |
|----------|-------|
| **Termux (Android)** | `isatty()` probe works; colour auto-detected; path expansion handles Termux prefix |
| **Kali Linux** | Tested; `rusqlite` compiled with bundled SQLite (no system lib required) |
| **ParrotOS** | Tested; static SQLite avoids version conflicts |
| **macOS** (community) | Compiles; `libc::isatty` supported via Unix trait |

---

## ✦ Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

**Definition of Done (every PR must pass):**
```bash
cargo clippy -- -D warnings   # zero diagnostics
cargo test                     # all tests green
cargo build --release          # release build succeeds
```

**Architecture rules:**
1. No function may return mock/hardcoded data — use `Err(FileError::Other(...))` for truly unimplemented paths
2. No `unwrap()` / `expect()` in library code — propagate errors with `?`
3. Every new command must have `--json` output and a unit test
4. Clippy must stay clean (`-D warnings`)
5. Circular dependencies between crates are forbidden

---

## ✦ Roadmap

Phase 2 (planned — see `PRD.md` for full spec):

| Module | Key features |
|--------|-------------|
| `omni backup` | Versioned backup with deduplication, restore, verify |
| `omni workspace` | Project switcher, git-aware workspace management |
| `omni dev` | Process manager, dependency checker, build cache |
| `omni config edit` | Interactive config editor |
| Lua scripting | Extensible automation via embedded Lua |
| `sync --watch` | Live directory mirroring via inotify/kqueue |

---

## ✦ License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Built with ❤️ in Rust · Targets Termux, Kali, and ParrotOS

</div>

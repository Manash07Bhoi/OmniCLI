# OmniCLI — Usage Guide

> **Full project documentation:** [README.md](../../README.md)
>
> This guide is the practical command reference for the `omnicli` binary. Every command includes real examples and expected output.

---

## Table of Contents

1. [Installation](#installation)
2. [Global flags](#global-flags)
3. [omnicli file](#omnicli-file)
4. [omnicli search](#omnicli-search)
5. [omnicli archive](#omnicli-archive)
6. [omnicli convert](#omnicli-convert)
7. [omnicli config](#omnicli-config)
8. [Scripting & JSON mode](#scripting--json-mode)
9. [Configuration reference](#configuration-reference)
10. [Exit codes](#exit-codes)
11. [Web dashboard panels](#web-dashboard-panels)

---

## Installation

```bash
# Clone
git clone https://github.com/Manash07Bhoi/OmniCLI
cd OmniCLI/omnicli

# Release build (~12 MB, statically linked SQLite)
cargo build --release

# Put on PATH
sudo cp target/release/omnicli /usr/local/bin/    # Linux
cp target/release/omnicli $PREFIX/bin/            # Termux

# Verify
omnicli --version
```

### Windows

#### Installing using pre-built release (Recommended)
1. Download `omnicli-windows.zip` from [GitHub Releases](https://github.com/Manash07Bhoi/OmniCLI/releases).
2. Extract and move `omni.exe` to a folder (e.g., `C:\Program Files\OmniCLI`).
3. Add the folder to your `PATH` environment variable.
4. Verify in PowerShell: `omnicli --version`

#### Building from source (Developer)
```powershell
git clone https://github.com/Manash07Bhoi/OmniCLI
cd OmniCLI\omnicli
cargo build --release
# The binary is located at target\release\omni.exe
```

See [README.md](../../README.md#installation) for platform-specific (Termux, Kali, ParrotOS, Replit) instructions.

---

## Global flags

These flags work on **every** `omnicli` command.

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | | Emit machine-readable JSON to stdout; no ANSI codes |
| `--no-color` | | Disable colour (also respects `NO_COLOR` env var) |
| `--quiet` | `-q` | Suppress all non-error output |
| `--verbose` | `-v` | Debug-level tracing to stderr |
| `--dry-run` | | For destructive ops: print plan, make no changes |
| `--config PATH` | | Override `~/.config/omni/omni.toml` |

---

## omnicli file

### find — locate files by name, type, size, or age

```bash
omnicli file find [PATTERN] [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `PATTERN` | Name pattern (substring or `--regex`) |
| `--type f\|d\|l\|any` | File, directory, symlink, or any [default: any] |
| `--size +50M\|-100K\|1G` | Larger than, smaller than, or exact size |
| `--modified 7d\|2h\|30m` | Changed within the given duration |
| `--path DIR` | Root search path [default: `.`] |
| `--max-depth N` | Limit recursion depth |
| `--long, -l` | Show size, modification time, and type |
| `--count, -c` | Print match count only |
| `--regex` | Treat PATTERN as a regular expression |

**Examples:**
```bash
# All files in the current directory (recursive)
omnicli file find

# Files matching a pattern
omnicli file find "*.rs" --path ~/projects

# Regex pattern
omnicli file find "^test_" --regex --path src/

# Files larger than 50 MB
omnicli file find --size +50M --path /var/log

# Files modified in the last 7 days
omnicli file find --modified 7d --path ~/projects

# Directories only
omnicli file find --type d --path .

# Limit depth to 2 levels
omnicli file find "*.toml" --max-depth 2

# Count all JSON files
omnicli file find "*.json" --count

# Long listing with metadata
omnicli file find --path ~/projects --long
```

Expected output:
```
10
```
(10 crates in `crates/` each carry a `Cargo.toml`; `--count` prints just the number.)

---

### copy — copy files or directories

```bash
omnicli file copy <src> <dst> [--recursive] [--verify] [--dry-run]
```

| Option | Description |
|--------|-------------|
| `--verify` | Re-hash after copy with BLAKE3 to confirm byte-identical transfer |
| `--recursive` | Copy entire directory trees |
| `--dry-run` | Preview without copying |

```bash
# Copy a single file
omnicli file copy report.pdf backup/report.pdf

# Copy with BLAKE3 verification
omnicli file copy firmware.bin /media/usb/firmware.bin --verify

# Copy a directory recursively
omnicli file copy src/ dist/ --recursive

# Dry-run
omnicli file copy src/ dist/ --recursive --dry-run
```

---

### move — rename or move

```bash
omnicli file move <src> <dst> [--verify] [--dry-run]
```

```bash
omnicli file move old_name.txt new_name.txt
omnicli file move report_draft.pdf ~/Documents/final_report.pdf
```

---

### compare — byte-level file comparison

```bash
omnicli file compare <a> <b> [--hash]
```

`--hash` compares by BLAKE3 digest only, skipping the byte-by-byte diff (faster for large files).

```bash
omnicli file compare original.bin patched.bin
omnicli file compare firmware.img backup.img --hash
omnicli file compare a.bin b.bin --json | jq .identical
```

Expected output:
```
[OK] Files are identical.    # exit 0
[WARN] Files differ.         # exit 1
```

---

### duplicate — detect and remove duplicate files

Two-pass: size-group first, then BLAKE3 hash only size-collision candidates.

```bash
omnicli file duplicate [PATH] [--min-size <SIZE>] [--delete-dupes] [--dry-run]
```

```bash
# Scan and report
omnicli file duplicate ~/Downloads

# Dry-run — show what would be deleted
omnicli file duplicate ~/Downloads --delete-dupes --dry-run

# Delete (keeps one copy per group)
omnicli file duplicate ~/Downloads --delete-dupes
```

Expected output:
```
⚠ 3 duplicate group(s) — 2.1 GB wasted
  a3b4c5d6... (512.0 MB)
    /Downloads/movie_copy.mp4
    /Downloads/movie.mp4
```

---

### clean — remove old or empty files

```bash
omnicli file clean <DIR> [--older-than DURATION] [--empty-dirs] [--dry-run]
```

```bash
# Remove files older than 30 days
omnicli file clean /tmp --older-than 30d

# Remove files older than 2 hours
omnicli file clean /var/log/app --older-than 2h

# Also remove empty directories
omnicli file clean ~/cache --older-than 7d --empty-dirs

# Dry-run: preview without deleting
omnicli file clean /tmp --older-than 30d --dry-run
```

---

### hash — compute file checksums

```bash
omnicli file hash <FILE> [--algo blake3|sha256|md5]
```

Default algorithm is read from `[file].default_hash` in config (BLAKE3 by default).

```bash
# BLAKE3 (default)
omnicli file hash firmware.bin

# SHA-256
omnicli file hash firmware.bin --algo sha256

# MD5 (legacy compatibility)
omnicli file hash legacy.iso --algo md5

# JSON output for scripting
omnicli file hash --json firmware.bin | jq -r .digest
```

Expected output:
```
af1349b9f5f9a1a6a0404dea36dadf5...  firmware.bin
algo: blake3  input: 1048576 bytes
```

---

### encrypt / decrypt — age X25519 file encryption

Uses the [age](https://age-encryption.org) specification with X25519 keypairs. The `age` crate is audited.

**Generate a keypair:**
```bash
# Install: cargo install age  (or: apt install age)
age-keygen -o key.txt            # private key in key.txt
cat key.txt | grep "public key"  # copy the AGE1... public key
```

**Encrypt:**
```bash
omnicli file encrypt report.pdf --recipient age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac97
# → creates report.pdf.age

# Custom output path
omnicli file encrypt report.pdf --recipient age1... --output encrypted/report.age
```

**Decrypt:**
```bash
omnicli file decrypt report.pdf.age --identity AGE-SECRET-KEY-1QJEPAXC...
# → creates report.pdf  (strips .age extension)

# Custom output path
omnicli file decrypt report.pdf.age --identity AGE-SECRET-KEY-... --output decrypted/report.pdf
```

> **Security note:** Passing `--identity` on the command line exposes the key in `ps aux`. For production, use shell substitution: `--identity "$(grep SECRET key.txt)"`

---

### compress — create archives (shorthand)

```bash
omnicli file compress <output.tar.gz> <inputs...>
```

Shorthand for `omnicli archive create`. Format is inferred from the extension.

```bash
omnicli file compress backup.tar.gz src/ docs/ scripts/
omnicli file compress archive.zip *.txt
```

---

### sync — content-hash directory sync

BLAKE3-powered one-way sync. Only copies files whose hash has changed.

```bash
omnicli file sync <src> <dst> [--delete] [--dry-run]
```

```bash
# One-way sync (only copies changed files)
omnicli file sync ~/projects /media/usb/projects

# Also delete files in dst that don't exist in src
omnicli file sync ~/projects /media/usb/projects --delete

# Preview only
omnicli file sync ~/projects /media/usb/projects --dry-run
```

Expected output:
```
✓ +12 new, ~3 updated, -0 deleted · 24.5 MB transferred
```

---

## omnicli search

Full-text search powered by SQLite FTS5 with the Porter stemmer and unicode61 tokenizer. Index lives at `~/.local/share/omni/search.db`.

### index — build or update the search index

```bash
omnicli search index [PATH...] [--rebuild]
```

```bash
# Index a directory
omnicli search index ~/projects

# Index multiple directories
omnicli search index ~/projects ~/Documents ~/work

# Rebuild from scratch
omnicli search index ~/projects --rebuild

# Use paths from config [search].index_paths
omnicli search index
```

### query — search the index

```bash
omnicli search "QUERY"                                  # shorthand
omnicli search query "QUERY" [OPTIONS]                  # full form
```

| Option | Description |
|--------|-------------|
| `--in TYPES` | Filter: `files`, `pdf`, `code`, `sqlite`, `json`, `logs`, `zip` |
| `--regex` | Treat query as a regular expression |
| `--case-sensitive` | Case-sensitive matching |
| `--limit N` | Maximum results [default: 100] |

```bash
# Quick lookup
omnicli search "CVE-2026-1234"

# With content-type filter
omnicli search query "buffer overflow" --in code,logs

# Regex across code
omnicli search query "TODO|FIXME|HACK" --regex --in code

# Case-sensitive
omnicli search query "NullPointerException" --case-sensitive

# Limit results
omnicli search query "password" --limit 20 --json | jq '.[].path'
```

**Content types for `--in`:**

| Type | What gets indexed |
|------|------------------|
| `files` | Plain text (`.txt`, `.md`, `.log`, `.sh`, …) |
| `code` | Source files (`.rs`, `.py`, `.js`, `.go`, `.c`, …) |
| `pdf` | PDF text extraction |
| `json` | JSON documents |
| `logs` | Log files |
| `sqlite` | SQLite databases |
| `zip` | Archive entry names |

Expected output:
```
PATH                                          TYPE    SIZE     MATCH
src/parser.rs                                 code    4.2 KB   // CVE-2026-1234: heap use-after-free
docs/advisory.txt                             files   1.1 KB   See CVE-2026-1234 for details.
  2 result(s)
```

---

## omnicli archive

All formats are **detected by magic bytes** on extraction and listing — not by extension — so renamed archives still work.

**Supported formats:** `.zip` · `.tar.gz` / `.tgz` · `.tar.xz` / `.txz` · `.tar.bz2` / `.tbz2` · `.tar`

### create

```bash
omnicli archive create <output> <inputs...>
```

```bash
omnicli archive create backup.tar.gz src/ docs/ README.md
omnicli archive create release.zip dist/
omnicli archive create archive.tar.xz large_dir/      # smaller, slower
omnicli archive create data.tar.bz2 data/
omnicli archive create snapshot.tar src/              # uncompressed
```

### extract

```bash
omnicli archive extract <archive> [--to DIR]
```

```bash
# Extract to auto-named directory (strips extension)
omnicli archive extract backup.tar.gz

# Extract to a specific directory
omnicli archive extract release.zip --to /tmp/release
```

### list

```bash
omnicli archive list <archive> [--json]
```

```bash
omnicli archive list backup.tar.gz
omnicli archive list build.zip --json | jq '.[].name'
```

Expected output:
```
NAME                      SIZE         DIR
src/main.rs               4.1 KB       no
src/lib.rs                2.3 KB       no
docs/README.md            1.8 KB       no
  3 entries
```

### convert

Re-pack one format into another (extract then repack, preserving directory structure):

```bash
omnicli archive convert old.zip new.tar.gz
omnicli archive convert backup.tar.bz2 backup.tar.xz
```

---

## omnicli convert

Format is always inferred from the file **extension**. No extra flags needed.

### list — show supported pairs

```bash
omnicli convert list
```

```
FROM        TO          DESCRIPTION
csv         json        CSV → JSON array of objects
json        csv         JSON array → CSV
yaml        toml        YAML → TOML
toml        yaml        TOML → YAML
yaml        json        YAML → JSON
json        yaml        JSON → YAML
toml        json        TOML → JSON
json        toml        JSON → TOML
png         webp        PNG → WebP image
webp        png         WebP → PNG image
jpg         png         JPEG → PNG image
jpg         webp        JPEG → WebP image
md          html        Markdown → HTML
pdf         txt         Extract text from PDF
  14 supported conversion(s)
```

### run — convert a file

```bash
omnicli convert run <input> <output>
```

```bash
# Data formats
omnicli convert run data.csv data.json
omnicli convert run data.json data.yaml
omnicli convert run data.yaml data.toml

# Config migration
omnicli convert run config.yaml config.toml
omnicli convert run settings.json settings.yaml

# Documents
omnicli convert run README.md README.html
omnicli convert run report.pdf report.txt

# Images
omnicli convert run logo.png logo.webp
omnicli convert run photo.jpg photo.png
omnicli convert run banner.webp banner.png

# Roundtrip: JSON → YAML → TOML → JSON
omnicli convert run data.json data.yaml
omnicli convert run data.yaml data.toml
omnicli convert run data.toml data2.json
```

Expected output:
```
✓ csv → json (2.1 KB)
```

---

## omnicli config

### show — print the active configuration

```bash
omnicli config show                          # TOML (default)
omnicli config show --json                   # JSON
omnicli config show --json | jq .search.index_paths
```

### path — show the config file location

```bash
omnicli config path
# → /home/user/.config/omni/omni.toml
```

---

## Scripting & JSON mode

Every command supports `--json` for machine-readable, pipe-friendly output.

```bash
# Get just the digest
omnicli file hash firmware.bin --json | jq -r .digest

# Check if files are identical in a script
if omnicli file compare a.bin b.bin --json | jq -e .identical > /dev/null; then
  echo "Files match"
fi

# Find large files and process with jq
omnicli file find --size +10M --path /var --json | jq '.[].path'

# Search and get paths only
omnicli search "TODO" --json | jq -r '.[].path' | sort -u

# Archive listing filtered to files only
omnicli archive list backup.tar.gz --json | jq 'map(select(.is_dir == false)) | length'

# Hash verification in CI
EXPECTED="af1349b9..."
ACTUAL=$(omnicli file hash dist/binary --json | jq -r .digest)
[ "$ACTUAL" = "$EXPECTED" ] || { echo "Hash mismatch — aborting"; exit 1; }

# Find duplicates and report wasted space
omnicli file duplicate ~/Downloads --json | jq '.wasted_bytes'

# Sync with JSON progress
omnicli file sync ~/src ~/backup --json | jq '{new: .added, updated: .updated, deleted: .deleted}'

# Get search results as CSV of paths
omnicli search "error" --in logs --json | jq -r '.[].path' | sort | uniq -c | sort -rn
```

---

## Configuration reference

Full annotated `~/.config/omni/omni.toml`:

```toml
[core]
# Colour output: "auto" (detect TTY), "always", "never"
color = "auto"

# Output style: "pretty" (coloured), "plain" (no colour), "json" (machine-readable)
output = "pretty"

# Editor for workspace interactive commands
editor = "vim"

[file]
# Default hash algorithm: "blake3" | "sha256" | "md5"
default_hash = "blake3"

# Move files to trash instead of permanent delete
trash_instead_of_delete = true

[search]
# Paths to index when no argument is given to `omnicli search index`
index_paths = [
  "~/projects",
  "~/Documents",
]

# Directories / filenames to skip during indexing
exclude = [".git", "node_modules", "target", ".venv", "__pycache__"]

# Auto-index when the system is idle (background)
index_on_idle = true

[backup]
# Default backup destination
default_dest = "~/.backups"

# Verify backup integrity after creation
verify_after_create = true

# Compression algorithm: "zstd" | "lz4" | "none"
compression = "zstd"

[workspace]
# SQLite database for notes, todos, and snippets
db_path = "~/.local/share/omni/omni.db"

# Editor opened by interactive workspace commands
editor = "vim"

[colors]
# Per-role colour overrides (ANSI colour names or hex)
success = "bright_green"
error   = "bright_red"
info    = "cyan"
warn    = "yellow"
```

---

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (bad arguments, file not found, permission denied) |
| `1` | `omnicli file compare` — files differ |

All errors are printed to **stderr**. `--json` output always goes to **stdout**.

---

## Web dashboard panels

The dashboard is a powerful web-based UI served by `artifacts/omni-dashboard` that connects to the REST API running at `artifacts/api-server`.

### How to access the dashboard

On **Termux**, **Kali Linux**, or other standard environments, you can start the dashboard using the built-in development server. Make sure you have Node.js and `pnpm` installed.

**Real Example (Termux):**
```bash
# 1. Install Node.js and pnpm if you haven't already
pkg install nodejs
npm install -g pnpm

# 2. Go to the project root
cd OmniCLI

# 3. Install dependencies and set up the database (first run only)
pnpm install
pnpm --filter @workspace/db run push

# 4. Start the dashboard
pnpm run dev
```
*Once it's running, open your mobile browser and navigate to `http://localhost:3000`! You can manage files, run conversions, and use the dev toolkit right from your phone.*

**Real Example (Kali/Debian/macOS):**
```bash
# Navigate to the project root
cd OmniCLI
# Install dependencies
pnpm install
# Push DB schema
pnpm --filter @workspace/db run push
# Start the web server and API
pnpm run dev
```
*Open `http://localhost:3000` in your web browser. The API server runs in the background on port `8080`.*

| Panel | URL path | What it shows |
|-------|----------|--------------|
| **Command Center** | `/` | Real-time telemetry — file counts, storage, module status, activity log |
| **Global Search** | `/search` | Full-text search across indexed files |
| **File Finder** | `/files` | Browse and inspect files via the API |
| **Archive Inspector** | `/archive` | List archive contents without extracting |
| **Format Converter** | `/convert` | Convert files between 16 format pairs |
| **Dev Toolkit** | `/dev` | Hash, Base64, UUID, regex tester, JSON formatter, JWT decoder |
| **Workspace** | `/workspace` | Notes, todos, and snippets — persisted in SQLite |
| **Backup Ops** | `/backup` | Backup jobs with BLAKE3 deduplication tracking |

---

*For architecture details, security notes, and contribution guidelines — see [README.md](../../README.md).*

# OmniCLI — Usage Guide

> **Full project documentation:** [README.md](../../README.md)
>
> This guide is the practical command reference for the `omni` binary. Every command includes real examples and expected output.

---

## Table of Contents

1. [Installation](#installation)
2. [Global flags](#global-flags)
3. [omni file](#omni-file)
4. [omni search](#omni-search)
5. [omni archive](#omni-archive)
6. [omni convert](#omni-convert)
7. [omni config](#omni-config)
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
sudo cp target/release/omni /usr/local/bin/       # Linux
cp target/release/omni $PREFIX/bin/               # Termux

# Verify
omni --version
```

See [README.md](../../README.md#installation) for platform-specific (Termux, Kali, ParrotOS, Replit) instructions.

---

## Global flags

These flags work on **every** `omni` command.

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | | Emit machine-readable JSON to stdout; no ANSI codes |
| `--no-color` | | Disable colour (also respects `NO_COLOR` env var) |
| `--quiet` | `-q` | Suppress all non-error output |
| `--verbose` | `-v` | Debug-level tracing to stderr |
| `--dry-run` | | For destructive ops: print plan, make no changes |
| `--config PATH` | | Override `~/.config/omni/omni.toml` |

---

## omni file

### find — locate files by name, type, size, or age

```bash
omni file find [PATTERN] [OPTIONS]
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
omni file find

# Files matching a pattern
omni file find "*.rs" --path ~/projects

# Regex pattern
omni file find "^test_" --regex --path src/

# Files larger than 50 MB
omni file find --size +50M --path /var/log

# Files modified in the last 7 days
omni file find --modified 7d --path ~/projects

# Directories only
omni file find --type d --path .

# Limit depth to 2 levels
omni file find "*.toml" --max-depth 2

# Count all JSON files
omni file find "*.json" --count

# Long listing with metadata
omni file find --path ~/projects --long
```

Expected output:
```
./Cargo.toml
./crates/omni-core/Cargo.toml
./crates/omni-file/Cargo.toml
  3 entries found
```

---

### copy — copy files or directories

```bash
omni file copy <src> <dst> [--recursive] [--verify] [--dry-run]
```

| Option | Description |
|--------|-------------|
| `--verify` | Re-hash after copy with BLAKE3 to confirm byte-identical transfer |
| `--recursive` | Copy entire directory trees |
| `--dry-run` | Preview without copying |

```bash
# Copy a single file
omni file copy report.pdf backup/report.pdf

# Copy with BLAKE3 verification
omni file copy firmware.bin /media/usb/firmware.bin --verify

# Copy a directory recursively
omni file copy src/ dist/ --recursive

# Dry-run
omni file copy src/ dist/ --recursive --dry-run
```

---

### move — rename or move

```bash
omni file move <src> <dst> [--recursive]
```

```bash
omni file move old_name.txt new_name.txt
omni file move report_draft.pdf ~/Documents/final_report.pdf
```

---

### compare — byte-level file comparison

```bash
omni file compare <a> <b> [--hash-only]
```

`--hash-only` skips byte-by-byte diff (faster for large files, uses BLAKE3).

```bash
omni file compare original.bin patched.bin
omni file compare firmware.img backup.img --hash-only
omni file compare a.bin b.bin --json | jq .identical
```

Expected output:
```
✓ files are identical        # exit 0
✗ files differ               # exit 1
```

---

### duplicate — detect and remove duplicate files

Two-pass: size-group first, then BLAKE3 hash only size-collision candidates.

```bash
omni file duplicate --scan <DIR> [--delete-dupes] [--dry-run]
```

```bash
# Scan and report
omni file duplicate --scan ~/Downloads

# Dry-run — show what would be deleted
omni file duplicate --scan ~/Downloads --delete-dupes --dry-run

# Delete (keeps one copy per group)
omni file duplicate --scan ~/Downloads --delete-dupes
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
omni file clean <DIR> [--older-than DURATION] [--empty-dirs] [--dry-run]
```

```bash
# Remove files older than 30 days
omni file clean /tmp --older-than 30d

# Remove files older than 2 hours
omni file clean /var/log/app --older-than 2h

# Also remove empty directories
omni file clean ~/cache --older-than 7d --empty-dirs

# Dry-run: preview without deleting
omni file clean /tmp --older-than 30d --dry-run
```

---

### hash — compute file checksums

```bash
omni file hash <FILE> [--algo blake3|sha256|md5]
```

Default algorithm is read from `[file].default_hash` in config (BLAKE3 by default).

```bash
# BLAKE3 (default)
omni file hash firmware.bin

# SHA-256
omni file hash firmware.bin --algo sha256

# MD5 (legacy compatibility)
omni file hash legacy.iso --algo md5

# JSON output for scripting
omni file hash --json firmware.bin | jq -r .digest
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
omni file encrypt report.pdf --recipient age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac97
# → creates report.pdf.age

# Custom output path
omni file encrypt report.pdf --recipient age1... --out encrypted/report.age
```

**Decrypt:**
```bash
omni file decrypt report.pdf.age --identity AGE-SECRET-KEY-1QJEPAXC...
# → creates report.pdf  (strips .age extension)

# Custom output path
omni file decrypt report.pdf.age --identity AGE-SECRET-KEY-... --out decrypted/report.pdf
```

> **Security note:** Passing `--identity` on the command line exposes the key in `ps aux`. For production, use shell substitution: `--identity "$(grep SECRET key.txt)"`

---

### compress — create archives (shorthand)

```bash
omni file compress <output.tar.gz> <inputs...>
```

Shorthand for `omni archive create`. Format is inferred from the extension.

```bash
omni file compress backup.tar.gz src/ docs/ scripts/
omni file compress archive.zip *.txt
```

---

### sync — content-hash directory sync

BLAKE3-powered one-way sync. Only copies files whose hash has changed.

```bash
omni file sync <src> <dst> [--delete-extraneous] [--dry-run]
```

```bash
# One-way sync (only copies changed files)
omni file sync ~/projects /media/usb/projects

# Also delete files in dst that don't exist in src
omni file sync ~/projects /media/usb/projects --delete-extraneous

# Preview only
omni file sync ~/projects /media/usb/projects --dry-run
```

Expected output:
```
✓ +12 new, ~3 updated, -0 deleted · 24.5 MB transferred
```

---

## omni search

Full-text search powered by SQLite FTS5 with the Porter stemmer and unicode61 tokenizer. Index lives at `~/.local/share/omni/search.db`.

### index — build or update the search index

```bash
omni search index [PATH...] [--rebuild]
```

```bash
# Index a directory
omni search index ~/projects

# Index multiple directories
omni search index ~/projects ~/Documents ~/work

# Rebuild from scratch
omni search index ~/projects --rebuild

# Use paths from config [search].index_paths
omni search index
```

### query — search the index

```bash
omni search "QUERY"                                  # shorthand
omni search query "QUERY" [OPTIONS]                  # full form
```

| Option | Description |
|--------|-------------|
| `--in TYPES` | Filter: `files`, `pdf`, `code`, `sqlite`, `json`, `logs`, `zip` |
| `--regex` | Treat query as a regular expression |
| `--case-sensitive` | Case-sensitive matching |
| `--limit N` | Maximum results [default: 100] |

```bash
# Quick lookup
omni search "CVE-2026-1234"

# With content-type filter
omni search query "buffer overflow" --in code,logs

# Regex across code
omni search query "TODO|FIXME|HACK" --regex --in code

# Case-sensitive
omni search query "NullPointerException" --case-sensitive

# Limit results
omni search query "password" --limit 20 --json | jq '.[].path'
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

## omni archive

All formats are **detected by magic bytes** on extraction and listing — not by extension — so renamed archives still work.

**Supported formats:** `.zip` · `.tar.gz` / `.tgz` · `.tar.xz` / `.txz` · `.tar.bz2` / `.tbz2` · `.tar`

### create

```bash
omni archive create <output> <inputs...>
```

```bash
omni archive create backup.tar.gz src/ docs/ README.md
omni archive create release.zip dist/
omni archive create archive.tar.xz large_dir/      # smaller, slower
omni archive create data.tar.bz2 data/
omni archive create snapshot.tar src/              # uncompressed
```

### extract

```bash
omni archive extract <archive> [--to DIR]
```

```bash
# Extract to auto-named directory (strips extension)
omni archive extract backup.tar.gz

# Extract to a specific directory
omni archive extract release.zip --to /tmp/release
```

### list

```bash
omni archive list <archive> [--json]
```

```bash
omni archive list backup.tar.gz
omni archive list build.zip --json | jq '.[].name'
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
omni archive convert old.zip new.tar.gz
omni archive convert backup.tar.bz2 backup.tar.xz
```

---

## omni convert

Format is always inferred from the file **extension**. No extra flags needed.

### list — show supported pairs

```bash
omni convert list
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
omni convert run <input> <output>
```

```bash
# Data formats
omni convert run data.csv data.json
omni convert run data.json data.yaml
omni convert run data.yaml data.toml

# Config migration
omni convert run config.yaml config.toml
omni convert run settings.json settings.yaml

# Documents
omni convert run README.md README.html
omni convert run report.pdf report.txt

# Images
omni convert run logo.png logo.webp
omni convert run photo.jpg photo.png
omni convert run banner.webp banner.png

# Roundtrip: JSON → YAML → TOML → JSON
omni convert run data.json data.yaml
omni convert run data.yaml data.toml
omni convert run data.toml data2.json
```

Expected output:
```
✓ csv → json (2.1 KB)
```

---

## omni config

### show — print the active configuration

```bash
omni config show                          # TOML (default)
omni config show --json                   # JSON
omni config show --json | jq .search.index_paths
```

### path — show the config file location

```bash
omni config path
# → /home/user/.config/omni/omni.toml
```

---

## Scripting & JSON mode

Every command supports `--json` for machine-readable, pipe-friendly output.

```bash
# Get just the digest
omni file hash firmware.bin --json | jq -r .digest

# Check if files are identical in a script
if omni file compare a.bin b.bin --json | jq -e .identical > /dev/null; then
  echo "Files match"
fi

# Find large files and process with jq
omni file find --size +10M --path /var --json | jq '.[].path'

# Search and get paths only
omni search "TODO" --json | jq -r '.[].path' | sort -u

# Archive listing filtered to files only
omni archive list backup.tar.gz --json | jq 'map(select(.is_dir == false)) | length'

# Hash verification in CI
EXPECTED="af1349b9..."
ACTUAL=$(omni file hash dist/binary --json | jq -r .digest)
[ "$ACTUAL" = "$EXPECTED" ] || { echo "Hash mismatch — aborting"; exit 1; }

# Find duplicates and report wasted space
omni file duplicate --scan ~/Downloads --json | jq '.wasted_bytes'

# Sync with JSON progress
omni file sync ~/src ~/backup --json | jq '{new: .added, updated: .updated, deleted: .deleted}'

# Get search results as CSV of paths
omni search "error" --in logs --json | jq -r '.[].path' | sort | uniq -c | sort -rn
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
# Paths to index when no argument is given to `omni search index`
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
| `1` | `omni file compare` — files differ |

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

# OmniCLI Usage Guide

A practical reference for every `omni` command with real examples and expected output.

---

## Table of contents

1. [Global flags](#global-flags)
2. [omni file](#omni-file)
3. [omni search](#omni-search)
4. [omni archive](#omni-archive)
5. [omni convert](#omni-convert)
6. [omni config](#omni-config)
7. [Scripting and JSON mode](#scripting-and-json-mode)
8. [Configuration reference](#configuration-reference)

---

## Global flags

These flags work on **every** command.

```
--json        Emit machine-readable JSON to stdout; no ANSI codes.
--no-color    Disable colour (also respects NO_COLOR env var).
-q / --quiet  Suppress all non-error output.
-v / --verbose Debug-level tracing to stderr.
--dry-run     For destructive ops: print plan, make no changes.
--config PATH Override ~/.config/omni/omni.toml.
```

---

## omni file

### find — locate files by name, type, size, or age

```bash
# All files in the current directory (recursive)
omni file find

# Files matching a pattern
omni file find "*.rs" --path ~/projects

# Regex pattern
omni file find "^test_" --regex --path src/

# Files larger than 50 MB
omni file find --size +50M --path /var/log

# Files smaller than 10 KB
omni file find --size -10K

# Files modified in the last 7 days
omni file find --modified 7d --path ~/projects

# Directories only
omni file find --type d --path .

# Limit depth to 2 levels
omni file find "*.toml" --max-depth 2
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
# Copy a single file
omni file copy report.pdf backup/report.pdf

# Copy with verification (BLAKE3 re-hash after copy)
omni file copy firmware.bin /media/usb/firmware.bin --verify

# Copy a directory recursively
omni file copy src/ dist/ --recursive

# Dry-run (preview without copying)
omni file copy src/ dist/ --recursive --dry-run
```

---

### move — rename / move

```bash
omni file move old_name.txt new_name.txt
omni file move report_draft.pdf ~/Documents/final_report.pdf
```

---

### compare — byte-level file comparison

```bash
# Full byte-by-byte comparison
omni file compare original.bin patched.bin

# Hash-only (fast, good for large files)
omni file compare firmware.img backup.img --hash-only
```

Expected output (identical):
```
✓ files are identical
```

Expected output (differ):
```
✗ files differ (first difference at byte 4096)
```
**Exit code**: `0` if identical, `1` if different. Scriptable.

---

### duplicate — detect and remove duplicate files

```bash
# Scan and report (two-pass: size → BLAKE3 hash)
omni file duplicate --scan ~/Downloads

# Also delete duplicates (keeps one copy per group)
omni file duplicate --scan ~/Downloads --delete-dupes

# Dry-run: show what would be deleted
omni file duplicate --scan ~/Downloads --delete-dupes --dry-run
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
# Remove files older than 30 days
omni file clean /tmp --older-than 30d

# Remove files older than 2 hours
omni file clean /var/log/app --older-than 2h

# Also remove empty directories after cleaning
omni file clean ~/cache --older-than 7d --empty-dirs

# Dry-run: preview without deleting
omni file clean /tmp --older-than 30d --dry-run
```

---

### hash — compute file checksums

```bash
# BLAKE3 (default)
omni file hash firmware.bin

# SHA256
omni file hash firmware.bin --algo sha256

# MD5 (legacy compatibility)
omni file hash legacy.iso --algo md5
```

Expected output:
```
af1349b9f5f9a1a6...  firmware.bin
```

The default algorithm is read from `[file].default_hash` in your config file (default: `blake3`).

---

### encrypt / decrypt — age X25519 file encryption

OmniCLI uses [age](https://age-encryption.org) with X25519 keypairs.

**Generate a key pair:**
```bash
# Install age: cargo install age  (or apt install age on Kali/Parrot)
age-keygen -o key.txt            # private key in key.txt
cat key.txt | grep "public key"  # copy the AGE1... public key
```

**Encrypt:**
```bash
omni file encrypt report.pdf --recipient age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac97
# → creates report.pdf.age

omni file encrypt report.pdf --recipient age1... --out encrypted/report.age
```

**Decrypt:**
```bash
omni file decrypt report.pdf.age --identity AGE-SECRET-KEY-1QJEPAXC...
# → creates report.pdf  (strips .age extension)

omni file decrypt report.pdf.age --identity AGE-SECRET-KEY-... --out decrypted/report.pdf
```

---

### compress — create archives (shorthand)

```bash
# Equivalent to `omni archive create`
omni file compress backup.tar.gz src/ docs/ scripts/
omni file compress archive.zip *.txt
```

---

### sync — content-hash directory sync

```bash
# One-way sync src → dest (BLAKE3-based, only copies changed files)
omni file sync ~/projects /media/usb/projects

# Also delete files in dest that don't exist in src
omni file sync ~/projects /media/usb/projects --delete-extraneous

# Preview only
omni file sync ~/projects /media/usb/projects --dry-run
```

Expected output:
```
✓ +12 files, ~3 updated, -0 deleted · 24.5 MB
```

---

## omni search

OmniCLI uses an SQLite FTS5 index with Porter stemmer for full-text search.

### Index files first

```bash
# Index a directory
omni search index ~/projects

# Index multiple directories
omni search index ~/projects ~/Documents ~/work

# Rebuild from scratch (useful after large refactors)
omni search index ~/projects --rebuild

# Use paths from config [search].index_paths (no args needed)
omni search index
```

### Query

```bash
# Bare shorthand — quick lookup
omni search "CVE-2026-1234"

# Full form with options
omni search query "buffer overflow" --in code,logs

# Regex search
omni search query "TODO|FIXME|HACK" --regex --in code

# Case-sensitive
omni search query "NullPointerException" --case-sensitive

# Limit results
omni search query "password" --limit 20

# Filter by content type
omni search query "error" --in logs,json
```

Content types for `--in`: `files`, `pdf`, `code`, `sqlite`, `json`, `logs`, `zip`

Expected output:
```
PATH                                                          TYPE      SIZE        MATCH
src/parser.rs                                                 code      4.2 KB      // CVE-2026-1234: heap use-after-free
docs/advisory.txt                                             files     1.1 KB      See CVE-2026-1234 for details.
  2 result(s)
```

---

## omni archive

All formats are detected by **magic bytes**, not extension (on extraction/listing).

### create

```bash
# Create a .tar.gz from multiple sources
omni archive create backup.tar.gz src/ docs/ README.md

# ZIP archive
omni archive create release.zip dist/

# XZ compression (smaller, slower)
omni archive create archive.tar.xz large_dir/

# BZip2 compression
omni archive create data.tar.bz2 data/

# Uncompressed tar
omni archive create snapshot.tar src/
```

### extract

```bash
# Extract to auto-named directory (strips extension)
omni archive extract backup.tar.gz

# Extract to specific directory
omni archive extract release.zip --to /tmp/release
```

### list

```bash
omni archive list backup.tar.gz
```

Expected output:
```
NAME                                                          SIZE          DIR
src/main.rs                                                   4.1 KB        no
src/lib.rs                                                    2.3 KB        no
docs/README.md                                                1.8 KB        no
  3 entries
```

### convert

Re-package one format into another (extract then repack, preserving directory structure):

```bash
omni archive convert old.zip new.tar.gz
omni archive convert backup.tar.bz2 backup.tar.xz
```

---

## omni convert

Format is always inferred from the file extension.

### list — see supported pairs

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
# CSV → JSON
omni convert run data.csv data.json

# YAML → TOML (config migration)
omni convert run config.yaml config.toml

# Markdown → full HTML page
omni convert run README.md README.html

# Extract text from a PDF
omni convert run report.pdf report.txt

# Image format conversion
omni convert run logo.png logo.webp
omni convert run photo.jpg photo.png
```

Expected output:
```
✓ csv → json (2.1 KB)
```

---

## omni config

### show — print the active configuration

```bash
# TOML format (default)
omni config show

# JSON (great for jq)
omni config show --json | jq .search.index_paths
```

### path — show config file location

```bash
omni config path
# → /home/user/.config/omni/omni.toml
```

---

## Scripting and JSON mode

Every command supports `--json` for machine-readable output:

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

# Archive listing in JSON
omni archive list backup.tar.gz --json | jq 'map(select(.is_dir == false)) | length'
```

**Exit codes:**
- `0` — success
- `1` — general error (bad args, file not found, etc.)
- `1` — `omni file compare` when files differ

---

## Configuration reference

Full annotated `~/.config/omni/omni.toml`:

```toml
[core]
# Colour output: "auto" (detect TTY), "always", "never"
color = "auto"

# Output style: "pretty" (coloured), "plain" (no colour), "json" (machine-readable)
output = "pretty"

# Editor for omni workspace interactive commands (phase 2)
editor = "vim"

[file]
# Default hash algorithm: "blake3" | "sha256" | "md5"
default_hash = "blake3"

# Move files to trash instead of permanent delete (phase 2)
trash_instead_of_delete = true

[search]
# Paths to index when no argument is given to `omni search index`
index_paths = [
  "~/projects",
  "~/Documents",
]
# Directories / filenames to skip during indexing
exclude = [".git", "node_modules", "target", ".venv"]

[backup]
# Default backup destination (phase 2)
default_dest = "~/.backups"
compression  = "zstd"

[workspace]
# Root directory for omni workspace commands (phase 2)
root      = "~/projects"
auto_sync = false

[colors]
# Per-role colour overrides (phase 2)
success = "bright_green"
error   = "bright_red"
```

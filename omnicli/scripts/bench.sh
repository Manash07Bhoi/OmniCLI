#!/usr/bin/env bash
#
# Reproducible performance measurements for OmniCLI.
#
#   ./scripts/bench.sh [BINARY] [CORPUS_DIR]
#
# Defaults: the release binary in this repository, and a generated 20,000-file
# corpus under a temporary directory (fixed random seed, so results are
# comparable between runs and between machines).
#
# It prints wall-clock timings plus the payloads the CLI reports, so a number in
# the README can always be traced back to a command you can run yourself.
set -euo pipefail

BIN="${1:-./target/release/omnicli}"
ROOT="${2:-$(mktemp -d -t omnicli-bench-XXXXXX)}"
CORPUS="$ROOT/corpus"

if [ ! -x "$BIN" ]; then
  echo "error: $BIN is not executable. Build it first: cargo build --release" >&2
  exit 1
fi

export XDG_DATA_HOME="$ROOT/data"
mkdir -p "$CORPUS" "$XDG_DATA_HOME"

echo "binary:  $BIN ($("$BIN" --version))"
echo "corpus:  $CORPUS"
echo "host:    $(uname -sm), $(nproc 2>/dev/null || echo '?') CPUs"

if [ ! -d "$CORPUS/dir000" ]; then
  python3 - "$CORPUS" <<'PY'
import os, random, sys

root = sys.argv[1]
random.seed(7)  # fixed seed: same corpus on every machine
words = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot",
         "golf", "hotel", "index", "token", "search", "needle"]

for i in range(20_000):
    d = os.path.join(root, f"dir{i % 200:03d}", f"sub{i % 20:02d}")
    os.makedirs(d, exist_ok=True)
    body = " ".join(random.choice(words) for _ in range(60))
    if i % 500 == 0:
        body += f" needle-in-haystack-{i}"
    with open(os.path.join(d, f"file{i:05d}.txt"), "w") as fh:
        fh.write(f"# document {i}\n{body}\n")
PY
fi

echo "files:   $(find "$CORPUS" -type f | wc -l)"
echo

echo "── search index (3 runs; run 1 is a fresh index) ──"
for i in 1 2 3; do
  start=$(date +%s%N)
  "$BIN" --json search index "$CORPUS" > "$ROOT/index-$i.json"
  end=$(date +%s%N)
  printf '  run %d: %d ms\n' "$i" $(( (end - start) / 1000000 ))
done
python3 - "$ROOT/index-1.json" <<'PY'
import json, sys
print("  payload:", json.load(open(sys.argv[1])))
PY

echo "── search (best of 5) ──"
for q in needle commonword; do
  best=999999
  for _ in 1 2 3 4 5; do
    start=$(date +%s%N)
    "$BIN" --json search "$q" > "$ROOT/q.json" 2>/dev/null || true
    end=$(date +%s%N)
    ms=$(( (end - start) / 1000000 ))
    [ "$ms" -lt "$best" ] && best=$ms
  done
  printf '  %-12s %d ms (%s hits)\n' "$q" "$best" \
    "$(python3 -c "import json,sys;print(len(json.load(open('$ROOT/q.json'))))" 2>/dev/null || echo '?')"
done

echo "── index state ──"
"$BIN" --json search info
du -h "$XDG_DATA_HOME/omni/search.db" 2>/dev/null | awk '{print "  size on disk: " $1}'
python3 - "$XDG_DATA_HOME/omni/search.db" <<'PY'
import sqlite3, sys
conn = sqlite3.connect(sys.argv[1])
rows = conn.execute("SELECT COUNT(*) FROM search_content_fts").fetchone()[0]
distinct = conn.execute("SELECT COUNT(DISTINCT path) FROM search_content_fts").fetchone()[0]
print(f"  content rows: {rows} for {distinct} paths" + ("  (duplicates!)" if rows != distinct else ""))
PY

echo "── hashing (best of 3, 64 MB file) ──"
SAMPLE="$ROOT/sample.bin"
[ -f "$SAMPLE" ] || head -c 67108864 /dev/urandom > "$SAMPLE"
for algo in blake3 sha256 md5; do
  best=999999
  for _ in 1 2 3; do
    start=$(date +%s%N)
    "$BIN" --json file hash "$SAMPLE" --algo "$algo" > /dev/null
    end=$(date +%s%N)
    ms=$(( (end - start) / 1000000 ))
    [ "$ms" -lt "$best" ] && best=$ms
  done
  printf '  %-7s %d ms\n' "$algo" "$best"
done

echo
echo "artifacts kept under: $ROOT"

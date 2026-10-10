#!/usr/bin/env bash
#
# End-to-end installer test: package the release binary exactly the way
# .github/workflows/release.yml does, publish it to a local HTTP server, and
# install it with the real scripts/install.sh.
#
#   ./scripts/test-installer.sh [BINARY]
#
# Checks:
#   1. a correct release installs, verifies its checksum, and the installed
#      binary runs and reports the expected version
#   2. a tampered archive is rejected (checksum mismatch)
#   3. a missing SHA256SUMS.txt is rejected (no unverified install)
#   4. an archive containing a path-traversal member is rejected before extract
#
# Requires: bash, python3 (local HTTP server), tar, sha256sum.
set -euo pipefail

BIN="${1:-./target/release/omnicli}"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPTS/../.." && pwd)"

for tool in python3 tar sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: $tool is required" >&2; exit 1; }
done
[ -x "$BIN" ] || { echo "error: $BIN is not executable (build it: cargo build --release)" >&2; exit 1; }

VERSION="$("$BIN" --version | awk '{print $NF}')"
TAG="v${VERSION}"
TARGET="linux-x86_64"
ARCHIVE="omnicli-${TARGET}.tar.gz"

WORK="$(mktemp -d -t omnicli-installer-XXXXXX)"
SERVER_PID=""
cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

pass() { printf '  \033[0;32mPASS\033[0m %s\n' "$*"; }
fail() { printf '  \033[0;31mFAIL\033[0m %s\n' "$*"; exit 1; }

# ── Build the release tree in the published layout ────────────────────────────
# Mirrors release.yml: ./omnicli ./README.md ./install.sh inside the tarball.
mkdir -p "$WORK/dist" "$WORK/releases/download/$TAG"
cp "$BIN" "$WORK/dist/omnicli"
cp "$REPO_ROOT/README.md" "$WORK/dist/README.md"
cp "$SCRIPTS/install.sh" "$WORK/dist/install.sh"
tar czf "$WORK/releases/download/$TAG/$ARCHIVE" -C "$WORK/dist" .
( cd "$WORK/releases/download/$TAG" && sha256sum "$ARCHIVE" > SHA256SUMS.txt )

# ── Serve it ─────────────────────────────────────────────────────────────────
python3 -m http.server 8731 --bind 127.0.0.1 --directory "$WORK/releases" >/dev/null 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 50); do
  curl -fsS "http://127.0.0.1:8731/download/$TAG/SHA256SUMS.txt" >/dev/null 2>&1 && break
  sleep 0.1
done

echo "installer test: $TAG ($TARGET), archive $ARCHIVE"
echo "── 1. install from a correct release ──"
OMNI_RELEASES_URL="http://127.0.0.1:8731" \
  sh "$SCRIPTS/install.sh" --prefix "$WORK/opt" --version "$TAG" > "$WORK/install.log" 2>&1 \
  || { sed 's/^/    /' "$WORK/install.log"; fail "installer exited non-zero"; }
grep -q "installed" "$WORK/install.log" || { sed 's/^/    /' "$WORK/install.log"; fail "installer did not confirm installation"; }
[ -x "$WORK/opt/omnicli" ] || fail "binary was not installed into the prefix"
INSTALLED_VERSION="$("$WORK/opt/omnicli" --version | awk '{print $NF}')"
[ "$INSTALLED_VERSION" = "$VERSION" ] || fail "installed binary reports $INSTALLED_VERSION, expected $VERSION"
pass "installed $WORK/opt/omnicli reports $INSTALLED_VERSION"
"$WORK/opt/omnicli" doctor >/dev/null 2>&1 && pass "installed binary runs a real command (doctor)" || fail "installed binary failed to run doctor"

echo "── 2. tampered archive is rejected ──"
# Flip one byte of the archive after the checksum file was written.
python3 - "$WORK/releases/download/$TAG/$ARCHIVE" <<'PY'
import sys
p = sys.argv[1]
data = bytearray(open(p, "rb").read())
data[len(data)//2] ^= 0xFF
open(p, "wb").write(data)
PY
if OMNI_RELEASES_URL="http://127.0.0.1:8731" \
   sh "$SCRIPTS/install.sh" --prefix "$WORK/opt2" --version "$TAG" > "$WORK/tamper.log" 2>&1; then
  fail "installer accepted an archive whose checksum does not match"
fi
grep -qi "checksum\|mismatch" "$WORK/tamper.log" || { sed 's/^/    /' "$WORK/tamper.log"; fail "rejection did not mention the checksum"; }
[ -e "$WORK/opt2/omnicli" ] && fail "tampered install still wrote a binary"
pass "rejected: $(grep -i 'checksum\|mismatch' "$WORK/tamper.log" | head -1 | sed 's/^\[omnicli\] //')"

echo "── 3. missing checksum file is rejected ──"
rm -f "$WORK/releases/download/$TAG/SHA256SUMS.txt"
if OMNI_RELEASES_URL="http://127.0.0.1:8731" \
   sh "$SCRIPTS/install.sh" --prefix "$WORK/opt3" --version "$TAG" > "$WORK/nosum.log" 2>&1; then
  fail "installer installed without a checksum file"
fi
grep -qi "refusing to install unverified" "$WORK/nosum.log" || { sed 's/^/    /' "$WORK/nosum.log"; fail "unexpected failure message"; }
pass "rejected: no SHA256SUMS.txt"

echo "── 4. path-traversal member is rejected before extraction ──"
python3 - "$WORK/dist" "$WORK/evil" <<'PY'
import os, sys, tarfile
src, out = sys.argv[1], sys.argv[2]
with tarfile.open(out, "w:gz") as tf:
    tf.add(os.path.join(src, "omnicli"), arcname="omnicli")
    payload = b"# not part of a release\n"
    info = tarfile.TarInfo("../escaped.txt")
    info.size = len(payload)
    import io as _io
    tf.addfile(info, _io.BytesIO(payload))
PY
cp "$WORK/evil" "$WORK/releases/download/$TAG/$ARCHIVE"
( cd "$WORK/releases/download/$TAG" && sha256sum "$ARCHIVE" > SHA256SUMS.txt )
if OMNI_RELEASES_URL="http://127.0.0.1:8731" \
   sh "$SCRIPTS/install.sh" --prefix "$WORK/opt4" --version "$TAG" > "$WORK/evil.log" 2>&1; then
  fail "installer accepted an archive with a traversal member"
fi
grep -qi "escapes the extraction directory" "$WORK/evil.log" || { sed 's/^/    /' "$WORK/evil.log"; fail "unexpected failure message"; }
[ -e "$WORK/escaped.txt" ] && fail "traversal member was written outside the extraction directory"
pass "rejected: traversal member"

echo
echo "installer test: all 4 scenarios behaved correctly"

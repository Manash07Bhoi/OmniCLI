#!/bin/sh
# OmniCLI installer — POSIX sh, no bash required.
#
# Supports: Linux (x86_64 / aarch64 / armv7), macOS (x86_64 / arm64),
#           Termux (Android aarch64 / armv7).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Manash07Bhoi/OmniCLI/main/omnicli/scripts/install.sh | sh
#   sh omnicli/scripts/install.sh [--prefix DIR] [--version vX.Y.Z] [--no-verify]
#
# Environment:
#   OMNI_VERSION        pin a release tag (e.g. v1.0.0); default "latest"
#   OMNI_RELEASES_URL   release download base URL; default the GitHub repo's
#                       /releases. Set it to install from a mirror or a fork.
#   OMNI_LATEST_API     JSON endpoint used to resolve "latest"; default the
#                       GitHub API for this repo.
#   NO_COLOR            disable coloured installer output
#
# Security: the downloaded archive is verified against the release's
# SHA256SUMS.txt before anything is extracted or installed. Verification failure
# aborts the install. Use --no-verify only if the release carries no checksums.

set -eu

REPO="https://github.com/Manash07Bhoi/OmniCLI"
# Both overrides exist so the installer can be exercised end to end against a
# local release directory (see scripts/test-installer.sh) and so a mirror or a
# fork can be installed from without editing this script.
RELEASES_URL="${OMNI_RELEASES_URL:-${REPO}/releases}"
LATEST_API="${OMNI_LATEST_API:-https://api.github.com/repos/Manash07Bhoi/OmniCLI/releases/latest}"
VERSION="${OMNI_VERSION:-latest}"
BINARY_NAME="omnicli"
VERIFY=1

# ── Output helpers ────────────────────────────────────────────────────────────
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'
  BOLD='\033[1m'; RESET='\033[0m'
else
  RED=''; GREEN=''; CYAN=''; BOLD=''; RESET=''
fi

info() { printf '%b%s%b\n' "${CYAN}"  "[omnicli] $*" "${RESET}"; }
ok()   { printf '%b%s%b\n' "${GREEN}" "[omnicli] $*" "${RESET}"; }
warn() { printf '%b%s%b\n' "${RED}"   "[omnicli] WARNING: $*" "${RESET}" >&2; }
die()  { printf '%b%s%b\n' "${RED}"   "[omnicli] ERROR: $*" "${RESET}" >&2; exit 1; }

usage() {
  cat <<'EOF'
omnicli installer

Options:
  --prefix DIR     install the binary into DIR (default: /usr/local/bin for
                   root, ~/.local/bin otherwise, $PREFIX/bin under Termux)
  --version TAG    install a specific release tag (e.g. --version v1.0.0)
  --no-verify      skip SHA-256 verification of the downloaded archive
  -h, --help       show this help

Environment:
  OMNI_VERSION        release tag to install (default: latest)
  OMNI_RELEASES_URL   release download base (default: this project on GitHub)
  OMNI_LATEST_API     endpoint used to resolve "latest"
  NO_COLOR            disable coloured output
EOF
}

# ── Detect OS and architecture ────────────────────────────────────────────────
detect_target() {
  _d_os="$(uname -s)"
  _d_arch="$(uname -m)"

  # Termux (Android)
  if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ]; then
    case "${_d_arch}" in
      aarch64|arm64) echo "linux-aarch64" ;;
      armv7*|armv8l) echo "linux-armv7"   ;;
      *) die "Unsupported Termux architecture: ${_d_arch}" ;;
    esac
    return
  fi

  case "${_d_os}" in
    Linux)
      case "${_d_arch}" in
        x86_64|amd64)  echo "linux-x86_64"  ;;
        aarch64|arm64) echo "linux-aarch64" ;;
        armv7*|armv6*) echo "linux-armv7"   ;;
        *) die "Unsupported Linux architecture: ${_d_arch}" ;;
      esac ;;
    Darwin)
      case "${_d_arch}" in
        x86_64) echo "macos-x86_64"  ;;
        arm64)  echo "macos-aarch64" ;;
        *) die "Unsupported macOS architecture: ${_d_arch}" ;;
      esac ;;
    *) die "Unsupported OS: ${_d_os}" ;;
  esac
}

# ── Install location ──────────────────────────────────────────────────────────
detect_prefix() {
  if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ]; then
    echo "${PREFIX:-/data/data/com.termux/files/usr}/bin"
  elif [ "$(id -u)" -eq 0 ]; then
    echo "/usr/local/bin"
  else
    echo "${HOME}/.local/bin"
  fi
}

# ── Download tools ────────────────────────────────────────────────────────────
require_downloader() {
  if command -v curl >/dev/null 2>&1; then
    echo "curl"
  elif command -v wget >/dev/null 2>&1; then
    echo "wget"
  else
    die "Neither curl nor wget found. Install one and retry."
  fi
}

# NOTE: POSIX sh has no `local`, so every function below uses variable names
# that cannot collide with the caller's variables (a real bug: verify_archive
# once overwrote the global ARCHIVE and broke extraction).
fetch() { # fetch URL [DEST] — prints to stdout when DEST is omitted
  _f_url="$1"
  _f_dest="${2:-}"
  if [ "${TOOL}" = "curl" ]; then
    if [ -n "${_f_dest}" ]; then curl -fsSL -o "${_f_dest}" "${_f_url}"; else curl -fsSL "${_f_url}"; fi
  else
    if [ -n "${_f_dest}" ]; then wget -q -O "${_f_dest}" "${_f_url}"; else wget -qO- "${_f_url}"; fi
  fi
}

# ── Resolve the release tag ───────────────────────────────────────────────────
resolve_version() {
  if [ "${VERSION}" != "latest" ]; then
    echo "${VERSION}"
    return
  fi
  fetch "${LATEST_API}" \
    | grep '"tag_name"' \
    | head -1 \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

# ── Checksum verification ─────────────────────────────────────────────────────
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo ""
  fi
}

verify_archive() { # verify_archive <archive_path> <expected_name> <sums_file>
  _v_path="$1"; _v_name="$2"; _v_sums="$3"
  # Checksums are matched on the *basename* of the recorded path: some published
  # release manifests record "dir/name" instead of "name".
  _v_expected="$(awk -v n="${_v_name}" '{ p = $2; sub(/.*\//, "", p); if (p == n) { print $1; exit } }' "${_v_sums}")"
  if [ -z "${_v_expected}" ]; then
    die "No checksum for ${_v_name} in SHA256SUMS.txt — refusing to install unverified binary (re-run with --no-verify to override)."
  fi
  _v_actual="$(sha256_of "${_v_path}")"
  if [ -z "${_v_actual}" ]; then
    warn "No sha256sum/shasum available; skipping verification."
    return 0
  fi
  if [ "${_v_expected}" != "${_v_actual}" ]; then
    die "Checksum mismatch for ${_v_name}.
      expected: ${_v_expected}
      actual:   ${_v_actual}
    The download is corrupt or has been tampered with. Aborting."
  fi
  ok "SHA-256 verified: ${_v_name}"
}

# ── Extract only the binary member ────────────────────────────────────────────
# Published archives have shipped the binary under more than one name
# ("omnicli" in the current workflow, "omni" in the v1.0.0 release), so the
# member is located by basename rather than assumed.
BINARY_CANDIDATES="omnicli omnicli.exe omni omni.exe"

archive_members() { # archive_members <archive_path>
  case "$1" in
    *.zip)    unzip -Z1 "$1" ;;
    *.tar.gz) tar tzf "$1" ;;
    *) return 1 ;;
  esac
}

# Reject entries that are absolute or contain a ".." path segment. Note the
# result is captured rather than returned from inside a pipeline: a `return`
# inside `cmd | while` only exits the subshell and would silently pass.
assert_safe_members() { # assert_safe_members <archive_path>
  _s_bad="$(archive_members "$1" 2>/dev/null | sed 's|^[.]/||' | grep -E '^/|(^|/)[.][.](/|$)' || true)"
  if [ -n "${_s_bad}" ]; then
    printf '%s\n' "${_s_bad}" >&2
    return 1
  fi
  return 0
}

find_binary_member() { # find_binary_member <archive_path> — prints the raw member path
  _fb_members="$(archive_members "$1" 2>/dev/null)"
  [ -n "${_fb_members}" ] || return 1
  for _fb_c in ${BINARY_CANDIDATES}; do
    # Exact match on the "./"-stripped member name, returning the raw name so
    # the extractor can address it verbatim.
    _fb_hit="$(printf '%s\n' "${_fb_members}" |
      awk -v c="${_fb_c}" '{ s = $0; sub(/^[.][/]/, "", s); if (s == c) { print $0; exit } }')"
    if [ -n "${_fb_hit}" ]; then printf '%s\n' "${_fb_hit}"; return 0; fi
  done
  # Tolerate archives that nest the binary inside a directory.
  for _fb_c in ${BINARY_CANDIDATES}; do
    _fb_hit="$(printf '%s\n' "${_fb_members}" |
      awk -v c="${_fb_c}" '{ s = $0; sub(/^[.][/]/, "", s); n = split(s, a, "/"); if (a[n] == c) { print $0; exit } }')"
    if [ -n "${_fb_hit}" ]; then printf '%s\n' "${_fb_hit}"; return 0; fi
  done
  return 1
}

extract_binary() { # extract_binary <archive_path> <dest_dir> — prints dest path
  _e_path="$1"; _e_dir="$2"
  _e_member="$(find_binary_member "${_e_path}")" || \
    die "Archive ${_e_path} contains no omnicli binary (looked for: ${BINARY_CANDIDATES})."
  case "${_e_path}" in
    *.zip)
      command -v unzip >/dev/null 2>&1 || die "unzip is required to install from a .zip archive."
      unzip -q -o -j "${_e_path}" -d "${_e_dir}" "${_e_member}" 2>/dev/null \
        || unzip -q -o -j "${_e_path}" -d "${_e_dir}" "${_e_member#./}" 2>/dev/null \
        || die "Failed to extract ${_e_member} from ${_e_path}."
      ;;
    *.tar.gz)
      # Extract exactly one member, so a hostile archive cannot write outside
      # ${_e_dir} via "../" entries or absolute paths.
      tar xzf "${_e_path}" -C "${_e_dir}" "${_e_member}" \
        || die "Failed to extract ${_e_member} from ${_e_path}."
      ;;
    *) die "Unsupported archive type: ${_e_path}" ;;
  esac

  _e_base="$(basename "${_e_member}")"
  # tar keeps nested paths; locate whatever landed and flatten it.
  _e_found=""
  for _e_c in "${_e_dir}/${_e_base}" "${_e_dir}/${_e_member}" "${_e_dir}/${_e_member#./}"; do
    [ -f "${_e_c}" ] && { _e_found="${_e_c}"; break; }
  done
  [ -n "${_e_found}" ] || _e_found="$(find "${_e_dir}" -type f -name "${_e_base}" | head -1)"
  [ -n "${_e_found}" ] || die "Extraction reported success but ${_e_base} was not found."

  [ "${_e_found}" = "${_e_dir}/${BINARY_NAME}" ] || mv -f "${_e_found}" "${_e_dir}/${BINARY_NAME}"
  printf '%s\n' "${_e_dir}/${BINARY_NAME}"
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
  PREFIX_ARG=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --prefix)    [ "$#" -ge 2 ] || die "--prefix requires a directory"; PREFIX_ARG="$2"; shift 2 ;;
      --version)   [ "$#" -ge 2 ] || die "--version requires a tag"; VERSION="$2"; shift 2 ;;
      --no-verify) VERIFY=0; shift ;;
      -h|--help)   usage; exit 0 ;;
      *) die "Unknown argument: $1 (try --help)" ;;
    esac
  done

  TOOL="$(require_downloader)"
  TARGET="$(detect_target)"
  INSTALL_DIR="${PREFIX_ARG:-$(detect_prefix)}"

  info "Detected target: ${TARGET}"
  info "Install directory: ${INSTALL_DIR}"

  TAG="$(resolve_version)"
  [ -n "${TAG}" ] || die "Could not determine the release version. Pass --version vX.Y.Z to pin one."
  info "Installing version: ${TAG}"

  # Archive naming changed between v1.0.0 ("omni-*") and later releases
  # ("omnicli-*"); try the current name first, then the historical one.
  BASE="${RELEASES_URL}/download/${TAG}"
  ARCHIVE_NAME="omnicli-${TARGET}.tar.gz"
  LEGACY_NAME="omni-${TARGET}.tar.gz"
  case "${TARGET}" in
    linux-armv7) die "No prebuilt archive is published for ${TARGET}. Build from source: cd omnicli && cargo build --release" ;;
  esac

  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT INT TERM

  ARCHIVE=""
  for CANDIDATE in "${ARCHIVE_NAME}" "${LEGACY_NAME}"; do
    info "Downloading ${CANDIDATE}…"
    if fetch "${BASE}/${CANDIDATE}" "${TMP}/${CANDIDATE}" 2>/dev/null; then
      ARCHIVE="${CANDIDATE}"
      break
    fi
    rm -f "${TMP}/${CANDIDATE}"
  done
  [ -n "${ARCHIVE}" ] || die "Could not download an archive for ${TARGET} from release ${TAG}."

  if [ "${VERIFY}" -eq 1 ]; then
    if fetch "${BASE}/SHA256SUMS.txt" "${TMP}/SHA256SUMS.txt" 2>/dev/null; then
      verify_archive "${TMP}/${ARCHIVE}" "${ARCHIVE}" "${TMP}/SHA256SUMS.txt"
    else
      die "Could not download SHA256SUMS.txt — refusing to install unverified. Use --no-verify to override."
    fi
  else
    warn "Checksum verification disabled (--no-verify)."
  fi

  case "${ARCHIVE}" in
    *.zip) command -v unzip >/dev/null 2>&1 || die "unzip is required to install from a .zip archive." ;;
  esac
  if ! assert_safe_members "${TMP}/${ARCHIVE}"; then
    die "Archive ${ARCHIVE} contains an entry that escapes the extraction directory. Aborting."
  fi

  info "Extracting…"
  mkdir -p "${TMP}/extract"
  BIN_PATH="$(extract_binary "${TMP}/${ARCHIVE}" "${TMP}/extract")"
  [ -f "${BIN_PATH}" ] || die "Extraction did not produce a usable binary."

  mkdir -p "${INSTALL_DIR}" 2>/dev/null || true
  if [ "$(id -u)" -eq 0 ] || [ -w "${INSTALL_DIR}" ]; then
    install -m 755 "${BIN_PATH}" "${INSTALL_DIR}/${BINARY_NAME}"
  elif command -v sudo >/dev/null 2>&1; then
    sudo install -m 755 "${BIN_PATH}" "${INSTALL_DIR}/${BINARY_NAME}"
  else
    die "Cannot write to ${INSTALL_DIR} and sudo is unavailable. Re-run with --prefix \"\$HOME/.local/bin\"."
  fi

  # ── Verify the installed binary actually runs ───────────────────────────────
  if "${INSTALL_DIR}/${BINARY_NAME}" --version >/dev/null 2>&1; then
    ok "${BOLD}OmniCLI ${TAG} installed: ${INSTALL_DIR}/${BINARY_NAME}${RESET}"
  else
    die "Installation verification failed: ${INSTALL_DIR}/${BINARY_NAME} did not run."
  fi

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) : ;;
    *)
      printf '\n%b%s%b\n' "${CYAN}" "Add ${INSTALL_DIR} to your PATH:" "${RESET}"
      printf '  %s\n\n' "export PATH=\"\${PATH}:${INSTALL_DIR}\""
      ;;
  esac

  # ── Shell completion hint (optional, non-fatal) ─────────────────────────────
  printf '%s\n' "Optional: ${BINARY_NAME} completions bash >> ~/.bashrc   # or zsh / fish"
}

main "$@"

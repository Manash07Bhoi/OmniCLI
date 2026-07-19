#!/bin/sh
# OmniCLI installer — POSIX sh, no bash required
# Supports: Kali Linux, ParrotOS, Debian/Ubuntu, Termux (Android)
# Usage: curl -fsSL https://raw.githubusercontent.com/YOUR_REPO/main/omnicli/scripts/install.sh | sh
#   or:  sh omnicli/scripts/install.sh [--prefix /path]

set -e

REPO="https://github.com/YOU/omnicli"
VERSION="${OMNI_VERSION:-latest}"
BINARY_NAME="omni"

# ── Colours (skip when NO_COLOR is set or not a TTY) ──────────────────────────
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'
  BOLD='\033[1m'; RESET='\033[0m'
else
  RED=''; GREEN=''; CYAN=''; BOLD=''; RESET=''
fi

info()  { printf '%b%s%b\n' "${CYAN}"  "[omni] $*" "${RESET}"; }
ok()    { printf '%b%s%b\n' "${GREEN}" "[omni] $*" "${RESET}"; }
die()   { printf '%b%s%b\n' "${RED}"   "[omni] ERROR: $*" "${RESET}" >&2; exit 1; }

# ── Detect OS and architecture ────────────────────────────────────────────────
detect_target() {
  OS="$(uname -s)"
  ARCH="$(uname -m)"

  # Termux (Android)
  if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ]; then
    case "${ARCH}" in
      aarch64|arm64) echo "android-aarch64" ;;
      armv7*|armv8l) echo "android-armv7"   ;;
      *) die "Unsupported Termux architecture: ${ARCH}" ;;
    esac
    return
  fi

  case "${OS}" in
    Linux)
      case "${ARCH}" in
        x86_64|amd64)       echo "linux-x86_64"   ;;
        aarch64|arm64)      echo "linux-aarch64"   ;;
        armv7*|armv6*)      echo "linux-armv7"     ;;
        *) die "Unsupported Linux architecture: ${ARCH}" ;;
      esac ;;
    Darwin)
      case "${ARCH}" in
        x86_64) echo "macos-x86_64"  ;;
        arm64)  echo "macos-aarch64" ;;
        *) die "Unsupported macOS architecture: ${ARCH}" ;;
      esac ;;
    *) die "Unsupported OS: ${OS}" ;;
  esac
}

# ── Determine install prefix ──────────────────────────────────────────────────
detect_prefix() {
  if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ]; then
    echo "${PREFIX:-/data/data/com.termux/files/usr}/bin"
  elif [ "$(id -u)" -eq 0 ]; then
    echo "/usr/local/bin"
  else
    echo "${HOME}/.local/bin"
  fi
}

# ── Require a download tool ───────────────────────────────────────────────────
require_downloader() {
  if command -v curl >/dev/null 2>&1; then
    echo "curl"
  elif command -v wget >/dev/null 2>&1; then
    echo "wget"
  else
    die "Neither curl nor wget found. Please install one and retry."
  fi
}

download() {
  URL="$1"
  DEST="$2"
  TOOL="$3"
  case "${TOOL}" in
    curl) curl -fsSL -o "${DEST}" "${URL}" ;;
    wget) wget -q -O "${DEST}"  "${URL}" ;;
  esac
}

# ── Resolve latest release tag ────────────────────────────────────────────────
resolve_version() {
  TOOL="$1"
  if [ "${VERSION}" = "latest" ]; then
    API_URL="https://api.github.com/repos/YOU/omnicli/releases/latest"
    case "${TOOL}" in
      curl) curl -fsSL "${API_URL}" | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/' ;;
      wget) wget -qO- "${API_URL}"  | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/' ;;
    esac
  else
    echo "${VERSION}"
  fi
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
  PREFIX_ARG=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --prefix) PREFIX_ARG="$2"; shift 2 ;;
      *) die "Unknown argument: $1" ;;
    esac
  done

  TARGET="$(detect_target)"
  INSTALL_DIR="${PREFIX_ARG:-$(detect_prefix)}"
  TOOL="$(require_downloader)"

  info "Detected target: ${TARGET}"
  info "Install directory: ${INSTALL_DIR}"

  TAG="$(resolve_version "${TOOL}")"
  [ -z "${TAG}" ] && die "Could not determine release version. Set OMNI_VERSION=vX.Y.Z to pin a version."
  info "Installing version: ${TAG}"

  ARCHIVE_NAME="omni-${TARGET}.tar.gz"
  DOWNLOAD_URL="${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"

  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT

  info "Downloading ${ARCHIVE_NAME}…"
  download "${DOWNLOAD_URL}" "${TMP}/${ARCHIVE_NAME}" "${TOOL}"

  info "Extracting…"
  tar xzf "${TMP}/${ARCHIVE_NAME}" -C "${TMP}"

  # Ensure the install directory exists
  mkdir -p "${INSTALL_DIR}"

  info "Installing to ${INSTALL_DIR}/${BINARY_NAME}…"
  if [ "$(id -u)" -eq 0 ] || [ -w "${INSTALL_DIR}" ]; then
    install -m 755 "${TMP}/omni" "${INSTALL_DIR}/${BINARY_NAME}"
  else
    # Try sudo for system paths
    sudo install -m 755 "${TMP}/omni" "${INSTALL_DIR}/${BINARY_NAME}"
  fi

  # ── Verify ────────────────────────────────────────────────────────────────
  if "${INSTALL_DIR}/${BINARY_NAME}" --version >/dev/null 2>&1; then
    ok "${BOLD}OmniCLI ${TAG} installed successfully!${RESET}"
  else
    die "Installation verification failed. The binary at ${INSTALL_DIR}/${BINARY_NAME} did not run."
  fi

  # ── PATH reminder ─────────────────────────────────────────────────────────
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) : ;;  # already in PATH
    *)
      printf '\n%b%s%b\n' "${CYAN}" \
        "Add ${INSTALL_DIR} to your PATH:" "${RESET}"
      printf '  %s\n\n' "export PATH=\"\${PATH}:${INSTALL_DIR}\""
      ;;
  esac
}

main "$@"

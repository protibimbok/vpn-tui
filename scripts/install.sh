#!/usr/bin/env bash
# Install vpn from GitHub releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/protibimbok/vpn-tui/master/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- --version v0.1.0
#   curl -fsSL ... | bash -s -- --install-dir /usr/local/bin

set -e

REPO="protibimbok/vpn-tui"
BINARY="vpn"
RELEASE="latest"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ── helpers ────────────────────────────────────────────────────────────────

info()  { printf '\033[0;32m[vpn]\033[0m %s\n' "$*"; }
warn()  { printf '\033[0;33m[vpn]\033[0m %s\n' "$*" >&2; }
error() { printf '\033[0;31m[vpn] error:\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || error "required tool not found: $1"; }

usage() {
  cat <<EOF
Install vpn from GitHub releases.

Usage:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/scripts/install.sh | bash
  curl -fsSL ... | bash -s -- [options]

Options:
  -d, --install-dir <dir>   Install directory (default: /usr/local/bin)
  -r, --version <tag>       Release tag to install (default: latest)
  -h, --help                Show this help

Environment:
  INSTALL_DIR               Same as --install-dir

Dependencies (install via your package manager if missing):
  wireguard-tools, curl, ping
EOF
}

parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      -d | --install-dir)
        [ -n "${2:-}" ] || error "--install-dir requires a path"
        INSTALL_DIR=$2
        shift 2
        ;;
      -r | --version | --release)
        [ -n "${2:-}" ] || error "--version requires a tag"
        RELEASE=$2
        shift 2
        ;;
      -h | --help)
        usage
        exit 0
        ;;
      *)
        error "unknown argument: $1 (try --help)"
        ;;
    esac
  done
}

detect_os() {
  case "$(uname -s)" in
    Linux*) echo linux ;;
    *) error "unsupported OS: $(uname -s) (vpn requires Linux)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64)  echo amd64 ;;
    aarch64 | arm64) echo arm64 ;;
    *) error "unsupported architecture: $(uname -m) (supported: amd64, arm64)" ;;
  esac
}

resolve_release_tag() {
  if [ "$RELEASE" = "latest" ]; then
    local json
    json=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")
    printf '%s\n' "$json" \
      | grep -m1 '"tag_name"' \
      | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
  else
    printf '%s\n' "$RELEASE"
  fi
}

verify_checksum() {
  local archive=$1
  local checksums=$2
  local archive_name expected actual

  archive_name=$(basename "$archive")
  expected=$(grep "${archive_name}" "${checksums}" | awk '{print $1}')
  [ -n "$expected" ] || error "checksum not found for ${archive_name}"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$archive")" && grep "${archive_name}" "$(basename "$checksums")" | sha256sum -c -)
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$archive" | awk '{print $1}')
    [ "$actual" = "$expected" ] || error "checksum mismatch for ${archive}"
  else
    error "sha256sum or shasum is required to verify downloads"
  fi
}

install_binary() {
  local src=$1
  local dest="${INSTALL_DIR}/${BINARY}"

  mkdir -p "$INSTALL_DIR"

  # setuid bit requires root; vpn drops privileges at startup and only
  # re-elevates wg/wg-quick child processes.
  if [ "$(id -u)" -eq 0 ]; then
    install -o root -g root -m 4755 "$src" "$dest"
  elif command -v sudo >/dev/null 2>&1; then
    info "Installing setuid binary to ${dest} (sudo required)"
    sudo install -o root -g root -m 4755 "$src" "$dest"
  else
    error "root or sudo is required to install vpn with setuid permissions"
  fi
}

check_dependencies() {
  need curl
  need tar

  local missing=()
  command -v wg-quick >/dev/null 2>&1 || missing+=(wireguard-tools)
  command -v curl >/dev/null 2>&1 || missing+=(curl)
  command -v ping >/dev/null 2>&1 || missing+=(iputils-ping)

  if [ "${#missing[@]}" -gt 0 ]; then
    warn "missing dependencies: ${missing[*]}"
    warn "install them with your package manager before running vpn"
  fi
}

install_via_release() {
  TAG=$(resolve_release_tag)
  [ -n "$TAG" ] || error "could not resolve release tag"

  ARCHIVE="${BINARY}_${OS}_${ARCH}.tar.gz"
  URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"

  info "Installing vpn ${TAG} (${OS}/${ARCH}) to ${INSTALL_DIR}..."

  TMP=$(mktemp -d)
  trap 'rm -rf "$TMP"' EXIT

  info "Downloading ${URL}"
  curl -fsSL "$URL" -o "${TMP}/${ARCHIVE}"

  CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/checksums.txt"
  curl -fsSL "$CHECKSUM_URL" -o "${TMP}/checksums.txt"
  verify_checksum "${TMP}/${ARCHIVE}" "${TMP}/checksums.txt"
  info "Checksum verified"

  tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"
  install_binary "${TMP}/${BINARY}"
}

# ── main ───────────────────────────────────────────────────────────────────

parse_args "$@"

OS=$(detect_os)
ARCH=$(detect_arch)

check_dependencies
install_via_release

if command -v vpn >/dev/null 2>&1; then
  info "Installed: $(command -v vpn)"
  vpn --version
else
  warn "vpn was installed to ${INSTALL_DIR}/${BINARY} but is not on your PATH"
  warn "add ${INSTALL_DIR} to your PATH, then run: vpn"
fi

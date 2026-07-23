#!/usr/bin/env bash
# Build release tarball and Linux packages for one target.
#
# Usage: build-release-artifacts.sh <version> <rust-target> <arch>
# Example: build-release-artifacts.sh 0.1.0 x86_64-unknown-linux-gnu amd64

set -euo pipefail

VERSION="${1:?version required (without v prefix)}"
TARGET="${2:?rust target required}"
ARCH="${3:?arch required (amd64 or arm64)}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/release-artifacts/${ARCH}"
BINARY="${ROOT}/target/${TARGET}/release/vpn"
STAGE="${OUT}/stage"

need() { command -v "$1" >/dev/null 2>&1 || { echo "required: $1" >&2; exit 1; }; }

need nfpm

[ -f "${BINARY}" ] || { echo "binary not found: ${BINARY}" >&2; exit 1; }

mkdir -p "${STAGE}" "${OUT}/dist"
cp "${BINARY}" "${OUT}/dist/vpn"
chmod 755 "${OUT}/dist/vpn"
cp "${ROOT}/LICENSE" "${OUT}/LICENSE"

cp "${BINARY}" "${STAGE}/vpn"
cp "${ROOT}/LICENSE" "${STAGE}/LICENSE"
tar -C "${STAGE}" -czf "${OUT}/vpn_linux_${ARCH}.tar.gz" vpn LICENSE

export NFPM_VERSION="${VERSION}"
export NFPM_ARCH="${ARCH}"

cd "${OUT}"
# Explicit targets: standalone nfpm has no file_name_template (GoReleaser-only).
nfpm package -f "${ROOT}/packaging/nfpm.yaml" --packager deb --target "vpn_${VERSION}_linux_${ARCH}.deb"
nfpm package -f "${ROOT}/packaging/nfpm.yaml" --packager rpm --target "vpn_${VERSION}_linux_${ARCH}.rpm"
nfpm package -f "${ROOT}/packaging/nfpm.yaml" --packager apk --target "vpn_${VERSION}_linux_${ARCH}.apk"

echo "Built ${OUT}/"

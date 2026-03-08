#!/usr/bin/env bash
set -euo pipefail

REPO="${REPO:-remcostoeten/kiekje}"
PREFIX="${PREFIX:-$HOME/.local}"
TAG=""
INSTALL_TUI=1
INSTALL_DEPS=0
ASSUME_YES=0

usage() {
    cat <<'EOF'
Usage: scripts/install-release.sh [options]

Download and install a prebuilt Kiekje release from GitHub.

Options:
  --tag TAG       Install a specific tag instead of the latest release
  --repo REPO     GitHub repo in owner/name form
  --prefix PATH   Install into PATH instead of ~/.local
  --no-tui        Skip installing the kiekje-tui helper binary
  --install-deps  Install missing core runtime packages when possible
  --yes           Non-interactive package install for supported package managers
  --help          Show this help text

Examples:
  scripts/install-release.sh
  scripts/install-release.sh --tag v0.0.1
EOF
}

download() {
    local url="$1"
    local out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$out"
        return
    fi
    if command -v wget >/dev/null 2>&1; then
        wget -qO "$out" "$url"
        return
    fi
    echo "Missing curl or wget for download." >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag)
            TAG="$2"
            shift 2
            ;;
        --repo)
            REPO="$2"
            shift 2
            ;;
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --no-tui)
            INSTALL_TUI=0
            shift
            ;;
        --install-deps)
            INSTALL_DEPS=1
            shift
            ;;
        --yes)
            ASSUME_YES=1
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH_NAME="$(uname -m)"

case "$ARCH_NAME" in
    x86_64) ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
esac

ASSET_BASENAME="kiekje-${OS_NAME}-${ARCH_NAME}"
ASSET_NAME="${ASSET_BASENAME}.tar.gz"

if [[ -n "$TAG" ]]; then
    URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET_NAME}"
else
    URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

ARCHIVE_PATH="$TMP_DIR/$ASSET_NAME"
download "$URL" "$ARCHIVE_PATH"
tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"

BUNDLE_DIR="$TMP_DIR/$ASSET_BASENAME"
if [[ ! -d "$BUNDLE_DIR" ]]; then
    echo "Downloaded archive did not contain $ASSET_BASENAME" >&2
    exit 1
fi

ARGS=(--prefix "$PREFIX")
if [[ "$INSTALL_TUI" -eq 0 ]]; then
    ARGS+=(--no-tui)
fi
if [[ "$INSTALL_DEPS" -eq 1 ]]; then
    ARGS+=(--install-deps)
fi
if [[ "$ASSUME_YES" -eq 1 ]]; then
    ARGS+=(--yes)
fi

"$BUNDLE_DIR/install.sh" "${ARGS[@]}"

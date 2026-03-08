#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${OUT_DIR:-dist}"
SKIP_BUILD=0

usage() {
    cat <<'EOF'
Usage: scripts/package-release.sh [options]

Build release artifacts and package them into a distributable tarball.

Options:
  --out-dir PATH  Write artifacts to PATH instead of ./dist
  --skip-build    Reuse existing release binaries
  --help          Show this help text
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out-dir)
            OUT_DIR="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
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

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH_NAME="$(uname -m)"

case "$ARCH_NAME" in
    x86_64) ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
esac

ASSET_BASENAME="kiekje-${OS_NAME}-${ARCH_NAME}"
STAGE_ROOT="$REPO_ROOT/$OUT_DIR/.stage"
PACKAGE_DIR="$STAGE_ROOT/$ASSET_BASENAME"
ARCHIVE_PATH="$REPO_ROOT/$OUT_DIR/${ASSET_BASENAME}.tar.gz"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"
TUI_TMP="$REPO_ROOT/target/${ASSET_BASENAME}-kiekje-tui"

mkdir -p "$REPO_ROOT/$OUT_DIR"
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"
    (
        cd "$REPO_ROOT/cmd/kiekje-tui"
        go build -o "$TUI_TMP" .
    )
fi

install -m755 "$REPO_ROOT/target/release/kiekje" "$PACKAGE_DIR/kiekje"
if [[ -x "$TUI_TMP" ]]; then
    install -m755 "$TUI_TMP" "$PACKAGE_DIR/kiekje-tui"
elif [[ -x "$REPO_ROOT/kiekje-tui" ]]; then
    install -m755 "$REPO_ROOT/kiekje-tui" "$PACKAGE_DIR/kiekje-tui"
fi
install -m644 "$REPO_ROOT/share/applications/kiekje.desktop" "$PACKAGE_DIR/kiekje.desktop"
install -m755 "$REPO_ROOT/scripts/bundle-install.sh" "$PACKAGE_DIR/install.sh"
install -m755 "$REPO_ROOT/scripts/bundle-uninstall.sh" "$PACKAGE_DIR/uninstall.sh"
install -m644 "$REPO_ROOT/scripts/deps.sh" "$PACKAGE_DIR/deps.sh"
install -m644 "$REPO_ROOT/README.md" "$PACKAGE_DIR/README.md"
install -m644 "$REPO_ROOT/LICENSE" "$PACKAGE_DIR/LICENSE"

tar -C "$STAGE_ROOT" -czf "$ARCHIVE_PATH" "$ASSET_BASENAME"

if command -v sha256sum >/dev/null 2>&1; then
    (
        cd "$REPO_ROOT/$OUT_DIR"
        sha256sum "${ASSET_BASENAME}.tar.gz" > "${ASSET_BASENAME}.tar.gz.sha256"
    )
elif command -v shasum >/dev/null 2>&1; then
    (
        cd "$REPO_ROOT/$OUT_DIR"
        shasum -a 256 "${ASSET_BASENAME}.tar.gz" > "${ASSET_BASENAME}.tar.gz.sha256"
    )
else
    echo "Missing sha256sum/shasum for checksum generation" >&2
    exit 1
fi

echo "Created:"
echo "  $ARCHIVE_PATH"
echo "  $CHECKSUM_PATH"

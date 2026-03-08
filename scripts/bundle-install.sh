#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
INSTALL_TUI=1
INSTALL_DEPS=0
ASSUME_YES=0

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./deps.sh
source "$BUNDLE_DIR/deps.sh"

usage() {
    cat <<'EOF'
Usage: ./install.sh [options]

Install a packaged Kiekje release bundle.

Options:
  --prefix PATH   Install into PATH instead of ~/.local
  --no-tui        Skip installing the kiekje-tui helper binary
  --install-deps  Install missing core runtime packages when possible
  --yes           Non-interactive package install for supported package managers
  --help          Show this help text
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
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

BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"

if [[ "$INSTALL_DEPS" -eq 1 ]]; then
    kiekje_install_core_dependencies "$ASSUME_YES"
fi

install -d "$BIN_DIR" "$APP_DIR"
install -m755 "$BUNDLE_DIR/kiekje" "$BIN_DIR/kiekje"
install -m644 "$BUNDLE_DIR/kiekje.desktop" "$APP_DIR/kiekje.desktop"

if [[ "$INSTALL_TUI" -eq 1 && -f "$BUNDLE_DIR/kiekje-tui" ]]; then
    install -m755 "$BUNDLE_DIR/kiekje-tui" "$BIN_DIR/kiekje-tui"
fi

echo "Installed Kiekje into $PREFIX"
echo "Make sure $BIN_DIR is in PATH."
if ! kiekje_print_dependency_status; then
    echo
    echo "Re-run with --install-deps to install missing core runtime packages automatically."
fi
"$BIN_DIR/kiekje" --doctor || true

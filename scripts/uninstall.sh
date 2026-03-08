#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
REMOVE_TUI=1

usage() {
    cat <<'EOF'
Usage: scripts/uninstall.sh [options]

Remove Kiekje from a local or system prefix.

Options:
  --prefix PATH   Remove from PATH instead of ~/.local
  --no-tui        Leave the kiekje-tui helper binary installed
  --help          Show this help text

Examples:
  scripts/uninstall.sh
  sudo scripts/uninstall.sh --prefix /usr/local
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --no-tui)
            REMOVE_TUI=0
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

rm -f "$BIN_DIR/kiekje"
if [[ "$REMOVE_TUI" -eq 1 ]]; then
    rm -f "$BIN_DIR/kiekje-tui"
fi
rm -f "$APP_DIR/kiekje.desktop"

echo "Removed install artifacts from $PREFIX"
echo "User config is kept in ~/.config/kiekje"

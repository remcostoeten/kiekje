#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
BUILD_MODE="release"
INSTALL_TUI=1
SKIP_BUILD=0
INSTALL_DEPS=0
ASSUME_YES=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./deps.sh
source "$SCRIPT_DIR/deps.sh"

usage() {
    cat <<'EOF'
Usage: scripts/install.sh [options]

Install Kiekje into a local or system prefix.

Options:
  --prefix PATH   Install into PATH instead of ~/.local
  --debug         Install the debug build instead of release
  --no-tui        Skip installing the kiekje-tui helper binary
  --skip-build    Reuse existing build artifacts
  --install-deps  Install missing core runtime packages when possible
  --yes           Non-interactive package install for supported package managers
  --help          Show this help text

Examples:
  scripts/install.sh
  sudo scripts/install.sh --prefix /usr/local
  scripts/install.sh --debug --no-tui
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --debug)
            BUILD_MODE="debug"
            shift
            ;;
        --no-tui)
            INSTALL_TUI=0
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
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

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
RUST_BIN="$REPO_ROOT/target/$BUILD_MODE/kiekje"
TUI_TMP="$REPO_ROOT/target/kiekje-tui-install"

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    if [[ "$BUILD_MODE" == "release" ]]; then
        cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"
    else
        cargo build --manifest-path "$REPO_ROOT/Cargo.toml"
    fi

    if [[ "$INSTALL_TUI" -eq 1 ]]; then
        (
            cd "$REPO_ROOT/cmd/kiekje-tui"
            go build -o "$TUI_TMP" .
        )
    fi
fi

if [[ "$INSTALL_DEPS" -eq 1 ]]; then
    kiekje_install_core_dependencies "$ASSUME_YES"
fi

if [[ ! -x "$RUST_BIN" ]]; then
    echo "Missing built binary: $RUST_BIN" >&2
    echo "Run scripts/install.sh without --skip-build or build kiekje first." >&2
    exit 1
fi

install -d "$BIN_DIR" "$APP_DIR"
install -m755 "$RUST_BIN" "$BIN_DIR/kiekje"
install -m644 "$REPO_ROOT/share/applications/kiekje.desktop" "$APP_DIR/kiekje.desktop"

if [[ "$INSTALL_TUI" -eq 1 ]]; then
    if [[ ! -x "$TUI_TMP" && ! -x "$REPO_ROOT/kiekje-tui" ]]; then
        echo "Missing built helper binary: $TUI_TMP" >&2
        exit 1
    fi
    if [[ -x "$TUI_TMP" ]]; then
        install -m755 "$TUI_TMP" "$BIN_DIR/kiekje-tui"
    else
        install -m755 "$REPO_ROOT/kiekje-tui" "$BIN_DIR/kiekje-tui"
    fi
fi

echo "Installed:"
echo "  $BIN_DIR/kiekje"
if [[ "$INSTALL_TUI" -eq 1 ]]; then
    echo "  $BIN_DIR/kiekje-tui"
fi
echo "  $APP_DIR/kiekje.desktop"
echo
echo "Make sure $BIN_DIR is in PATH."
if ! kiekje_print_dependency_status; then
    echo
    echo "Re-run with --install-deps to install missing core runtime packages automatically."
fi
"$BIN_DIR/kiekje" --doctor || true

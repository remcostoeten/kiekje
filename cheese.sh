#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_NAME="cheese-wails"
BIN_PATH="$ROOT_DIR/build/bin/$APP_NAME"
TRAY_BIN_PATH="$ROOT_DIR/build/bin/cheese-tray"
DIST_DIR="$ROOT_DIR/dist"

cd "$ROOT_DIR"

usage() {
  cat <<EOF
Usage: ./cheese.sh [command]

Commands:
  menu      Open the interactive menu
  dev       Run Wails with live reload
  run       Run the built binary, building it first if needed
  stop      Stop running Cheese/Wails processes
  restart   Stop, rebuild, then run the built binary
  rebuild   Clean production build into build/bin
  ship      Clean production build and create a tar.gz in dist/
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

stop_app() {
  pkill -x "cheese-tray" >/dev/null 2>&1 || true
  pkill -x "$APP_NAME" >/dev/null 2>&1 || true
  pkill -f "cheese-wails-dev-linux-amd64" >/dev/null 2>&1 || true
  pkill -f "wails dev" >/dev/null 2>&1 || true
}

build_tray() {
  require_cmd go
  mkdir -p "$(dirname "$TRAY_BIN_PATH")"
  go build -o "$TRAY_BIN_PATH" ./cmd/cheese-tray
}

build_app() {
  require_cmd wails
  wails build -clean
  build_tray
}

run_app() {
  if [[ ! -x "$BIN_PATH" ]]; then
    build_app
  elif [[ ! -x "$TRAY_BIN_PATH" ]]; then
    build_tray
  fi
  if [[ "${CHEESE_DEBUG:-0}" == "1" ]]; then
    exec "$BIN_PATH"
  fi
  exec "$BIN_PATH" 2> >(
    grep -vE "Gtk-WARNING|Theme parsing error|Overriding existing handler for signal|JSC_SIGNAL_FOR_GC" >&2
  )
}

ship_app() {
  build_app
  mkdir -p "$DIST_DIR"

  local os arch archive staging
  os="$(go env GOOS)"
  arch="$(go env GOARCH)"
  archive="$DIST_DIR/$APP_NAME-$os-$arch.tar.gz"
  staging="$(mktemp -d)"

  cp "$BIN_PATH" "$staging/$APP_NAME"
  cp "$TRAY_BIN_PATH" "$staging/cheese-tray"
  cp README.md "$staging/README.md"
  tar -C "$staging" -czf "$archive" .
  rm -rf "$staging"

  echo "Shippable binary: $BIN_PATH"
  echo "Tray sidecar: $TRAY_BIN_PATH"
  echo "Archive: $archive"
}

print_header() {
  clear
  echo "Cheese Wails"
  echo "============"
  echo
  echo "Project: $ROOT_DIR"
  echo "Binary:  $BIN_PATH"
  echo "Tray:    $TRAY_BIN_PATH"
  echo
}

pause() {
  echo
  read -r -p "Press Enter to continue..." _
}

run_interactive_command() {
  case "$1" in
    dev)
      require_cmd wails
      build_tray
      exec wails dev
      ;;
    run)
      run_app
      ;;
    stop)
      stop_app
      echo "Stopped running Cheese/Wails processes."
      ;;
    restart)
      stop_app
      build_app
      run_app
      ;;
    rebuild)
      build_app
      ;;
    ship)
      require_cmd go
      ship_app
      ;;
    status)
      if pgrep -x "$APP_NAME" >/dev/null 2>&1; then
        echo "$APP_NAME is running:"
        pgrep -ax "$APP_NAME"
      else
        echo "$APP_NAME is not running."
      fi
      if pgrep -f "wails dev" >/dev/null 2>&1; then
        echo
        echo "wails dev is running:"
        pgrep -af "wails dev"
      fi
      ;;
  esac
}

menu() {
  while true; do
    print_header
    cat <<EOF
Choose an action:

  1) Dev        Run Wails with live reload
  2) Run        Run the built binary
  3) Stop       Stop running app/dev processes
  4) Restart    Stop, rebuild, run
  5) Rebuild    Clean production build
  6) Ship       Build and create dist tarball
  7) Status     Show running processes
  q) Quit

EOF
    read -r -p "> " choice
    echo
    case "$choice" in
      1|dev) run_interactive_command dev ;;
      2|run) run_interactive_command run ;;
      3|stop) run_interactive_command stop; pause ;;
      4|restart) run_interactive_command restart ;;
      5|rebuild) run_interactive_command rebuild; pause ;;
      6|ship) run_interactive_command ship; pause ;;
      7|status) run_interactive_command status; pause ;;
      q|Q|quit|exit) exit 0 ;;
      *) echo "Unknown choice: $choice"; pause ;;
    esac
  done
}

case "${1:-menu}" in
  menu)
    menu
    ;;
  dev)
    require_cmd wails
    build_tray
    exec wails dev
    ;;
  run)
    run_app
    ;;
  stop)
    stop_app
    ;;
  restart)
    stop_app
    build_app
    run_app
    ;;
  rebuild)
    build_app
    ;;
  ship)
    require_cmd go
    ship_app
    ;;
  -h|--help|help|"")
    usage
    ;;
  *)
    echo "Unknown command: $1" >&2
    usage >&2
    exit 1
    ;;
esac

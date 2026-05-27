#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_NAME="kiekje"
APP_ID="kiekje"
APP_TITLE="Kiekje"
BIN_PATH="$ROOT_DIR/build/bin/$APP_NAME"
TRAY_BIN_PATH="$ROOT_DIR/build/bin/kiekje-tray"
DIST_DIR="$ROOT_DIR/dist"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
INSTALL_BIN_DIR="$INSTALL_PREFIX/bin"
INSTALL_APP_PATH="$INSTALL_BIN_DIR/$APP_ID"
INSTALL_TRAY_PATH="$INSTALL_BIN_DIR/kiekje-tray"
INSTALL_ICON_PATH="$INSTALL_PREFIX/share/icons/hicolor/256x256/apps/$APP_ID.png"
DESKTOP_PATH="$INSTALL_PREFIX/share/applications/$APP_ID.desktop"
AUTOSTART_PATH="$HOME/.config/autostart/$APP_ID.desktop"
HYPR_DIR="$HOME/.config/hypr"
HYPR_SNIPPET="$HYPR_DIR/kiekje-bindings.conf"

cd "$ROOT_DIR"

usage() {
  cat <<EOF
Usage: ./kiekje.sh [command]

Commands:
  menu      Open the interactive menu
  dev       Run Wails with live reload
  run       Run the built binary, building it first if needed
  stop      Stop running Kiekje processes
  restart   Stop, rebuild, then run the built binary
  rebuild   Clean production build into build/bin
  ship      Clean production build and create a tar.gz in dist/
  release   Prepare semver tag and changelog (git-cliff)
  install   Install app, autostart, desktop entry, and Hyprland config
  uninstall Remove installed app, autostart, and desktop entry
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

stop_app() {
  pkill -x "kiekje-tray" >/dev/null 2>&1 || true
  pkill -x "$APP_NAME" >/dev/null 2>&1 || true
  pkill -f "kiekje-dev-linux-amd64" >/dev/null 2>&1 || true
  pkill -f "wails dev" >/dev/null 2>&1 || true
}

build_tray() {
  require_cmd go
  mkdir -p "$(dirname "$TRAY_BIN_PATH")"
  go build -o "$TRAY_BIN_PATH" ./cmd/kiekje-tray
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
  if [[ "${KIEKJE_DEBUG:-0}" == "1" ]]; then
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
  cp "$TRAY_BIN_PATH" "$staging/kiekje-tray"
  cp build/appicon.png "$staging/$APP_ID.png"
  cp build/install.sh "$staging/install.sh"
  cp README.md "$staging/README.md"
  chmod +x "$staging/install.sh"
  tar -C "$staging" -czf "$archive" .
  rm -rf "$staging"

  echo "Shippable binary: $BIN_PATH"
  echo "Tray sidecar: $TRAY_BIN_PATH"
  echo "Archive: $archive"
}

desktop_file() {
  local exec_path="$1"
  cat <<EOF
[Desktop Entry]
Type=Application
Name=$APP_TITLE
Comment=Screenshot capture and annotation tool
Exec=$exec_path
Icon=$APP_ID
Terminal=false
Categories=Utility;Graphics;
StartupNotify=false
EOF
}

write_hyprland_config() {
  local app_path="${1:-$INSTALL_APP_PATH}"
  if [[ ! -x "$app_path" ]]; then
    echo "Missing Kiekje binary for Hyprland sync: $app_path" >&2
    return 1
  fi
  "$app_path" --sync-hyprland
}

install_app() {
  build_app
  mkdir -p "$INSTALL_BIN_DIR" "$(dirname "$INSTALL_ICON_PATH")" "$(dirname "$DESKTOP_PATH")" "$(dirname "$AUTOSTART_PATH")"

  install -m 755 "$BIN_PATH" "$INSTALL_APP_PATH"
  install -m 755 "$TRAY_BIN_PATH" "$INSTALL_TRAY_PATH"
  install -m 644 build/appicon.png "$INSTALL_ICON_PATH"

  desktop_file "$INSTALL_APP_PATH" > "$DESKTOP_PATH"
  cp "$DESKTOP_PATH" "$AUTOSTART_PATH"
  chmod 644 "$DESKTOP_PATH" "$AUTOSTART_PATH"

  write_hyprland_config
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$INSTALL_PREFIX/share/applications" >/dev/null 2>&1 || true
  fi

  echo "Installed $APP_TITLE:"
  echo "  App:       $INSTALL_APP_PATH"
  echo "  Tray:      $INSTALL_TRAY_PATH"
  echo "  Desktop:  $DESKTOP_PATH"
  echo "  Autostart: $AUTOSTART_PATH"
  echo "  Hyprland: $HYPR_SNIPPET"
}

uninstall_app() {
  stop_app
  rm -f "$INSTALL_APP_PATH" "$INSTALL_TRAY_PATH" "$INSTALL_ICON_PATH" "$DESKTOP_PATH" "$AUTOSTART_PATH"
  echo "Removed installed $APP_TITLE files."
  echo "Left Hyprland config in place: $HYPR_SNIPPET"
}

print_header() {
  clear
  echo "Kiekje"
  echo "============"
  echo
  echo "Project: $ROOT_DIR"
  echo "Binary:  $BIN_PATH"
  echo "Tray:    $TRAY_BIN_PATH"
  echo "Install: $INSTALL_APP_PATH"
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
      echo "Stopped running Kiekje processes."
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
    release)
      "$ROOT_DIR/scripts/release-prepare.sh" "${@:2}"
      ;;
    install)
      install_app
      ;;
    uninstall)
      uninstall_app
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
  7) Install    Install app, autostart, and Hyprland config
  8) Uninstall  Remove installed app files
  9) Status     Show running processes
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
      7|install) run_interactive_command install; pause ;;
      8|uninstall) run_interactive_command uninstall; pause ;;
      9|status) run_interactive_command status; pause ;;
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
  release)
    "$ROOT_DIR/scripts/release-prepare.sh" "${@:2}"
    ;;
  install)
    install_app
    ;;
  uninstall)
    uninstall_app
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

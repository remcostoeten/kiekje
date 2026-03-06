#!/usr/bin/env bash
set -euo pipefail

APP_BIN="${APP_BIN:-capture-app}"
CONFIG_PATH="${XDG_CONFIG_HOME:-$HOME/.config}/screeny/config.json"
RESOLVED_APP_BIN=""
USE_COLOR=0
C_RESET=""
C_DIM=""
C_BOLD=""
C_BLUE=""
C_GREEN=""
C_YELLOW=""
C_RED=""

init_colors() {
  if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    USE_COLOR=1
    C_RESET="$(printf '\033[0m')"
    C_DIM="$(printf '\033[2m')"
    C_BOLD="$(printf '\033[1m')"
    C_BLUE="$(printf '\033[34m')"
    C_GREEN="$(printf '\033[32m')"
    C_YELLOW="$(printf '\033[33m')"
    C_RED="$(printf '\033[31m')"
  fi
}

style() {
  local color="$1"
  local text="$2"
  if [[ "$USE_COLOR" -eq 1 ]]; then
    printf "%b%s%b" "$color" "$text" "$C_RESET"
  else
    printf "%s" "$text"
  fi
}

info() {
  echo "$(style "$C_BLUE" "[info]") $*"
}

ok() {
  echo "$(style "$C_GREEN" "[ok]") $*"
}

warn() {
  echo "$(style "$C_YELLOW" "[warn]") $*"
}

err() {
  echo "$(style "$C_RED" "[error]") $*"
}

print_header() {
  clear 2>/dev/null || true
  echo "$(style "$C_BOLD" "screeny")"
  echo "$(style "$C_DIM" "------------------------------")"
}

pause_continue() {
  echo
  read -r -p "Press Enter to continue..." _
}

ensure_config() {
  if [[ -f "$CONFIG_PATH" ]]; then
    return
  fi

  mkdir -p "$(dirname "$CONFIG_PATH")"
  cat > "$CONFIG_PATH" <<JSON
{
  "delay_ms": 0,
  "default_save_location": "$HOME/Pictures/Screenshots",
  "copy_to_clipboard": true,
  "open_editor": true,
  "default_capture_mode": "region",
  "auto_save": false,
  "filename_template": "screeny-{timestamp}-{mode}.png"
}
JSON
}

read_json_value() {
  local key="$1"
  grep -E "^[[:space:]]*\"${key}\"[[:space:]]*:" "$CONFIG_PATH" \
    | head -n1 \
    | sed -E 's/^[^:]*:[[:space:]]*//; s/[",]//g; s/[[:space:]]+$//'
}

set_json_scalar() {
  local key="$1"
  local value="$2"
  local tmp
  local is_literal="false"
  tmp="$(mktemp)"

  if [[ "$value" =~ ^(true|false|null|-?[0-9]+(\.[0-9]+)?)$ ]]; then
    is_literal="true"
  fi

  awk -v key="$key" -v value="$value" -v is_literal="$is_literal" '
    BEGIN {
      done=0
      pattern = "^[[:space:]]*\"" key "\"[[:space:]]*:"
    }
    {
      if (!done && $0 ~ pattern) {
        if (is_literal == "true") {
          print "  \"" key "\": " value ","
        } else {
          gsub(/\\/, "\\\\", value)
          gsub(/"/, "\\\"", value)
          print "  \"" key "\": \"" value "\"," 
        }
        done=1
      } else {
        print $0
      }
    }
  ' "$CONFIG_PATH" > "$tmp"

  mv "$tmp" "$CONFIG_PATH"
}

toggle_bool() {
  local key="$1"
  local current
  current="$(read_json_value "$key")"
  if [[ "$current" == "true" ]]; then
    set_json_scalar "$key" "false"
  else
    set_json_scalar "$key" "true"
  fi
}

show_settings() {
  echo "$(style "$C_BOLD" "Config:") $CONFIG_PATH"
  echo "delay_ms:             $(read_json_value delay_ms)"
  echo "copy_to_clipboard:    $(read_json_value copy_to_clipboard)"
  echo "open_editor:          $(read_json_value open_editor)"
  echo "auto_save:            $(read_json_value auto_save)"
  echo "default_capture_mode: $(read_json_value default_capture_mode)"
  echo "default_save_location: $(read_json_value default_save_location)"
  echo "filename_template:    $(read_json_value filename_template)"
}

run_capture() {
  local mode="$1"
  if ! resolve_app_bin; then
    return 1
  fi
  "$RESOLVED_APP_BIN" "$mode"
}

resolve_app_bin() {
  if [[ -n "$RESOLVED_APP_BIN" && -x "$RESOLVED_APP_BIN" ]]; then
    return 0
  fi

  if [[ -n "${APP_BIN:-}" ]]; then
    if command -v "$APP_BIN" >/dev/null 2>&1; then
      RESOLVED_APP_BIN="$(command -v "$APP_BIN")"
      return 0
    fi
    if [[ -x "$APP_BIN" ]]; then
      RESOLVED_APP_BIN="$APP_BIN"
      return 0
    fi
  fi

  if [[ -x "./target/release/capture-app" ]]; then
    RESOLVED_APP_BIN="./target/release/capture-app"
    return 0
  fi
  if [[ -x "./target/debug/capture-app" ]]; then
    RESOLVED_APP_BIN="./target/debug/capture-app"
    return 0
  fi

  err "capture-app binary not found."
  if command -v cargo >/dev/null 2>&1; then
    read -r -p "Build release binary now? [Y/n]: " build_now
    case "${build_now:-Y}" in
      n|N|no|NO)
        warn "Skipped build. You can run: cargo build --release"
        return 1
        ;;
      *)
        info "Building capture-app..."
        if cargo build --release; then
          RESOLVED_APP_BIN="./target/release/capture-app"
          ok "Build completed."
          return 0
        fi
        err "Build failed."
        return 1
        ;;
    esac
  fi

  info "Hint: APP_BIN=./target/release/capture-app ./scripts/menu.sh"
  return 1
}

bool_badge() {
  local v="$1"
  if [[ "$v" == "true" ]]; then
    style "$C_GREEN" "ON"
  else
    style "$C_RED" "OFF"
  fi
}

capture_menu() {
  while true; do
    print_header
    echo "$(style "$C_BOLD" "Capture")"
    echo "1) Region"
    echo "2) Fullscreen"
    echo "3) Window (placeholder)"
    echo "b) Back"
    echo "q) Quit"
    read -r -p "Choose: " choice

    case "$choice" in
      1|region|r) run_capture region; pause_continue ;;
      2|fullscreen|f) run_capture fullscreen; pause_continue ;;
      3|window|w) run_capture window; pause_continue ;;
      b|back) return 0 ;;
      q|quit|exit|0) exit 0 ;;
      *) warn "Unknown option"; pause_continue ;;
    esac
  done
}

settings_menu() {
  while true; do
    print_header
    echo "$(style "$C_BOLD" "Settings")"
    echo "1) Toggle clipboard copy     [$(bool_badge "$(read_json_value copy_to_clipboard)") ]"
    echo "2) Toggle open editor        [$(bool_badge "$(read_json_value open_editor)") ]"
    echo "3) Toggle auto-save          [$(bool_badge "$(read_json_value auto_save)") ]"
    echo "4) Set delay (ms)            [$(read_json_value delay_ms)]"
    echo "5) Set default mode          [$(read_json_value default_capture_mode)]"
    echo "6) Show all settings"
    echo "b) Back"
    echo "q) Quit"
    read -r -p "Choose: " choice

    case "$choice" in
      1)
        toggle_bool copy_to_clipboard
        ok "copy_to_clipboard=$(read_json_value copy_to_clipboard)"
        pause_continue
        ;;
      2)
        toggle_bool open_editor
        ok "open_editor=$(read_json_value open_editor)"
        pause_continue
        ;;
      3)
        toggle_bool auto_save
        ok "auto_save=$(read_json_value auto_save)"
        pause_continue
        ;;
      4)
        read -r -p "Delay in ms (or b to cancel): " delay
        if [[ "$delay" == "b" || "$delay" == "back" ]]; then
          continue
        fi
        if [[ "$delay" =~ ^[0-9]+$ ]]; then
          set_json_scalar delay_ms "$delay"
          ok "delay_ms=$delay"
        else
          warn "Invalid number"
        fi
        pause_continue
        ;;
      5)
        read -r -p "Mode region/fullscreen/window (or b to cancel): " mode
        case "$mode" in
          region|fullscreen|window)
            set_json_scalar default_capture_mode "$mode"
            ok "default_capture_mode=$mode"
            ;;
          b|back)
            ;;
          *)
            warn "Invalid mode"
            ;;
        esac
        pause_continue
        ;;
      6)
        show_settings
        pause_continue
        ;;
      b|back) return 0 ;;
      q|quit|exit|0) exit 0 ;;
      *) warn "Unknown option"; pause_continue ;;
    esac
  done
}

main_menu() {
  ensure_config

  while true; do
    print_header
    echo "$(style "$C_BOLD" "Main Menu")"
    echo "1) Capture"
    echo "2) Settings"
    echo "3) Show settings"
    echo "0) Exit"
    read -r -p "Choose: " choice

    case "$choice" in
      1|capture|c) capture_menu ;;
      2|settings|s) settings_menu ;;
      3) show_settings; pause_continue ;;
      0|q|quit|exit) exit 0 ;;
      *) warn "Unknown option"; pause_continue ;;
    esac
  done
}

init_colors
main_menu

# screeny (`capture-app`)

Fast Wayland-first screenshot utility for Linux (Hyprland-first), with a minimal GTK4/libadwaita annotation editor.

## Features (MVP)

- Capture modes:
  - `region` (via `slurp` + `grim`)
  - `fullscreen` (via `grim`)
  - `window` (active Hyprland window via `hyprctl` + `grim`)
- Editor tools:
  - rectangle
  - arrow
  - freehand draw
  - text (basic)
  - highlight
  - undo
  - clear
- Clipboard:
  - auto-copy PNG to Wayland clipboard via `wl-copy`
- Saving:
  - configurable default save location
  - filename template with `{timestamp}` and `{mode}`
  - optional auto-save
- Settings persistence:
  - JSON config in `~/.config/screeny/config.json` (or `$XDG_CONFIG_HOME/screeny/config.json`)
- CLI:
  - `capture-app region`
  - `capture-app fullscreen`
  - `capture-app window`
  - `capture-app --interactive` (menu-driven)

## Requirements

- Linux Wayland session
- Hyprland (target compositor, but works with generic Wayland tools)
- Installed tools:
  - `grim`
  - `slurp`
  - `wl-copy`
  - `hyprctl` (required for `window` capture mode)
- GTK dependencies for build/runtime:
  - `gtk4`
  - `libadwaita-1`

Example (Arch):

```bash
sudo pacman -S grim slurp wl-clipboard hyprland gtk4 libadwaita
```

## Build

```bash
cargo build
```

Release build:

```bash
cargo build --release
```

Binary:

```bash
./target/release/capture-app
```

## Usage

```bash
capture-app region
capture-app fullscreen
capture-app window
capture-app --interactive
```

If no mode is provided, the app uses `default_capture_mode` from config.

Interactive mode provides a small menu to:
- run capture commands
- toggle clipboard/editor/auto-save
- set capture delay
- set default capture mode
- inspect current settings

Small Bash menu (requested):

```bash
./scripts/menu.sh
```

If `capture-app` is not in `PATH`, set:

```bash
APP_BIN=./target/release/capture-app ./scripts/menu.sh
```

Navigation in the Bash menu:
- `b` / `back` returns to previous menu
- `q` / `quit` / `0` exits
- Prompts for delay/mode support `b` to cancel
- Colorized output is enabled on TTY by default; disable with `NO_COLOR=1 ./scripts/menu.sh`

Bubble Tea TUI (Go):

```bash
go mod tidy
go build -o ./bin/screeny-tui ./cmd/screeny-tui
./bin/screeny-tui
```

Controls:
- `j`/`k` or arrow keys to navigate
- `Enter` to select
- `b` or `Esc` to go back
- `q` to quit

Notes:
- TUI resolves `capture-app` using `APP_BIN`, `PATH`, `./target/release/capture-app`, then `./target/debug/capture-app`.
- If not found, it attempts `cargo build --release` automatically.

## Hyprland Keybind Example

In `~/.config/hypr/hyprland.conf`:

```ini
# Region capture + editor
bind = SUPER SHIFT, S, exec, capture-app region

# Fullscreen capture
bind = SUPER SHIFT, F, exec, capture-app fullscreen
```

## Config

Path: `~/.config/screeny/config.json`

Default config shape:

```json
{
  "delay_ms": 0,
  "default_save_location": "/home/user/Pictures/Screenshots",
  "copy_to_clipboard": true,
  "open_editor": true,
  "default_capture_mode": "region",
  "auto_save": false,
  "filename_template": "screeny-{timestamp}-{mode}.png"
}
```

## Architecture

- `src/main.rs`: CLI entrypoint and flow orchestration
- `src/capture`: capture mode dispatch and mode handlers
- `src/platform/linux`: `grim`/`slurp` process wrappers
- `src/editor`: annotation tools and canvas rendering/export
- `src/clipboard`: `wl-copy` integration
- `src/storage`: save path rendering and PNG writes
- `src/settings`: persisted config model and load/save
- `src/app`: GTK/libadwaita window shell

## Future Backends (macOS/Windows)

- Introduce trait-based backend abstraction:
  - `CaptureBackend` with `capture_region/fullscreen/window`
- macOS backend options:
  - `screencapture` CLI for MVP
  - later native APIs (ScreenCaptureKit)
- Windows backend options:
  - PowerShell/WinRT tool for MVP
  - later DXGI Desktop Duplication / Windows Graphics Capture
- Keep editor and storage layers platform-independent.

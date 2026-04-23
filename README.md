# Kiekje

[![Version](https://img.shields.io/badge/version-0.0.1-black)](https://github.com/remcostoeten/kiekje)
[![Platform](https://img.shields.io/badge/platform-Linux%20Wayland-black)](https://github.com/remcostoeten/kiekje)
[![Desktop](https://img.shields.io/badge/desktop-Hyprland%20friendly-black)](https://github.com/remcostoeten/kiekje)
[![License](https://img.shields.io/badge/license-MIT-black)](./LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-black)](https://www.rust-lang.org/)

**Kiekje** *(noun)*  
/ˈkik.jə/ — *Dutch slang, “a quick snapshot or photo.”*

Kiekje is a fast Wayland screenshot tool for Linux with annotation, tray controls, save automation, and Hyprland-friendly shortcut workflows.

Current binary names: `kiekje` and `kiekje-tui`

## Status and planning docs

Use the status hub for current product state, Linux support boundaries, and
overhaul planning.

- [Project status hub](./docs/status/README.md)
- [Current work tracker](./docs/status/current-status.md)
- [Pre-Tauri scope](./docs/status/overhaul/pre-tauri-scope.md)
- [Tauri implementation plan](./docs/status/overhaul/tauri-implementation.md)

## Feature List

- Region, fullscreen, and active-window capture
- GTK4/libadwaita annotation editor with rectangle, arrow, pen, text, and highlight tools
- Tray icon with capture actions, delay presets, and shared toggles
- Launcher with tray autostart and Hyprland shortcut setup
- Clipboard copy, auto-save, filename templates, and default save folder support
- Save As, undo/redo, selection handles, color presets, and annotation sizing
- Dependency doctor and recovery-oriented error handling

## Features (MVP)

- Capture modes:
  - `region` (via in-app area selector + `grim`)
  - `fullscreen` (via `grim`)
  - `window` (active Hyprland window via `hyprctl` + `grim`)
- Editor tools:
  - rectangle
  - arrow
  - freehand draw
  - text
  - highlight
  - undo
  - redo
  - clear
- Editor accessibility:
  - keyboard shortcuts for tool selection, save, undo, clear, close
  - visible shortcut/status help
  - scrollable canvas for large screenshots
  - text entry field for placed annotations
- Editor controls:
  - `Save As` button
  - unsaved-changes warning on close
  - annotation selection and resize handles
  - explicit annotated-image copy
  - default save folder picker
  - close-after-copy toggle
  - open-after-save toggle
  - right-click color picker
  - scroll to change annotation size
- Region capture UX:
  - dimmed in-app overlay with higher-contrast selection border
  - `Fullscreen` action available while choosing an area
- Clipboard:
  - auto-copy PNG to Wayland clipboard via `wl-copy`
- Saving:
  - configurable default save location
  - filename template with `{timestamp}` and `{mode}`
  - optional auto-save
- Settings persistence:
  - JSON config in `~/.config/kiekje/config.json` (or `$XDG_CONFIG_HOME/kiekje/config.json`)
- CLI:
  - `kiekje region`
  - `kiekje fullscreen`
  - `kiekje window`
  - `kiekje --launcher`
  - `kiekje --tray`
  - `kiekje --doctor`
  - `kiekje --interactive` (menu-driven)
- Desktop integration:
  - StatusNotifier tray icon via `kiekje --tray`
  - launcher toggle to start the tray on login
  - recordable Hyprland shortcut assignments for region/fullscreen/window capture
  - generated Hyprland include file plus reload action from the launcher

## Requirements

- Linux Wayland session
- Hyprland (target compositor, but works with generic Wayland tools)
- Installed tools:
  - `grim`
  - `wl-copy`
  - `hyprctl` (required for `window` capture mode)
- GTK dependencies for build/runtime:
  - `gtk4`
  - `libadwaita-1`

Example (Arch):

```bash
sudo pacman -S grim wl-clipboard hyprland gtk4 libadwaita
```

## Install

Fastest for users:

```bash
curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/main/scripts/install-release.sh | bash
```

Install and attempt runtime dependencies too:

```bash
curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/main/scripts/install-release.sh | bash -s -- --install-deps
```

Install a specific release:

```bash
curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/main/scripts/install-release.sh | bash -s -- --tag v0.0.1
```

Local install into `~/.local`:

```bash
scripts/install.sh
```

System-wide install into `/usr/local`:

```bash
sudo scripts/install.sh --prefix /usr/local
```

Manual source install:

```bash
cargo build --release
install -Dm755 target/release/kiekje ~/.local/bin/kiekje
```

Optional helper binary:

```bash
cd cmd/kiekje-tui
go build -o ../../kiekje-tui .
install -Dm755 ../../kiekje-tui ~/.local/bin/kiekje-tui
```

Desktop launcher:

- `scripts/install.sh` also installs
  [share/applications/kiekje.desktop](./share/applications/kiekje.desktop).
- If `~/.local/bin` is not in `PATH`, add it before using the desktop entry.
- Tagged GitHub releases now publish a `kiekje-linux-x86_64.tar.gz` bundle with `install.sh` and `uninstall.sh` included.
- Installers check for `grim`, `wl-copy`, and `hyprctl`, print exact guidance, and support `--install-deps` for core packages on `pacman`, `apt-get`, `dnf`, and `zypper`.

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
./target/release/kiekje
```

## Usage

```bash
kiekje region
kiekje fullscreen
kiekje window
kiekje --launcher
kiekje --tray
kiekje --doctor
kiekje --interactive
```

## Error Handling and Recovery

- The app validates required tools before running capture flows.
- Missing dependencies return a structured error with:
  - error code
  - what is missing and why
  - install command suggestions (when package manager is detected)
  - workaround options (for example, fallback capture modes or disabling clipboard)
- In `--interactive` mode, dependency failures include recovery actions:
  - disable clipboard and retry
  - fallback from `window` to `fullscreen`/`region`
  - fallback from `region` to `fullscreen`
  - run install commands directly from the prompt
- Go TUI (`kiekje-tui`) applies automatic recovery for common cases:
  - disable clipboard when `wl-copy` is missing
  - fallback to fullscreen when mode-specific tools are missing
  - show install hints when manual installation is required
- GUI launcher (`kiekje --launcher`) exposes the same core capture actions, delay presets, and shared settings in a single GTK window.
- Tray mode (`kiekje --tray`) adds a persistent StatusNotifier item with capture actions, delay presets, toggles, and a doctor shortcut.
- Launcher desktop integration controls can write `~/.config/autostart/kiekje-tray.desktop` and generate `~/.config/hypr/kiekje-shortcuts.conf`.
- Use `kiekje --doctor` to print a full dependency readiness report.

If no mode is provided, the app uses `default_capture_mode` from config.

Interactive mode provides a small menu to:
- run capture commands
- toggle clipboard/editor/auto-save
- set capture delay
- set default capture mode
- inspect current settings

Bubble Tea TUI (Go):

```bash
cd cmd/kiekje-tui
go mod tidy
go build -o ../../bin/kiekje-tui .
cd ../..
./bin/kiekje-tui
```

Controls:
- `j`/`k` or arrow keys to navigate
- `Enter` to select
- `b` or `Esc` to go back
- `q` to quit

Notes:
- TUI resolves `kiekje` using `APP_BIN`, `PATH`, `./target/release/kiekje`, then `./target/debug/kiekje`.
- If not found, it attempts `cargo build --release` automatically.

Tray autostart and Hyprland shortcut setup:

- Open `kiekje --launcher`.
- Enable `Start Tray on Login` to create the autostart desktop entry.
- Use the `Record` buttons under `Hyprland Shortcuts` to assign per-mode shortcuts.
- Click `Install Hyprland Include`, then add the shown `source = ...` line to your Hyprland config once if needed.
- Click `Reload Hyprland` to apply the updated shortcut include immediately.

## Hyprland Keybind Example

In `~/.config/hypr/hyprland.conf`:

```ini
# Region capture + editor
bind = SUPER SHIFT, S, exec, kiekje region

# Fullscreen capture
bind = SUPER SHIFT, F, exec, kiekje fullscreen
```

## Config

Path: `~/.config/kiekje/config.json`

Default config shape:

```json
{
  "delay_ms": 0,
  "default_save_location": "/home/user/Pictures/Screenshots",
  "copy_to_clipboard": true,
  "close_after_copy": false,
  "open_after_save": false,
  "open_editor": true,
  "default_capture_mode": "region",
  "auto_save": false,
  "tray_autostart": false,
  "shortcut_region": "SUPER SHIFT, S",
  "shortcut_fullscreen": "SUPER SHIFT, F",
  "shortcut_window": "SUPER SHIFT, W",
  "filename_template": "kiekje-{timestamp}-{mode}.png"
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

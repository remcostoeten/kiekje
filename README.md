# Cheese Wails

Single-window capture and annotation tool for Hyprland and other Wayland desktops.

## Run

```bash
wails dev
```

## Build

```bash
wails build
```

## Install

```bash
./cheese.sh install
```

This installs the app to `~/.local/bin/kiekje`, installs the tray sidecar beside it, adds a desktop launcher, enables autostart, installs the icon, and writes a managed Hyprland snippet at `~/.config/hypr/cheese-bindings.conf`.

To remove the installed app files:

```bash
./cheese.sh uninstall
```

## Hyprland floating rule

The installer writes this automatically and sources it from `~/.config/hypr/hyprland.conf`:

```ini
windowrule = float on, match:title ^(Cheese)$
windowrule = pin on, match:title ^(Cheese)$
windowrule = center on, match:title ^(Cheese)$
bind = CTRL, C, exec, "~/.local/bin/kiekje" --capture
```

If you want the app to stay above tiled windows, keep `AlwaysOnTop` enabled in `main.go`.

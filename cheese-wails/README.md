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

## Troubleshooting

### WebKitWebProcess crashed (Arch / Wayland)

If you see **"WebKitWebProcess has encountered an error and closed"** (sometimes twice — WebKit runs multiple helper processes):

1. Rebuild after pulling — the app sets `WebviewGpuPolicyNever` and `WEBKIT_DISABLE_DMABUF_RENDERER=1` to avoid a known WebKitGTK + transparent-window crash.
2. Make sure only one Cheese instance is running: `pkill -x kiekje; pkill -x cheese-wails` then start again.
3. On NVIDIA, ensure modesetting is on: `cat /sys/module/nvidia_drm/parameters/modeset` should print `Y`. If not, enable `nvidia_drm modeset=1` and reboot.

Capture mode no longer forces a fully transparent native window background; the dim overlay is drawn on the canvas instead.

# Kiekje

Single-window capture and annotation tool for Hyprland and other Wayland desktops.

Author: Remco Stoeten

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
./kiekje.sh install
```

This installs the app to `~/.local/bin/kiekje`, installs the tray sidecar beside it, adds a desktop launcher, enables autostart, installs the icon, and syncs Hyprland config (writes `~/.config/hypr/kiekje-bindings.conf` and adds the `source` line to `hyprland.conf` automatically).

Hyprland sync also runs on every app start and whenever capture binds change. To run it manually:

```bash
kiekje --sync-hyprland
```

For tarball installs, extract the archive and run `./install.sh` (uses the same sync path).

## Releases

Kiekje uses semantic versioning and [git-cliff](https://git-cliff.org) for changelog-driven GitHub releases. See [docs/RELEASING.md](docs/RELEASING.md) for the full maintainer workflow.

```bash
./scripts/release-prepare.sh prepare --bump auto   # local tag + changelog
git push origin HEAD v0.1.0                        # triggers release build
```

Install git-cliff once: `cargo install git-cliff --locked`

To remove the installed app files:

```bash
./kiekje.sh uninstall
```

## Hyprland floating rule

The app manages `~/.config/hypr/kiekje-bindings.conf` and ensures this line exists in `~/.config/hypr/hyprland.conf`:

```ini
source = ./kiekje-bindings.conf # Kiekje capture shortcuts
```

Legacy `cheese-bindings.conf` source lines are removed automatically during sync. The snippet contains:
```ini
windowrule = float on, match:title ^(Kiekje)$
windowrule = no_initial_focus on, match:title ^(Kiekje)$
windowrule = workspace current silent, match:title ^(Kiekje)$
windowrule = center on, match:title ^(Kiekje)$
bind = CTRL, C, exec, "~/.local/bin/kiekje" --capture
```

If you want the app to stay above tiled windows, keep `AlwaysOnTop` enabled in `main.go`.

### WebKitWebProcess crashed (Arch / Wayland)

If you see **"WebKitWebProcess has encountered an error and closed"** (sometimes twice — WebKit runs multiple helper processes):

1. Rebuild after pulling — the app sets `WebviewGpuPolicyNever` and `WEBKIT_DISABLE_DMABUF_RENDERER=1` to avoid a known WebKitGTK + transparent-window crash.
2. Make sure only one Kiekje instance is running: `pkill -x kiekje; pkill -x kiekje-tray` then start again.
3. On NVIDIA, ensure modesetting is on: `cat /sys/module/nvidia_drm/parameters/modeset` should print `Y`. If not, enable `nvidia_drm modeset=1` and reboot.

Capture mode no longer forces a fully transparent native window background; the dim overlay is drawn on the canvas instead.

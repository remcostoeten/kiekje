---
title: Desktops
description: What Kiekje uses on KDE Plasma, Hyprland and Omarchy, Sway, GNOME and X11, and what each one needs from you.
---

# Desktops

Kiekje picks its backends at runtime from the Wayland registry, D-Bus names, portal interface versions and session variables such as `XDG_SESSION_TYPE` and `HYPRLAND_INSTANCE_SIGNATURE`. It never guesses from which binaries are installed. Run `kiekje doctor` to see the result for your session.

| Desktop | Capture backend | Overlay | Global shortcuts | Status |
| --- | --- | --- | --- | --- |
| KDE Plasma 6, Wayland | KWin `ScreenShot2` over D-Bus | layer-shell | GlobalShortcuts portal, kglobalaccel | Lab tested on Plasma 6.7.5 in a VM |
| Hyprland, Omarchy | `ext-image-copy-capture`, then `wlr-screencopy` | layer-shell | Managed `hyprland.conf` include, GlobalShortcuts portal | Lab tested on Hyprland 0.56.2 in a VM. Omarchy unverified |
| Sway and other wlroots compositors | `ext-image-copy-capture`, then `wlr-screencopy` | layer-shell | GlobalShortcuts portal where the portal offers it | Lab tested on headless sway 1.9 and 1.12 |
| GNOME, Wayland | xdg-desktop-portal Screenshot | fullscreen window | GlobalShortcuts portal | Lab tested on GNOME 50.5 in a VM. Shortcut keys and overlay focus unverified |
| KDE Plasma, X11 | KWin `ScreenShot2` | fullscreen window | kglobalaccel | Unverified |

Other X11 sessions have no capture backend. `doctor` reports it as unavailable.

## KDE Plasma

- **Setup.** Run `kiekje install-desktop-file` once. KWin grants `org.kde.KWin.ScreenShot2` to any executable whose installed desktop file lists `X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2` and whose `Exec` points at it. Without the file KWin answers `NoAuthorized` and Kiekje falls back to the portal, which is slower and shows a dialog.
- **Capture.** ScreenShot2 returns raw premultiplied BGRA over a pipe, so the overlay shows the frame with no conversion. One 4K frame took 36 to 57 ms in the lab. `native-resolution` gives the physical pixels at fractional scale.
- **Shortcuts.** Registered through the GlobalShortcuts portal. KDE shows its own confirmation dialog the first time.
- **Clipboard.** KWin exposes `ext_data_control_manager_v1`, so the background daemon can set the clipboard without focus, and Klipper keeps the copy after Kiekje quits.
- **Window capture and snapping.** KWin scripting supplies the window geometry.

## Hyprland and Omarchy

- **Setup.** The daemon writes its binds to a managed include at `~/.config/kiekje/hyprland.conf` and only rewrites it when the content changes. Add the `source` line that `kiekje doctor` prints to `~/.config/hypr/hyprland.conf`, or to any file it sources up to four levels deep. Until then `doctor` reports `hyprland binds: not sourced`.
- **Capture.** `ext-image-copy-capture-v1` when the compositor offers it, `wlr-screencopy` otherwise. Window capture uses the compositor's window list for geometry.
- **Shortcuts.** The binds in the include dispatch `global` shortcuts that the Hyprland portal registers. The portal ignores preferred triggers, so the include is what sets the keys.
- **Tray.** Needs a tray module in Waybar. `doctor` points at `~/.config/waybar/config.jsonc` when no tray host is running.
- **Unverified.** Whether Hyprland reloads the include on change, and whether Omarchy's own Print bind fires alongside it.

## Sway and other wlroots compositors

- **Capture.** The same screencopy backends as Hyprland. No window capture: sway reports no window geometry Kiekje can use.
- **Shortcuts.** Bind the CLI in your sway config:

  ```text
  bindsym Print exec kiekje capture
  bindsym Shift+Print exec kiekje capture fullscreen
  ```

- **Tray.** Waybar or swaybar with a tray module.
- **Portal.** On Ubuntu with sway, `doctor` reports the Screenshot portal as unavailable even with `xdg-desktop-portal-wlr` installed. Capture still works through screencopy.

## GNOME

- **Capture.** The Screenshot portal. The first capture shows a permission dialog; accept it once and it persists. The portal call has a 30 second timeout so an unanswered dialog can't block later captures.
- **Overlay.** GNOME has no layer-shell, so the overlay is a fullscreen window per monitor. Focus and stacking above other fullscreen apps are not guaranteed.
- **Shortcuts.** The GlobalShortcuts portal on GNOME 48 or newer. The keys themselves are unverified.
- **Tray.** Needs the AppIndicator extension. Ubuntu ships it, Fedora does not.
- **Pins.** `doctor` says `pins: on top not guaranteed`.
- **Not available.** Window capture and window snapping.

## X11

Plasma on X11 keeps the KWin backend and uses kglobalaccel for shortcuts. The overlay is a fullscreen window. Other X11 sessions have no capture path; an `x11rb` backend with MIT-SHM is on hold.

## Flatpak

The Flatpak bundle uses the portals for capture and global shortcuts on every desktop. KWin ScreenShot2 and the direct Hyprland protocols stay unavailable from the sandbox until the required portal behavior has been verified on real desktops.

## Clipboard managers

Kiekje owns the clipboard while it runs. After Quit the image survives only if a clipboard manager copied it. Settings and `kiekje doctor` say whether Klipper, GPaste, CopyQ or cliphist is running, and warn when none is.

## Tray hosts

| Host | Left click | Right click |
| --- | --- | --- |
| Plasma | Kiekje's styled popover | Native menu |
| Waybar | Popover, positioned from the click when Waybar sends coordinates | Native menu |
| GNOME with AppIndicator | Popover as a small window, placed by GNOME | Native menu |

The native menu is drawn by the tray host in the system theme and carries the same actions. It is the accessible fallback and what hosts without click events use.

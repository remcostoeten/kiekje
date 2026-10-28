---
title: Troubleshooting
description: Read the doctor report and fix the common gaps.
---

# Troubleshooting

Start with the doctor:

```bash
kiekje doctor
```

It prints one line per capability, then every problem with its fix. Nothing is detected from installed binaries; each line comes from the Wayland registry, D-Bus or the file system. The same report sits on the About page in Settings.

| Line | What it says |
| --- | --- |
| `type`, `desktop` | The session type and desktop Kiekje detected |
| capture backends | Each backend with a reason when it is unavailable, such as `no Screenshot portal` |
| `overlay` | layer-shell or fullscreen window |
| `pins` | `kept on top`, or `on top not guaranteed` on GNOME |
| `window snap`, `window capture` | The geometry source, or why there is none |
| `daemon`, `tray` | Whether the daemon runs and which tray host it found |
| `clipboard` | The clipboard manager, or `none found, copies are lost when Kiekje quits` |
| `recording` | `ScreenCast portal, WebM`, or what is missing |
| `kwin access` | Whether the installed desktop file authorises ScreenShot2 |
| `hyprland binds` | `sourced` or `not sourced`, with the line to add |
| `config` | The path, and whether it parsed |
| `fonts` | Whether the bundled Geist fonts were found |

## Common entries

**`kwin access` fails or captures fall back to the portal on Plasma.** Run `kiekje install-desktop-file`. If a desktop file with another `Exec` exists, doctor names it: `points at another binary`. Remove or fix that file.

**`hyprland binds: not sourced`.** Add the printed `source = ~/.config/kiekje/hyprland.conf` line to your Hyprland config. The daemon keeps the include up to date.

**`No capture backend is available`.** You are on an X11 session without KWin, or on a Wayland compositor without screencopy and without a Screenshot portal. Install your desktop's `xdg-desktop-portal-*` backend, or on wlroots compositors check that the compositor offers `ext-image-copy-capture` or `wlr-screencopy`.

**`tray: no tray host`.** Enable a tray module in Waybar (`doctor` points at the config file) or install the AppIndicator extension on GNOME.

**`clipboard: none found`.** Install a clipboard manager, or keep the daemon running. Captures are also always written to disk unless saving is off.

**`recording: unavailable`.** Install the GStreamer PipeWire plugin and the good plugins, and a ScreenCast portal backend. The notification after a failed recording names the package.

**`config: invalid`.** Defaults apply until the file is fixed. The message names the token or key the parser rejected. Delete the file to start over; Kiekje writes a fresh default.

**`fonts: missing, system font used`.** The UI falls back to the system font. Packages install the fonts to `/usr/share/kiekje/fonts`; a source build finds them in the repository.

## Other problems

**No shortcut fires.** On GNOME and Plasma the GlobalShortcuts portal may have shown a confirmation dialog once. Open Settings, Shortcuts to see the registration state (`active`, `needs system dialog`, `conflict`, `unavailable`). On sway, bind the CLI yourself.

**A capture is dropped as "already running".** One capture is in flight at a time. On GNOME an unanswered portal dialog held the previous one; the portal call now times out after 30 seconds.

**Ubuntu 24.04 or Linux Mint 22 looks unstyled.** Fixed in 0.1.0-alpha.5. The stylesheet is flattened to plain values at build time so GTK 4.14 renders it.

**Nothing in the clipboard after a fullscreen capture from the tray on Plasma.** The daemon sets the clipboard through `ext_data_control_manager_v1`; on compositors without it a capture started without focus cannot reach the clipboard. The file is still saved.

## Reporting a bug

Open an issue with the bug report form on [GitHub](https://github.com/remcostoeten/kiekje/issues). Include the output of `kiekje doctor`, your desktop and its version, and the steps that trigger the bug. Run `kiekje daemon` from a terminal to capture its log; `KIEKJE_LOG=debug kiekje daemon` adds detail.

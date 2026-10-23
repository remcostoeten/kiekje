---
title: Support matrix
description: Which desktops, distributions, packages and features are lab tested, unit tested or unverified.
---

# Support matrix

**Lab tested** means it passed in a virtual machine or a headless session in the [lab](/project/lab-results). **Unit tested** means the logic has tests but the behavior has not been seen on a desktop. **Unverified** means the code path exists and has not run on that desktop. Nothing has been run on physical hardware yet; that is [issue #41](https://github.com/remcostoeten/kiekje/issues/41).

## Desktops

| Desktop | Capture backend | Overlay | Global shortcuts | Status |
| --- | --- | --- | --- | --- |
| KDE Plasma 6, Wayland | KWin `ScreenShot2` over D-Bus | layer-shell | GlobalShortcuts portal, kglobalaccel | Lab tested on Plasma 6.7.5 in a VM |
| Hyprland, Omarchy | `ext-image-copy-capture`, then `wlr-screencopy` | layer-shell | Managed `hyprland.conf` include, GlobalShortcuts portal | Lab tested on Hyprland 0.56.2 in a VM. Omarchy unverified |
| Sway and other wlroots compositors | `ext-image-copy-capture`, then `wlr-screencopy` | layer-shell | GlobalShortcuts portal where the portal offers it | Lab tested on headless sway 1.9 and 1.12 |
| GNOME, Wayland | xdg-desktop-portal Screenshot | fullscreen window | GlobalShortcuts portal | Lab tested on GNOME 50.5 in a VM. Shortcut keys and overlay focus unverified |
| KDE Plasma, X11 | KWin `ScreenShot2` | fullscreen window | kglobalaccel | Unverified |

## Distributions

The release binary links gtk4-layer-shell statically. CI installs the packages and starts the binary on each distribution below before a release ships. Started means the binary launches and opens its surfaces, not that a desktop was clicked through.

| Distribution | x86_64 | aarch64 | Styling |
| --- | --- | --- | --- |
| Arch Linux | yes | not built | full |
| Fedora | yes | yes | full |
| openSUSE Tumbleweed | yes | not built | full |
| Debian 13 | yes | yes | full |
| Ubuntu 25.10 | yes | not built | full |
| Ubuntu 24.04 | yes | yes | full |

Linux Mint 22 shares Ubuntu 24.04's GTK 4.14 and behaves the same, but is not in CI.

## Not supported

| What | Status | Notes |
| --- | --- | --- |
| X11 capture backend | On hold | Pure X11 sessions other than Plasma have no capture path. `doctor` reports it as unavailable |
| Window capture on GNOME, Sway and X11 | Not built | Needs the compositor to report window geometry, which only Plasma 6 and Hyprland do |
| KWin ScreenShot2 and Hyprland protocols from the Flatpak | Not yet | The sandboxed build uses the portals only |
| macOS, Windows | Not planned | Kiekje is Linux only |
| Text recognition (OCR) | Post-V1 | Planned with Tesseract |
| Real hardware | Unverified | No run on physical machines with Omarchy, Plasma or GNOME yet |

## Features

### Capture

| Feature | Details | Status |
| --- | --- | --- |
| Frozen-frame selection | The screen is grabbed at trigger time; you select on that still image | Lab tested |
| Region capture | Drag to select, release to copy and save | Lab tested |
| Full screen | Every monitor, stitched into one image | Lab tested |
| Window capture | The window under the pointer lights up and a click or Enter takes it. Crops the frozen frame | Plasma 6 and Hyprland, unverified |
| Repeat last region | Crops the last selected area again without the overlay | Lab tested on sway |
| Delay | `--delay` up to 60 seconds | Lab tested on sway |
| Multi-monitor | Selections can cross monitors and mixed scales, cropped at native resolution | Lab tested on Plasma, sway, Hyprland |
| Fractional scaling and rotation | Mapped back to physical pixels | Lab tested on sway and Hyprland virtual outputs |
| Window snapping | The window under the pointer lights up; click to take it | Plasma 6 and Hyprland, unverified |
| Pixel loupe | Magnifier with position and hex colour under the pointer | Unverified on desktops |
| Colour picker | C copies the hex colour; picked colours are listed in the gallery | Unverified on desktops |
| Hint bar | Lists the overlay keys; ? hides it | Lab tested on sway |
| Monitor unplug | Disconnecting a monitor during a selection closes the overlay and cancels the capture with one notification | Unit tested, unverified on desktops |

### Annotation

| Feature | Details | Status |
| --- | --- | --- |
| Inline editor | Draw on the selection right on the overlay with Accept, Cancel, Save, Save as and Copy | Lab tested on sway |
| Editor window | Opens from the gallery or straight after a capture | Lab tested on sway |
| Tools | Select, box, ellipse, arrow, line, pen, marker, numbered steps, text, pixelate, crop | Lab tested on sway |
| Styling | 7 colours, size 1 to 24 px, opacity 10 to 100%, remembered between editors | Lab tested on sway |
| Edit shapes | Move, resize, nudge, restyle and delete placed shapes | Lab tested on sway |
| Right-click menu | Every tool, line size, undo, redo and delete | Lab tested on sway |
| Rebindable keys | Every tool and thinner or thicker lines | Lab tested on sway |
| Undo and redo | Every change is one undoable step | Unit tested |
| Non-destructive | Saving an edit writes a new `-edited` file; the original stays | Lab tested on sway |

### Output and library

| Feature | Details | Status |
| --- | --- | --- |
| Clipboard | PNG on the clipboard, also from the background daemon on Plasma | Lab tested |
| Save to disk | PNG or JPEG in `~/Pictures/Kiekje`, exclusive create, never overwrites | Lab tested |
| Unsaved captures | With saving off, captures are kept in the cache under an Unsaved badge with a Keep action, pruned after 7 days | Unit tested, unverified on desktops |
| Filename templates | Tokens with a live preview in Settings | Unit and lab tested |
| Pin on screen | Keep a capture floating above other windows; drag, scroll to zoom | Unverified on desktops |
| Notifications | With the image, Open and Show in folder actions | Lab tested |
| Capture sound | Sound theme shutter or the bundled one, needs the GStreamer base plugins | Lab tested |
| Gallery | Grid and list views, search, sorting, and filters for kind, period, dimensions, file size, annotated and unsaved | Filters unit tested, unverified on desktops |
| File actions | Open, show in folder, copy image, copy path, trash, remove, edit, pin, rename | Lab tested on sway |
| Screen recording | `kiekje record` through the ScreenCast portal, WebM (VP8) to `~/Videos/Kiekje` | Pipeline lab tested, unverified on desktops |
| Temporary upload | Off by default. Upload and copy link sends the file to Litterbox with a 1 to 72 hour expiry | Unit tested, unverified on desktops |
| Missing files | Detects moved or deleted files and lets you relink them | Lab tested on sway |
| Large libraries | Thumbnails load only for visible items; 10,000 captures tested | Lab tested on sway |

### App

| Feature | Details | Status |
| --- | --- | --- |
| Tray | Styled popover on left click, native menu on right click | Lab tested on Plasma. Waybar and GNOME unverified |
| Global shortcuts | Print for a region, Shift+Print for every screen, a window shortcut without a default key, rebindable with a recorder | Lab tested on Plasma and Hyprland. The window shortcut is unverified |
| Settings | Capture, Shortcuts, Saving, General and About pages | Lab tested on sway |
| Start at login | Toggle in Settings | Unverified |
| Doctor | Reports what the session supports and how to fix the rest | Lab tested |
| Update check | Off by default. Asks GitHub at startup and at most once a day; Check now in About | Unverified on desktops |
| Clipboard manager notice | Settings and `kiekje doctor` say whether a clipboard manager is running | Unverified on desktops |
| Danger zone | Reset settings, clear gallery, colours or thumbnails, with a two-click guard | Lab tested on sway |
| Single instance | CLI commands talk to the running daemon over D-Bus | Lab tested |

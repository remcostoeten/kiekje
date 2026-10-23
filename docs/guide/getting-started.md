---
title: Getting started
description: Start the daemon, finish the one-time setup for your desktop and take the first capture.
---

# Getting started

## 1. Start the daemon

```bash
kiekje daemon
```

The daemon owns the tray icon, the global shortcuts, the clipboard and the capture pipeline. Turn on **Start Kiekje at login** in Settings, General, to have it start with your session. Closing a window never quits it. **Quit** in the tray or `kiekje quit` does.

## 2. One-time setup per desktop

::: code-group

```bash [KDE Plasma]
kiekje install-desktop-file
```

```bash [Hyprland, Omarchy]
# Add the line that `kiekje doctor` prints to ~/.config/hypr/hyprland.conf:
source = ~/.config/kiekje/hyprland.conf
```

```text [GNOME]
Install the AppIndicator extension for the tray icon.
The first capture shows a portal permission dialog; accept it once.
```

```text [Sway]
Bind the CLI in your sway config:
bindsym Print exec kiekje capture
bindsym Shift+Print exec kiekje capture fullscreen
```

:::

KWin only allows screenshots from an app whose installed desktop file lists `X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2`, and `install-desktop-file` writes exactly that file with the right `Exec`. On Hyprland the daemon writes its binds to a managed include and `doctor` reports `hyprland binds: not sourced` with the line to add until it is sourced. The [desktops page](/guide/desktops) has the details for each compositor.

## 3. Take a capture

Press <kbd>Print</kbd>, drag, let go. The region is on your clipboard and in `~/Pictures/Kiekje`. A notification offers Open and Show in folder.

Other ways to start one:

| Trigger | What it does |
| --- | --- |
| <kbd>Shift</kbd>+<kbd>Print</kbd> | Every monitor, stitched into one image |
| Left click the tray icon | The capture menu with every mode and the recent captures |
| Right click the tray icon | The native menu with the same actions |
| `kiekje capture --delay 5` | Wait 5 seconds before freezing the screen |
| `kiekje capture --edit` | Annotate the region on the overlay before keeping it |

Before you drag, <kbd>F</kbd> takes the monitor under the pointer, <kbd>W</kbd> or <kbd>Enter</kbd> the hovered window, <kbd>Shift</kbd>+<kbd>R</kbd> the last region and <kbd>Esc</kbd> cancels. A hint bar at the bottom lists these; <kbd>?</kbd> hides it. The full list is under [Keys](/reference/keys).

## 4. Check what your session supports

```bash
kiekje doctor
```

The report names the capture backend, the overlay type, the shortcut host, the tray host, the clipboard manager and the recording pipeline, and prints a fix for each gap. The same report sits on the About page in Settings. [Troubleshooting](/guide/troubleshooting) walks through the common entries.

## Where things land

| Data | Location |
| --- | --- |
| Captures | `~/Pictures/Kiekje`, configurable |
| Recordings | `~/Videos/Kiekje` |
| Settings | `$XDG_CONFIG_HOME/kiekje/config.toml` |
| Gallery index | `$XDG_DATA_HOME/kiekje` |
| Thumbnails and unsaved captures | `$XDG_CACHE_HOME/kiekje` |

Two settings people change first:

- `[capture] after_selection = "edit"` opens the inline editor after every selection, so you can draw before the capture is copied and saved.
- `[saving] enabled = false` only copies captures. They still appear in the gallery as Unsaved for 7 days, and Keep moves one to the capture folder.

Every key is listed in the [configuration reference](/reference/configuration).

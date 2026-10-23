---
title: Command line
description: Every kiekje command, capture mode and flag.
---

# Command line

```text
kiekje <command> [...flags]
```

One instance runs at a time. Every command below talks to the running daemon over D-Bus and starts one when none is running. `kiekje help` prints the overview, `kiekje capture --help` the capture modes and flags.

## Capture

| Command | What it does |
| --- | --- |
| `kiekje capture` | Freeze the screen and select an area (the default mode) |
| `kiekje capture region` | Same as above |
| `kiekje capture fullscreen` | Freeze the screen and keep all of it, every monitor stitched into one image |
| `kiekje capture window` | Freeze the screen and pick a window. Plasma 6 and Hyprland; exits non-zero elsewhere |
| `kiekje capture repeat` | Capture the last selected region again, without the overlay |

### Flags

| Flag | Default | What it does |
| --- | --- | --- |
| `--backend <name>` | `auto` | Force `kwin`, `wlroots` or `portal` for this capture |
| `--delay <seconds>` | `0` | Wait before freezing the screen, up to 60 |
| `--edit` | | Annotate before keeping. A region on the overlay, other modes in the editor window |
| `--no-save` | | Copy to the clipboard without writing a file |

```bash
kiekje capture                                        # select a region, copy and save it
kiekje capture --delay 5                              # open a menu, then capture after 5 seconds
kiekje capture --edit                                 # select a region and annotate it
kiekje capture fullscreen --backend kwin --no-save    # every monitor through KWin, clipboard only
```

## Session

| Command | What it does |
| --- | --- |
| `kiekje daemon` | Start the background instance with the tray, the shortcuts and the clipboard |
| `kiekje quit` | Stop the running instance. Every thread, stream and file handle closes |
| `kiekje menu` | Open or close the capture menu |
| `kiekje settings` | Open the settings window |
| `kiekje gallery` | Open the gallery |
| `kiekje pin <path>` | Keep an image on screen above other windows |
| `kiekje record` | Start recording the screen, or stop and save the running recording |

## Setup

| Command | What it does |
| --- | --- |
| `kiekje doctor` | Report what this session supports and how to fix the rest |
| `kiekje install-desktop-file` | Register the launcher entry. Required on KDE for ScreenShot2 access |
| `kiekje help`, `-h`, `--help` | Show the overview |
| `kiekje version`, `-V`, `--version` | Print the version |

## Exit codes and output

A command exits non-zero when the daemon refuses it, such as window capture on a desktop without window geometry, or when the arguments are wrong. An unknown command suggests the nearest one: `unknown command 'doctr', did you mean 'doctor'?`. Help and errors print in colour on a terminal and plain when piped or when `NO_COLOR` is set.

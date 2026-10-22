---
title: Introduction
description: What Kiekje is, how a capture works and where it stands today.
---

# Introduction

**Kiekje** *(noun)*, /ˈkik.jə/, Dutch slang for a quick snapshot or photo.

Kiekje is a screenshot tray app for Linux. It grabs the screen the moment you trigger it, shows that frozen frame so you can pick the part you want, then copies it, saves it and keeps it in a gallery. It targets KDE Plasma and Hyprland (Omarchy) on Wayland. GNOME, Sway and X11 come after.

## How a capture works

1. **Trigger.** A shortcut, the tray, the CLI or a button sends one action to the running instance over D-Bus. A second trigger while one capture runs is dropped.
2. **Freeze.** Every monitor is grabbed in one backend call and kept as a raw buffer. Nothing is encoded yet.
3. **Select.** A layer-shell overlay per monitor shows its frozen frame. You drag a region, press a key to take a monitor or a window, or draw on it first.
4. **Keep.** The crop is set on the clipboard, encoded once, saved with an exclusive create so nothing is overwritten, and added to the gallery. A notification follows, with a sound if you want one.

Capture always happens before selection. What you select is what was on screen when you pressed the key, not what is there when you let go.

## What it does

| Area | What you get |
| --- | --- |
| Capture | Region, full screen, window (Plasma 6 and Hyprland), repeat last region, delay up to 60 seconds, multi-monitor with mixed scales, a pixel loupe and a colour picker |
| Annotate | An inline editor on the overlay and a separate editor window with box, ellipse, arrow, line, pen, marker, numbered steps, text, check, cross, emoji, pixelate, crop, undo and redo |
| Library | A gallery with grid and list views, search, filters, sorting, file actions, missing-file relinking, pinned captures and picked colours |
| Record | WebM screen recording through the ScreenCast portal |
| App | Tray with a styled popover, global shortcuts with a recorder, settings, a doctor that says what your session supports and how to fix the rest |

## Where it stands

Kiekje is pre-release. The current version ships as an alpha from [GitHub Releases](https://github.com/remcostoeten/kiekje/releases). Every feature has been exercised in virtual machines and headless sessions in the [lab](/project/lab-results), and the [support matrix](/reference/support) labels each one as lab tested, unit tested or unverified. Runs on real hardware are tracked in [issue #41](https://github.com/remcostoeten/kiekje/issues/41).

Not built, and not planned for V1: an X11 capture backend outside Plasma, text recognition, and anything on macOS or Windows.

## Next

- [Install](/guide/install) picks the package for your distribution.
- [Getting started](/guide/getting-started) takes the first capture and lists the one-time setup steps per desktop.
- [Desktops](/guide/desktops) explains what each compositor offers and where Kiekje falls back.

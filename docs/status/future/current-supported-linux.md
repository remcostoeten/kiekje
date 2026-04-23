# Current supported Linux

This page is the strict current Linux support matrix. It describes what the
codebase supports now, not what we intend to support later.

## Current support matrix

The app is Linux-only and Wayland-first. Support depends on compositor and
backend tooling, not on distro branding alone.

- Hyprland on Wayland: best supported.
  Region, fullscreen, and active-window capture exist. Launcher, tray,
  editor, doctor, tray autostart, and Hyprland shortcut generation all match
  this path.
- Other wlroots-style Wayland compositors: partial support.
  Fullscreen and region capture may work if `grim` works in that session.
  Active-window capture does not, because current code has only Hyprland
  window introspection.
- GNOME Wayland and KDE Wayland: not supported as first-class paths.
  Current backend does not use desktop portals for capture and assumes `grim`.
- X11 sessions: unsupported.

## What this means in practice

If someone asks, "Does Kiekje support Linux?", the precise answer is narrower.

- Supported now means Linux plus Wayland plus a compositor path that matches
  `grim`, with Hyprland as the strongest target.
- Distro packaging alone does not make the app supported.
- Display manager choice does not decide support.
- A broader Linux claim requires backend work first.

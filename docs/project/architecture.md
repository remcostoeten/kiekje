---
title: Architecture
description: The stack, the module layout, the capability contracts and the rules every change follows.
---

# Architecture

Kiekje is one Rust binary with a GTK 4 interface and direct protocol and D-Bus backends. The [rewrite proposal](/project/proposal) records why this stack was chosen over Qt, Tauri and Wails and how the two earlier versions informed it. This page is the current shape.

## Stack

| Concern | Choice |
| --- | --- |
| Core | Rust, edition 2024, one binary. `GtkApplication` for the single instance and D-Bus remote actions |
| UI | GTK 4 without libadwaita. One app stylesheet, `resources/style.css`, compiled in with GResource |
| Overlay and popover | `gtk4-layer-shell` on KWin, Hyprland and Sway. Fullscreen toplevels on GNOME and X11 |
| Capture | KWin `org.kde.KWin.ScreenShot2` over `zbus`; `ext-image-copy-capture` and `wlr-screencopy` through `smithay-client-toolkit`; `hyprland-toplevel-export` for windows; xdg-desktop-portal through `ashpd`; `x11rb` with MIT-SHM on hold |
| Shortcuts | GlobalShortcuts portal, kglobalaccel, a managed Hyprland include, CLI fallback everywhere |
| Tray | `ksni` on its own thread. It only sends commands to the GTK main loop and never creates widgets |
| Clipboard | GDK, owned by the long-running daemon. `ext_data_control_manager_v1` where the compositor offers it |
| Storage | `png` and JPEG encoders, exclusive-create writes, filename templates |
| Library | `rusqlite` (bundled), `fast_image_resize` thumbnails on a worker pool |
| Config | TOML through `serde` and `toml`, migrated on load, written atomically |
| Recording | GStreamer loaded at runtime with `dlopen`, `pipewiresrc` from the ScreenCast portal, VP8 in WebM |
| Async | GLib main loop for the UI, a worker runtime for D-Bus and IO, bridged with `async-channel` |

Not in the stack, by decision: a webview, Tauri, Wails, Qt, libadwaita, and subprocess calls to `grim`, `slurp`, `spectacle` or `wl-copy`.

## Layout

```text
src/
  main.rs
  app/          single instance, action routing, lifecycle
  capture/      request, frame, monitor layout, coordinate mapping
  backends/     kwin/, wlroots/, portal/, x11/, windows/ (window geometry)
  overlay/      layer-shell surfaces, selection, keyboard, magnifier, window snap
  editor/       annotation canvas and tools
  pin/          floating pinned captures
  record/       ScreenCast portal session and the GStreamer pipeline
  upload/       temporary upload
  shortcuts/    model, recorder, validation, registrars
  tray/         ksni host and the styled popover
  storage/      paths, filename templates, exclusive writes, formats
  library/      sqlite metadata, thumbnails, file watching
  ui/           settings/, gallery/, about/, widgets/
  doctor/       capability report and fix hints
  config/       load, migrate, atomic save
resources/      style.css, icons, fonts, gresource manifest
packaging/      aur/, deb/, rpm/, desktop files
lab/            the VM and headless test driver
docs/           this site
```

## Contracts

Product code depends on three capability contracts and never on a concrete backend:

- `CaptureBackend`: capability flags and `capture(outputs) -> Frames`.
- `ShortcutRegistrar`: capability flags, `register`, `effective_bindings`, `needs_system_dialog`.
- `TrayHost`: the tray thread's command channel.

The UI reads the flags, disables what a backend can't do and says why. Backends are chosen at runtime from the Wayland registry, D-Bus names, portal versions and session variables, never from which binaries are installed. An unimplemented capability reports as unavailable; nothing is ever presented as a placeholder that works.

## Capture flow

1. **Trigger.** A shortcut, tray item, CLI command or button sends one action to the single instance over D-Bus.
2. **Gate.** One capture is in flight at a time. Extra triggers are dropped and logged.
3. **Hide own UI.** Open Kiekje windows close. A tray-triggered capture waits for the menu to unmap.
4. **Acquire.** Every monitor is grabbed in one backend call. Frames stay raw buffers.
5. **Overlay.** One layer-shell surface per monitor shows its frame. Window mode opens the same overlay in a window-pick state.
6. **Select.** The rectangle is kept in global logical coordinates and mapped per monitor on confirm: `(logical - monitor_origin) * monitor_scale`, rounded outward.
7. **Post-capture** on a worker: crop, clipboard, encode once, save with exclusive create, insert the gallery record, notify.
8. **Cleanup.** Buffers are freed when the overlay closes. A monitor unplug during selection cancels everything with one notification.

## Rules

- The GTK main thread does no blocking work: no encoding, file IO, D-Bus round trips, thumbnailing or subprocess waits. Work goes to `gio::spawn_blocking` or the worker runtime and comes back over `async-channel`.
- Frames stay raw until the final crop. Encode once.
- One running instance. Closing a window never quits; Quit stops every thread, stream and file handle.
- File writes use exclusive create. Config and metadata writes go to a temp file and `rename`.
- Captures live in XDG Pictures and Videos, settings in `$XDG_CONFIG_HOME/kiekje`, the library in `$XDG_DATA_HOME/kiekje`, thumbnails and unsaved captures in `$XDG_CACHE_HOME/kiekje`.
- Errors are `thiserror` enums per module where callers branch on them, `anyhow` only at the app edge. An error is never detected by matching its message.
- No `unwrap` or `expect` outside tests and startup invariants. No empty error handling. Finite states are enums.

## Testing

Unit tests sit inline under `#[cfg(test)]`, named by behavior. Coordinate mapping, filename templates, shortcut validation and config migration have tests and keep them whenever they change. Backend and overlay behavior is exercised by `lab/kiekje-lab`, which drives VMs and headless sessions and writes each run to `lab-results/`. What was run on which desktop is recorded in the [lab results](/project/lab-results) and in each pull request, with anything untested labelled unverified.

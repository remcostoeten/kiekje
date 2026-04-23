# Runtime support and flags

This page defines the real runtime boundary of the program today. It focuses on
Linux only, because the current repository does not implement a non-Linux
backend.

## Operating system support

The codebase is Linux-only today.

- Linux on Wayland is target environment.
- Linux on X11 is not supported by the current capture path.
- macOS and Windows are not supported.

## Session and compositor support

The session matters more than the display manager. GDM, SDDM, and LightDM are
not the real compatibility gate. The real gate is whether the logged-in
Wayland session provides tooling and protocols that match the current backend.

- Hyprland is current best-supported compositor.
  Reason: active-window capture uses `hyprctl`, and the launcher can generate a
  Hyprland include file with shortcuts.
- Other wlroots-style compositors may support fullscreen and region capture if
  `grim` works there.
  This is inference from the backend choice, not an explicit support promise.
- Non-Hyprland compositors do not support active-window capture in current
  code, because there is no generic window-enumeration backend.
- GNOME Wayland and KDE Wayland are not first-class targets in current code.
  The app does not implement a portal-based capture backend, and the current
  path assumes `grim`.

## Desktop integration support

The product also depends on a few surrounding desktop capabilities.

- Tray support depends on a StatusNotifier-compatible host.
- Clipboard copy depends on `wl-copy`.
- Open-after-save depends on `xdg-open`.
- Startup portal warnings and repair helpers depend on
  `xdg-desktop-portal` and `gdbus` visibility.
- Hyprland shortcut setup is compositor-specific and writes
  `~/.config/hypr/kiekje-shortcuts.conf`.

## Runtime dependencies

These are the exact dependency classes the current code expects.

- `grim`: required for all capture modes.
- `wl-copy`: required only when clipboard copy stays enabled.
- `hyprctl`: required only for active-window capture on Hyprland.
- GTK4 and libadwaita runtime libraries: required for launcher, tray windows,
  editor, and region selector.
- `xdg-desktop-portal`: not used as capture backend, but checked to reduce GTK
  startup warnings and session issues.

## CLI flags and modes

This is the complete CLI surface from `src/main.rs`.

- `kiekje region`
  Run region capture.
- `kiekje fullscreen`
  Run fullscreen capture.
- `kiekje window`
  Run active-window capture through Hyprland.
- `kiekje --interactive`
  Open terminal menu with capture and settings actions.
- `kiekje --launcher`
  Open GTK launcher window.
- `kiekje --tray`
  Run tray icon service.
- `kiekje --doctor`
  Print dependency and portal readiness report.
- `kiekje --startup-delay-ms <ms>`
  Hidden internal flag used when launcher or tray spawns the main binary after
  a short delay.

## Support boundary

If you need exact present-tense positioning, use this summary.

- Supported now: Linux, Wayland, Hyprland-first workflow.
- Likely usable with caveats: Linux, Wayland, non-Hyprland wlroots compositor,
  region and fullscreen only.
- Not supported now: X11, GNOME-first path, KDE-first path, and non-Linux
  platforms.

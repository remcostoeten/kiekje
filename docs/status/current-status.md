# Current status

This page is the live work tracker for the current Linux-first product effort.
It is intentionally operational. Update the checkboxes as work lands.

## Current focus

The immediate goal is to stabilize the Linux core before any Tauri migration.

- [x] Create central status and planning docs under `docs/status/`.
- [ ] Decide whether `kiekje-tui` stays as maintainer tool or is deprecated.
- [ ] Finish tray and launcher parity for save location and secondary export
  toggles.
- [ ] Make non-editor save and dependency failures visibly actionable.
- [ ] Normalize cancellation behavior across CLI, launcher, tray, editor, and
  TUI.
- [ ] Clean stale `screeny` and `capture-app` references where migration
  compatibility is no longer needed.

## Pre-Tauri blockers

These are the blockers that matter before a new shell is introduced.

- [ ] Extract application services out of GTK-heavy workflow code.
- [ ] Define stable command and response shapes for capture, settings,
  diagnostics, and export.
- [ ] Add compositor abstraction so Linux support is not effectively
  `grim` plus Hyprland only.
- [ ] Add stronger automated coverage for core paths and high-risk parsing.
- [ ] Lock down support policy for Linux compositor and distro targets.

## Tauri preparation

These items shape the migration, but should not start before the blockers
above are under control.

- [ ] Decide frontend package location and naming so it does not clash with the
  current Rust `src/` tree.
- [ ] Define Rust command surface for a Tauri shell.
- [ ] Define React route and component inventory.
- [ ] Decide default styling stack and animation policy.
- [ ] Decide whether GTK editor remains fallback path during migration.

## Future Linux support

These items broaden the product after the backend is less coupled.

- [ ] Define Ubuntu support target and backend path.
- [ ] Define Debian support target and packaging constraints.
- [ ] Define Fedora support target and backend path.
- [ ] Add documented compositor matrix for Hyprland, Sway, GNOME, KDE, and
  others.

# Kiekje

A Linux-first screenshot tray app. It captures the screen at the moment of the trigger, shows that frozen frame in a selection overlay, then copies, saves and lists the result in a gallery. Primary targets are KDE Plasma on Wayland and Hyprland (Omarchy). GNOME, Sway and X11 come after.

The architecture, capability matrix, phases and acceptance tests live in [`docs/proposal.md`](./docs/proposal.md). Read it before changing capture, overlay, shortcuts or tray behavior. When a decision in this file and the proposal disagree, this file wins and the proposal gets updated.

## Status

Phase 0 is done, phase 1 code is in, phase 2 code is in: the capture library, its thumbnail cache, and a gallery with grid and list views, search, filters, sorting, file actions and missing-file handling. Phase 2b (the annotation editor) is next. Phase 1 has lab coverage on five desktops, overlay warm-up, portal global shortcuts, selection across monitors, the tray with its native menu, the TOML config with filename templates, capture delay, repeat last region, notifications with sound, the styled tray popover, the shortcut recorder and bundled fonts. Phase 1 still needs real-hardware runs on Omarchy, Plasma and GNOME. Nothing is released. Results live in [`docs/phase-0-results.md`](./docs/phase-0-results.md).

Decided:

- New repo; `~/dev/kiekje` (Rust/GTK) and `~/dev/cheese/cheese-wails` (Wails) are reference only. Port ideas, not files.
- The annotation editor is part of V1 (phase 2b).
- Dark-only UI following the design system in proposal section 10.

Open:

- Nothing open. Decided 2026-10-02: native packages first, Flatpak later as a portal-only build. Keep sandbox-incompatible backends behind capability checks so that build stays possible.

## Stack

- Rust, edition 2024, one binary.
- GTK4 without libadwaita. All styling comes from one app stylesheet (`resources/style.css`, compiled in with GResource).
- `gtk4-layer-shell` for the overlay and tray popover on KWin, Hyprland and Sway. Fullscreen toplevels on GNOME and X11.
- Capture backends: KWin `org.kde.KWin.ScreenShot2` over `zbus`; `ext-image-copy-capture` and `wlr-screencopy` through `smithay-client-toolkit`; `hyprland-toplevel-export` for windows; xdg-desktop-portal through `ashpd`; `x11rb` with MIT-SHM.
- `ksni` for the tray. The tray thread only sends commands to the GTK main loop. It never creates widgets.
- `rusqlite` (bundled) for the library, `toml` + `serde` for config, `fast_image_resize` for thumbnails, `png` for encoding.
- GStreamer for recording, Tesseract for OCR. Both are post-V1.

Do not add a webview, Tauri, Wails, Qt or libadwaita. Do not shell out to `grim`, `slurp`, `spectacle` or `wl-copy`; talk to the protocol or D-Bus interface directly.

## Layout

```
src/
  main.rs
  app/          single instance, action routing, lifecycle
  capture/      request, frame, monitor layout, coordinate mapping
  backends/     kwin/, wlroots/, portal/, x11/
  overlay/      layer-shell surfaces, selection, keyboard
  editor/       annotation canvas and tools
  shortcuts/    model, recorder, validation, registrars
  tray/         ksni host and the styled popover
  storage/      paths, filename templates, exclusive writes, formats
  library/      sqlite metadata, thumbnails, file watching
  ui/           settings/, gallery/, about/, widgets/
  doctor/       capability report and fix hints
  config/       load, migrate, atomic save
resources/      style.css, icons, fonts, gresource manifest
packaging/      aur/, deb/, rpm/, desktop files
docs/
```

File and module names are `snake_case`, as Rust requires. Keep modules small and domain-focused.

## Architecture rules

- Product code depends on capability contracts (`CaptureBackend`, `ShortcutRegistrar`, `TrayHost`), never on a concrete backend. The UI disables what a backend can't do and says why.
- Backends are chosen at runtime from the Wayland registry, D-Bus names, portal versions and session variables. Never from which binaries happen to be installed.
- Capture happens before selection. The overlay always shows an immutable frame grabbed at trigger time. Never select first and capture afterwards.
- The GTK main thread does no blocking work: no encoding, file IO, D-Bus round trips, thumbnailing or subprocess waits. Use `gio::spawn_blocking` or the worker runtime and return results over `async-channel`.
- Frames stay raw buffers until the final crop. Encode once.
- One running instance. CLI commands and later launches activate actions on it over D-Bus. Closing a window never quits; Quit stops every thread, stream and file handle.
- One capture in flight at a time. Extra triggers while one runs are dropped and logged.
- File writes use exclusive create; config and metadata writes go to a temp file and `rename`.
- User captures live in XDG Pictures/Videos; settings in `$XDG_CONFIG_HOME/kiekje`, library in `$XDG_DATA_HOME/kiekje`, thumbnails and unsaved captures in `$XDG_CACHE_HOME/kiekje`.
- Never present a placeholder or simulated backend as working support. Unimplemented capabilities report as unavailable.

## Code style

- No comments except a one-line workaround reason, a one-line explanation of an opaque expression or regex, or a `TODO`.
- Public functions in shared modules and business logic get a rustdoc block: a one-line summary, one sentence of detail when needed, and an `# Examples` section. Nothing else gets doc comments.
- Errors: `thiserror` enums per module for errors callers branch on (cancel, permission denied, backend unavailable), `anyhow` only at the app edge. Never detect an error by matching its message text.
- No `unwrap` or `expect` outside tests and startup invariants that are truly impossible to violate.
- No empty error handling. Swallowing an error needs a one-line reason.
- Finite states are enums, never strings.

## Design

Follow proposal section 10, sourced from `~/dev/marktplaats-tweakers-jev-scraper/DESIGN.md`:

- Tokens live once on `:root` in `style.css`. Write `--accent: #fe5101;` with a space after the colon; GTK's parser rejects it without.
- Derive tints with `color-mix()`. Don't add colours.
- Geist for content, uppercase Geist Mono for labels and buttons. Both fonts are bundled.
- One primary button per view. Selection is a tint, never an inverted fill.
- GTK has no `::before`/`::after`. Corner marks are drawn by a widget.
- Monochrome symbolic icons only. No emoji anywhere in the UI.
- Every interactive element is keyboard reachable with a visible focus ring.

## Commands

```
cargo build
cargo run -- doctor
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

CI runs fmt, clippy and test. All three must pass before a change is done.

## Testing

- Unit tests sit inline under `#[cfg(test)]`, named by behavior (`maps_selection_across_fractional_scale`).
- Coordinate mapping, filename templates, shortcut validation and config migration need tests whenever they change.
- Backend and overlay behavior can't be proven by unit tests. Record what was tested on which desktop (KDE, Hyprland, GNOME) in the PR, and label anything untested as unverified.
- Benchmarks from proposal section 8 report p50 and p95 with the machine and desktop version. Never write an estimate as a measurement.

## Commits and PRs

- Conventional Commit prefixes, short and imperative: `feat: add kwin screenshot2 backend`.
- PRs state what changed, why, the checks run, and which desktops were tested. Attach screenshots for UI changes.

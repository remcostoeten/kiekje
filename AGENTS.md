# Kiekje

A Linux-first screenshot tray app. It captures the screen at the moment of the trigger, shows that frozen frame in a selection overlay, then copies, saves and lists the result in a gallery. Primary targets are KDE Plasma on Wayland and Hyprland (Omarchy). GNOME, Sway and X11 come after.

The architecture, capability matrix, phases and acceptance tests live in [`docs/project/proposal.md`](./docs/project/proposal.md). Read it before changing capture, overlay, shortcuts or tray behavior. When a decision in this file and the proposal disagree, this file wins and the proposal gets updated.

## Status

Phase 0 is done, phase 1 code is in, phase 2 code is in: the capture library, its thumbnail cache, and a gallery with grid and list views, search, filters (kind, period, dimensions, file size, annotated, unsaved), sorting, file actions and missing-file handling. With `[saving] enabled = false` captures are copied and kept in `$XDG_CACHE_HOME/kiekje/unsaved` with an Unsaved badge and a Keep action, and pruned at startup after `keep_unsaved_days` (default 7); this needs real-desktop runs. Phase 2b, the annotation editor, has started: it opens from the gallery with shape, pen, marker, step and pixelate tools, text, crop, a select tool that moves, resizes and deletes shapes, undo and redo, copy and save, and it opens straight after a capture with `capture --edit` or the tray's Annotate region item. A region can also be annotated on the overlay itself, Spectacle style, with the tools above the selection and Accept, Cancel, Save, Save as and Copy below it; `capture --edit`, Annotate region and the `[capture] after_selection = "edit"` setting use it. The inline editor works on the whole frozen monitor frame with the region as its crop, so the region keeps handles to move and resize it (arrows move it, Ctrl+arrows resize it), and D or a tool letter before selecting starts drawing first, with S picking the region afterwards. Before a selection F takes the monitor under the pointer (with several monitors F highlights one, F again or Shift+F cycles, Enter captures it), W or Tab cycles windows and Enter captures the highlighted one, Shift+R takes the last region, and the keyboard alone can draw a region: arrows move a selection cursor (Shift for 10 pixels), Space sets the first corner, arrows move the second, Space or Enter finishes; a hint bar lists the keys (`[overlay] hints`, ? hides it). In the editor the arrows move a keyboard cursor (Shift for 10 pixels) and Space places text, stamps and step markers or starts and finishes a rectangle, ellipse, arrow, line, pen stroke, marker, pixelate box or crop (Enter also finishes); Page Up and Page Down select the previous or next shape and describe it, and arrows and Delete then move or remove it. Save as keeps the editor open when the file chooser is cancelled or fails. A capture delay shows a countdown window with a Cancel button, and Escape cancels it. Pins move with the arrows (1 pixel, Shift 10) where a layer-shell pin exists, Page Up and Page Down change opacity, and a pin on an ordinary toplevel says in its description that moving needs the window manager. These need real-desktop runs. Phase 2b is code complete; it still needs real-desktop runs. Settings is a sidebar window with Capture, Shortcuts, Saving, General, Doctor and About pages built from shared preference rows (`ui/widgets`); About holds the version, data paths, the doctor report and the danger zone (reset settings, clear gallery, clear colours, clear thumbnails). The overlay has a pixel loupe with the colour under the pointer and snaps to the window under the pointer on Plasma 6 and Hyprland, and a capture can be pinned on screen as a floating window from the overlay, the gallery or `kiekje pin`; all three need real-desktop runs. Colours copied from the loupe with C are listed under Colours in the gallery. Window capture is its own mode: `kiekje capture window`, a tray item and a shortcut without a default key open the overlay in a window-pick state where a click or Enter takes the hovered window. It works where a window geometry source exists (Plasma 6, Hyprland); elsewhere the tray item and shortcut are disabled with the reason, the CLI exits non-zero and `doctor` lists it as unavailable. Unplugging a monitor during a selection closes the overlay and cancels the capture with one notification and no file. Both need real-desktop runs. An opt-in update check (`[general] check_for_updates`, off by default) asks the GitHub releases API at startup and at most once a day, and About has a Check now button; Settings and `kiekje doctor` say whether a clipboard manager keeps copies after Quit. Both need real-desktop runs. An opt-in temporary upload (`[upload] enabled`, off by default) sends a capture from the gallery to Litterbox with a 1 to 72 hour expiry and copies the link; it is unverified against the live host. Screen recording (`kiekje record`, the tray and the capture menu) goes through the ScreenCast portal and a GStreamer pipeline loaded at runtime with `dlopen`, like the capture sound, and saves WebM to `~/Videos/Kiekje`; the pipeline is lab tested against a PipeWire source, the portal handshake needs real-desktop runs. GIF export and upload after a recording come next. Phase 1 has lab coverage on five desktops, overlay warm-up, portal global shortcuts, selection across monitors, the tray with its native menu, the TOML config with filename templates, capture delay, repeat last region, notifications with sound, the styled tray popover, the shortcut recorder and bundled fonts. Phase 1 still needs real-hardware runs on Omarchy, Plasma and GNOME. Pre-release alphas ship from GitHub Releases; nothing is stable yet. Results live in [`docs/project/lab-results.md`](./docs/project/lab-results.md).

Decided:

- New repo; `~/dev/kiekje` (Rust/GTK) and `~/dev/cheese/cheese-wails` (Wails) are reference only. Port ideas, not files.
- The annotation editor is part of V1 (phase 2b).
- Dark and light UI following the design page, light switched with `[general] theme` (2026-10-23).
- The binary, packages and app id are `kiekje` and `dev.remcostoeten.Kiekje` (2026-10-04).
- MIT license (2026-10-04). The bundled Geist fonts stay under the OFL.

Open:

- Nothing open. Decided 2026-10-02: native packages first, Flatpak later as a portal-only build. Keep sandbox-incompatible backends behind capability checks so that build stays possible.

## Stack

- Rust, edition 2024, one binary.
- GTK4 without libadwaita. All styling comes from one app stylesheet (`resources/style.css`, compiled in with GResource).
- `gtk4-layer-shell` for the overlay and tray popover on KWin, Hyprland and Sway. Fullscreen toplevels on GNOME and X11.
- Capture backends: KWin `org.kde.KWin.ScreenShot2` over `zbus`; `ext-image-copy-capture` and `wlr-screencopy` through `smithay-client-toolkit`; `hyprland-toplevel-export` for windows; xdg-desktop-portal over `zbus`. An `x11rb` backend with MIT-SHM is on hold.
- `ksni` for the tray. The tray thread only sends commands to the GTK main loop. It never creates widgets.
- `rusqlite` (bundled) for the library, `toml` + `serde` for config, `fast_image_resize` for thumbnails, `png` for encoding.
- GStreamer for recording, Tesseract for OCR. Both are post-V1.

Do not add a webview, Tauri, Wails, Qt or libadwaita. Do not shell out to `grim`, `slurp`, `spectacle` or `wl-copy`; talk to the protocol or D-Bus interface directly.

## Layout

```
src/
  main.rs
  cli.rs        argument parsing for the CLI commands
  ids.rs        the application id
  log.rs        error, warn, info and debug macros, filtered by KIEKJE_LOG
  app/          session state and action routing; startup, shortcuts, tray, panels, capture flow, delivery, saving, recording
  capture/      request, frame, monitor layout, coordinate mapping
  backends/     kwin/, wlroots/, portal/, windows/ (window geometry)
  overlay/      layer-shell surfaces, selection, keyboard, magnifier, window snap
  editor/       annotation canvas and tools
  pin/          floating pinned captures
  shortcuts/    model, recorder, validation, registrars
  tray/         ksni host and the styled popover
  storage/      paths, filename templates, exclusive writes, formats
  library/      sqlite metadata, thumbnails, file watching
  ui/           settings/, gallery/, about/, widgets/
  doctor/       capability report and fix hints
  config/       load, migrate, atomic save
resources/      style.css, icons, fonts, gresource manifest
packaging/      aur/, deb/, rpm/, desktop files
docs/          the VitePress documentation site: guide, reference, project pages, release notes
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
- Errors: `thiserror` enums per module for errors callers branch on (cancel, permission denied, backend unavailable), no `anyhow`: the app edge prints the typed enums (`CliError` for the CLI) and `Display` carries the message. Never detect an error by matching its message text.
- No `unwrap` or `expect` outside tests and startup invariants that are truly impossible to violate.
- No empty error handling. Swallowing an error needs a one-line reason.
- Diagnostics go through `error!`, `warn!`, `info!` and `debug!` from `src/log.rs`, which write `kiekje: [module] message` to stderr. `KIEKJE_LOG=error|warn|info|debug` filters them (default `info`). Plain `println!` is only for CLI results such as `doctor`, `help` and `--version`. The lab script reads the daemon log, so change a logged message together with `lab/kiekje-lab`.
- Finite states are enums, never strings.

## Design

The "Crisp tool" direction (2026-10-23), a warm graphite dark UI in the spirit of Linear and Raycast. [`docs/project/design.md`](./docs/project/design.md) is the working reference; where it and proposal section 10 disagree, the design page wins.

- Tokens live once on `:root` in `style.css`, with the light values in `:root.light`. Write `--accent: #fe5101;` with a space after the colon; GTK's parser rejects it without.
- Derive tints with `color-mix(in srgb, <colour> <n>%, <colour>)`. Don't add colours outside the token list. `build.rs` flattens tokens and `color-mix()` into plain values for GTK 4.14 and fails the build on any other form.
- One sans family, Geist, in sentence case at 12 to 13px. Geist Mono only for values that are code: file name templates and paths. No uppercase, no letter spacing, no corner marks.
- One orange accent, the app icon's `#fe5101`, used for selection, switches, the primary button and focus. Red is for danger only.
- One primary button per view. Selection is a background tint, never an inverted fill.
- Monochrome symbolic icons only, drawn as filled paths so GTK can recolour them. No emoji anywhere in the UI.
- State feedback (hover, focus, focus-visible, focus-within, selected, checked, active) is a background colour, an icon fill or a text colour change, never an outline, border, border-colour change or box-shadow ring. Borders at rest stay; a state never adds or recolours one.
- Every interactive element is keyboard reachable with a visible focus state. Focus-visible and selected use a stronger tint and brighter text or icon than hover.

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

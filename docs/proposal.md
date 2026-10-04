# Kiekje rewrite proposal

Status: proposal, nothing implemented. Written 2026-10-01.

Labels used throughout:

- **[R]** your requirement
- **[S]** my suggestion
- **Measured**: run on this machine (Arch, Plasma 6.7.5, KWin 6.7.5 Wayland)
- **Code**: verified by reading the existing repos
- **Docs**: from upstream protocol or portal documentation, still needs a real desktop test
- **Unverified**: belief only, must be tested before it is relied on

## 1. The two existing versions

| Area | Rust/GTK `kiekje` | Wails `cheese/cheese-wails` |
|---|---|---|
| Capture backends | grim (`src/platform/linux/grim.rs`), portal (`portal.rs`), Spectacle (`spectacle.rs`), backend picker in `platform/linux/mod.rs` | `slurp` + `grim` and Spectacle on Plasma (`app.go:1032-1065`) |
| Frozen frame | Yes: captures first, then selects on the frame in `app/overlay_editor.rs` | Yes on Hyprland (overlay in `frontend/src/capture/overlay.js`). Plasma grabs fullscreen through Spectacle and crops |
| Editor | Basic tools; broken undo; no blur (`editor/canvas.rs`) | Undo/redo, blur, stroke width, resize handles, ellipse, steps, crop (`frontend/src/editor/*`) |
| Shortcuts | Writes Hyprland and KDE config files (`platform/linux/integration.rs`) | Hyprland managed include (`internal/hyprland/config.go`) |
| Tray | ksni in-process, freezes when it opens GTK windows (`app/tray.rs:771`) | Separate sidecar binary (`cmd/kiekje-tray/main.go`) |
| Install / deps | Doctor screen (`diagnostics/mod.rs`) | pkexec dependency install (`internal/deps/install.go`) |
| Health | Tests pass, clippy fails with about 70 errors, 5 critical bugs | `go vet` and tests pass; `app.go` is a 1627-line monolith; frontend is untyped JS |

Wrong assumptions to drop:

- The Wails comment at `app.go:1033` says KWin's ScreenShot2 is whitelisted to Spectacle and the KDE portal only. KWin actually grants access to any app whose installed `.desktop` file declares `X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2`. **Measured**: Spectacle, plasmashell and the KDE portal all use exactly that key on this machine.
- Both apps shell out to `grim`, `slurp`, `spectacle` and `wl-copy`. Each spawn adds latency and PNG encode/decode round trips. The rewrite should talk to the protocols directly.

What to keep or adapt:

- **Keep**:
  - The Wails editor tool set, as the annotation spec.
  - The pkexec dependency install flow (`internal/deps`).
  - The Hyprland managed include file pattern.
  - The filename token tests (`kiekje/src/storage/save.rs`).
  - The doctor concept.
- **Adapt**:
  - KDE shortcut writing (`integration.rs`): fix the quoting and the data wipe on read errors.
  - The portal code (`portal.rs`): fix the subscribe-before-call race and percent decoding.
- **Replace**:
  - The webview overlay.
  - The tray sidecar and the in-thread GTK tray windows.
  - The Go TUI: the CLI covers it.
  - All `wl-copy` and `grim` subprocesses.

## 2. Recommended stack

**Rust core, GTK4 UI with a custom stylesheet (no libadwaita, see section 10), gtk4-layer-shell overlay, direct protocol and D-Bus backends.**

| Concern | Choice |
|---|---|
| Core | Rust 2024, one binary, `GtkApplication` for single instance and D-Bus remote actions |
| Overlay | GTK4 + `gtk4-layer-shell` (overlay layer, exclusive keyboard) on KWin, Hyprland and Sway; fullscreen toplevel per monitor on GNOME and X11 |
| Settings, gallery, About | Plain GTK4 widgets with one app stylesheet built from the design tokens in section 10, symbolic (monochrome) icons, built lazily |
| Tray popup | Our own layer-shell popover on left click, styled like the app; native SNI menu on right click as a fallback (section 10) |
| Capture on KDE | `zbus` to `org.kde.KWin.ScreenShot2` (raw pixels over a pipe fd); portal as fallback |
| Capture on wlroots / Hyprland | `wayland-client` + `smithay-client-toolkit`: `ext-image-copy-capture`, then `wlr-screencopy`; window capture through `hyprland-toplevel-export` |
| Capture on GNOME | Screenshot portal through `ashpd` |
| Capture on X11 | `x11rb` with the MIT-SHM extension |
| Global shortcuts | Per platform: GlobalShortcuts portal (`ashpd`), Hyprland managed `bind` include, KDE portal, `XGrabKey` on X11, CLI fallback everywhere |
| Tray | `ksni` (StatusNotifierItem) on its own thread, sends commands only; never builds windows |
| Clipboard | GDK clipboard owned by the long-running tray process (no `wl-copy`) |
| Encoding | `png` with a fast filter as the default; lossless WebP and JPEG as options |
| Thumbnails | `fast_image_resize` on a worker pool, cached in `$XDG_CACHE_HOME/kiekje/thumbs` |
| Metadata | SQLite through `rusqlite` (bundled) in `$XDG_DATA_HOME/kiekje/library.db` |
| Config | TOML through `serde` + `toml` in `$XDG_CONFIG_HOME/kiekje/config.toml`, with atomic writes |
| Async | GLib main loop for the UI; one worker runtime for D-Bus and IO, bridged with `async-channel` |
| Recording (later) | GStreamer: `pipewiresrc` from the ScreenCast portal, VA-API or x264 encode |
| OCR (later) | Tesseract |
| Lint and format | clippy `-D warnings`, rustfmt, enforced in CI along with `cargo test` |
| Packaging | AUR (`kiekje`, `kiekje-bin`), `.deb` and `.rpm` through `cargo-deb` and `cargo-generate-rpm`, portable tarball; Flatpak later with reduced features (see section 9) |

Why this stack beats the alternatives:

- **Qt 6 / QML** is the most native fit for KDE and has `LayerShellQt`. The catch is the language:
  - With C++, you lose the Rust crates that make the backends small: `ashpd`, `zbus`, `smithay-client-toolkit` and `x11rb`.
  - With Rust through `cxx-qt`, you add a second build system and a much thinner ecosystem.
  - I would only pick Qt if a Breeze-native look on KDE is a hard requirement. The cost is that GTK4 + libadwaita looks like a GNOME app on Plasma, not like Breeze.
- **Tauri / Wails** have three problems for this app:
  - A webview overlay needs every full-resolution frame copied or encoded across the IPC boundary on the hottest path.
  - It can't use layer-shell.
  - WebKitGTK has a higher idle memory footprint than a native toolkit. That's a general property; I did not measure it.

  The Wails editor ideas port fine to a cairo or GSK canvas.
- **Rust + GTK4** gets the overlay latency right and keeps one language from capture to UI. You already know gtk-rs. The kiekje failures were architectural (the tray thread running GTK, blocking the main thread), not toolkit limits. Plain GTK4 with a custom stylesheet can render the design system in section 10, so the GNOME look of libadwaita on Plasma no longer applies.

## 3. Capability matrix

S = supported, F = fallback, U = unavailable, ? = unverified

| Environment | Fullscreen | Region (frozen) | Window | Overlay placement | Global shortcuts | Tray | Clipboard | Recording |
|---|---|---|---|---|---|---|---|---|
| **Plasma 6 Wayland (Arch, primary)** | S: KWin ScreenShot2 (Docs, ?), F: portal (**Measured**: 266-473 ms) | S: frozen frame from ScreenShot2 or portal | S: ScreenShot2 `CaptureActiveWindow` / `CaptureInteractive` (?) | S: layer-shell (**Measured**: `zwlr_layer_shell_v1` advertised) | S: GlobalShortcuts portal v2 (**Measured**: version 2 present); KDE shows its own confirm dialog | S: native SNI | S | S: ScreenCast portal v5 (**Measured**: present) |
| **Hyprland / Omarchy** | S: screencopy (Code: grim works) | S | S: `hyprland-toplevel-export` (?) | S: layer-shell | S: managed `bind = …, global, kiekje:<id>` include + portal; preferred triggers are ignored by the Hyprland portal (Docs) | F: needs a Waybar tray module; Omarchy's Waybar config (?) | S | S: portal / PipeWire |
| **Sway** | S: screencopy | S | F: no standard window capture; select a window's rectangle from the frozen frame through `ext-foreign-toplevel-list` / IPC geometry (?) | S: layer-shell | F: CLI commands bound in the Sway config | F: Waybar or swaybar tray | S | S: portal (xdg-desktop-portal-wlr) |
| **GNOME Wayland (Ubuntu, Fedora, Omakub)** | S: portal; first use asks for permission, then it persists (Docs, ?) | S: frozen frame from the portal; overlay is a fullscreen toplevel per monitor, not layer-shell (U) | F: portal interactive mode only, which is GNOME's own UI, not our overlay | F: fullscreen window; no stacking guarantees above other fullscreen apps (?) | S: GlobalShortcuts portal on GNOME 48+ (Docs, ?); F: CLI + GNOME custom shortcut | F: needs the AppIndicator extension (Ubuntu ships it, Fedora does not) | S | S: portal |
| **Plasma on Fedora, Kubuntu, Debian, openSUSE** | Same as primary, but depends on the Plasma version. Debian stable ships an older Plasma (?) | same | same | same | same | S | S | S |
| **X11 (Xfce, Cinnamon, MATE, Plasma X11, GNOME Xorg)** | S: XShm | S: fullscreen override-redirect windows | S: `_NET_ACTIVE_WINDOW` + XGetImage | S | S: XGrabKey | S: SNI or XEmbed (?) | S | F: ffmpeg x11grab or GStreamer `ximagesrc` |

Runtime detection reads `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, the Wayland registry globals, D-Bus names (`org.kde.KWin`, portal interface versions) and the `HYPRLAND_INSTANCE_SIGNATURE` variable. It never guesses from which binaries are installed. Each missing capability maps to a doctor entry that names the exact package or setting to fix.

## 4. Capture flow

1. **Trigger**: shortcut, tray item, CLI (`kiekje capture region`) or a button sends an action to the single instance over D-Bus.
2. **Gate**: if a capture is already in flight, the request is dropped and that is logged; repeated shortcut presses never stack.
3. **Hide own UI**: close any open Kiekje windows. If the trigger came from the tray menu, wait for the menu to unmap, using a fixed short delay (?). That delay is measured in the prototype.
4. **Acquire**: grab every monitor (or the active one) in a single backend call, record the trigger and frame timestamps, and keep frames as raw buffers. Nothing is encoded yet.
5. **Fullscreen mode**: skip to step 8.
6. **Overlay**: one layer-shell surface per monitor, each showing its own frozen frame scaled for that monitor's logical size. Keyboard focus goes to the overlay. Esc cancels and drops all buffers.
7. **Select**: the selection rect is kept in global logical coordinates. On confirm it is converted per monitor: `(logical - monitor_origin) * monitor_scale`, rounded outward to whole pixels. This handles negative origins, rotation (via the output transform) and fractional scale (via `wp_fractional_scale_v1` or the output size versus the buffer size).
8. **Post-capture** on a worker thread: crop, set the clipboard (GDK, on the main thread with a ready texture), encode, save with exclusive create, insert the gallery record, then notification and sound.
9. **Cleanup**: frame buffers are freed when the overlay closes. A monitor disconnect during selection cancels that monitor's surface; if the selection was on it, the capture is canceled with a message.

Timing per backend:

- **KWin ScreenShot2**: synchronous and non-interactive. Expected to be the fastest KDE path, but I haven't measured it yet.
- **KDE portal**: **Measured** at 473 ms cold and 266-293 ms warm. It writes a PNG into `~/Pictures` and overwrote its own file when two captures landed in the same second. **Measured**: 3 captures produced only 2 files. So the portal is a fallback only, and Kiekje must move or delete its file.
- **Persistent ScreenCast stream** (a pre-authorized PipeWire stream kept open for an instant frame): not the default. The prototype measures its idle CPU and memory against the per-capture latency, and it is only offered as an opt-in if the saving is material.

## 5. Module structure

```
src/
  main.rs
  app/            single-instance, action routing, lifecycle
  capture/        capture-request, frame, monitor-layout, coordinate-mapping
  backends/
    kwin/         screenshot2 over D-Bus
    wlroots/      ext-image-copy-capture, wlr-screencopy, hyprland-toplevel-export
    portal/       screenshot, screencast, global-shortcuts (ashpd)
    x11/          xshm, xgrabkey
  overlay/        layer-shell surfaces, selection, keyboard
  shortcuts/      model, recorder, validation, per-platform registrars
  tray/           ksni, sends commands only
  storage/        paths, filename templates, exclusive writes, formats
  library/        sqlite metadata, thumbnails, file watcher
  ui/
    settings/     categories
    gallery/      grid, list, filters, search
    about/
  doctor/         capability report, fix hints
  config/         toml load, migrate, atomic save
```

Contracts: `CaptureBackend` (capabilities, `capture(outputs) -> Frames`), `ShortcutRegistrar` (capabilities, `register`, `effective_bindings`, `needs_system_dialog`) and `TrayHost`. The product logic only reads capability flags; the UI greys out what a backend can't do and explains why.

## 6. Consolidated requirements

Your requirements **[R]** from the brief are all kept: sections 3 to 11 of the brief, including the tray menu, storage, filename presets, general settings, gallery filters and search, the shortcut recorder rules, About/updates/danger zone, and the optional extensions as roadmap items.

My additions and decisions **[S]**:

1. **Default folders** are `$XDG_PICTURES_DIR/Kiekje` and `$XDG_VIDEOS_DIR/Kiekje`, instead of `~/kiekje/...`. They respect localized and relocated user dirs and file managers already index them. `~/kiekje` stays available as a preset.
2. **When saving is disabled**, captures go to `$XDG_CACHE_HOME/kiekje/unsaved`, are shown in the gallery with an "unsaved" badge and a "Keep" action, and are pruned after 7 days (configurable).
3. **`$MOD`** is Super (Meta) on Linux. In a future port it would map to Cmd on macOS and Ctrl+Alt on Windows (Windows reserves most Super combos). PrintScreen is allowed with no modifier.
4. **Default bindings**:

   | Action | Binding |
   |---|---|
   | Capture (default mode) | `Print` |
   | Region | `$MOD+Shift+S` |
   | Fullscreen | `Shift+Print` |
   | Settings | `$MOD+Shift+K` |
   | Quit | none (only available on request) |
5. **Shortcut activation** is press-based, on the main key with exactly the bound modifier set. Holding a letter before the modifiers does nothing because the compositor matches whole combos.
6. **Deleting from the gallery** offers "Remove from gallery" (record only) and "Move to trash" (file plus record, through `gio::File::trash`). Permanent deletion is never the default.
7. **Missing files**: on startup and on gallery open, entries are checked against disk and marked as missing. Moved files are re-linked by size and hash when they appear inside the capture folders.
8. **Settings** apply immediately, except the capture backend override and the tray toggle, which take effect on next launch and say so.
9. **Extra filters**: date range, dimensions, file size, "has annotations", "unsaved".
10. **The doctor** is reachable from Settings and from `kiekje doctor`.
11. **Clipboard persistence**: Kiekje owns the clipboard while it runs. After Quit, the image survives only if a clipboard manager (Klipper, cliphist) copied it. The UI tells you this.

## 7. Phases

**Phase 0: feasibility prototype (first, before anything else)**

A throwaway Rust binary that does trigger → capture → layer-shell overlay with the frozen frame → select → clipboard. Backends: KWin ScreenShot2, portal, wlroots screencopy. It is tested on:

- Plasma here.
- A Hyprland / Omarchy session: the Hyprland portal is already installed on this machine.
- GNOME 48+ in a VM.

Pass criteria are in section 8. If ScreenShot2 refuses access, KDE falls back to the portal and we decide whether its measured latency is acceptable.

**Phase 1: V1 core**

- Single instance and CLI.
- Tray.
- Config.
- Storage and filenames.
- Clipboard.
- Notifications and sound.
- Shortcuts with the recorder.
- Doctor.
- Multi-monitor and scaling.
- Repeat last region.
- Capture delay: it's cheap, so it's included.

**Phase 2: V1 gallery**

- SQLite library.
- Thumbnails.
- Grid and list views.
- Sort, filter and search.
- File actions.
- Missing-file handling.

**Phase 2b: V1 annotation editor** (decided 2026-10-02: in V1)

Port the Wails tool set (`cheese-wails/frontend/src/editor/*`) to a GTK snapshot canvas: rectangle, ellipse, arrow, line, pen, highlight, text, numbered steps, blur/pixelate, crop, resize handles, undo/redo as edit commands. Opens from the overlay after selection and from the gallery.

**Phase 3: V1 release**

- About and update check. About is done in settings; the update check waits for a release feed.
- Danger zone. Done: reset settings, clear gallery, clear thumbnails.
- AUR, deb, rpm and tarball packages.
- CI with clippy, tests and fmt.

**Optional, roadmap**

| Feature | Cost | Notes |
|---|---|---|
| Recording | Large | GStreamer + ScreenCast portal; MP4 (H.264) and WebM; pause/resume; mic and desktop audio through PipeWire |
| Color picker | Small | |
| Ruler | Small | |
| OCR | Medium | |
| Flatpak | Medium | |

## 8. Acceptance tests and benchmarks

- **Trigger-to-frame latency**: from shortcut press (or D-Bus call) to frame in memory, p50 and p95 over 50 runs, per backend. The baseline is the KDE portal measured above at 266-473 ms.
- **Trigger-to-overlay latency**: until the first frame is presented, using the GTK frame clock timestamp.
- **Frozen frame correctness**: play a video, trigger, and wait 3 s. The selected crop must match the frame at trigger time, not 3 s later.
- **Idle cost**: tray-only RSS and CPU over 10 minutes; also with the opt-in persistent stream, if it exists.
- **Multi-monitor**:
  - Scales 1.0 + 1.5, 1.0 + 2.0 and rotated layouts with a negative origin.
  - A selection across a monitor edge must crop pixel-exactly. Verify with a test pattern.
- **Monitor hot-unplug during selection**: no crash, a clear message.
- **Shortcut lifecycle**:
  - Register, change, disable one, disable all, restore defaults, and restart persistence.
  - Replacement failure must keep the old binding.
  - Run per platform registrar.
- **Cancellation**: Esc at overlay, tray-triggered capture and double-press, all leave no file and no gallery record.
- **Clipboard**: paste into a browser, Discord and GIMP while the app runs; behavior after Quit with and without a clipboard manager.
- **Storage**:
  - A read-only folder, a full disk (loop mount) and an unmounted USB target each produce an error notice with no data loss.
  - 20 captures in the same second produce 20 files.
- **Gallery at scale**: 10,000 items; scrolling stays smooth and only visible thumbnails decode.
- **Unit tests**: coordinate mapping, filename templates, shortcut validation and config migration.

## 9. Open decisions

1. **Flatpak**: decided 2026-10-02, native packages first, Flatpak later. Original question: Flatpak as a first-class target? Inside Flatpak, KWin ScreenShot2 is unavailable (the restricted-interface check depends on the host `.desktop` file), and direct Hyprland protocols depend on sandbox permissions (?). Making Flatpak first-class means portal-only capture, with the portal's latency and permission dialogs. My suggestion is native packages first and Flatpak later.
2. **Annotation editor**: decided, part of V1 (Phase 2b).
3. **Breeze-native look on KDE**: no longer relevant now that the app has its own design system (section 10).

## 10. Design and look and feel

Source: the design system in `marktplaats-tweakers-jev-scraper/DESIGN.md`, which matches the facehash.dev look. Its accent `#fe5101` is facehash's dark-mode orange.

The look:
- Near-black ground (`#0a0a0a`), monochrome, one orange accent.
- Geist for content, uppercase Geist Mono for labels and buttons.
- Structure drawn with 1px dashed lines and small square corner marks.
- A dot grid behind headers, short ease-out motion.

Kiekje is dark only, which follows your "dark, neutral UI" requirement. It uses the dark token column only.

### What GTK can render

**Measured**: parsed with `Gtk.CssProvider` on GTK 4.22.5 here. Parsing was tested, not on-screen rendering.

| Design feature | GTK 4.22 |
|---|---|
| CSS variables (`--accent`, `var()`) | OK. GTK's parser needs a space after the colon: `--accent:#fe5101` fails, `--accent: #fe5101` works |
| `color-mix()` for tints, hover washes and the focus ring | OK, also with `var()` inside |
| Dashed borders, radius, `box-shadow` | OK |
| Dot grid (`radial-gradient` + `background-size`) | OK |
| Uppercase mono labels (`text-transform`, `letter-spacing`) | OK |
| Focus ring (`outline` + `outline-offset`) | OK |
| Press scale, entry animations (`transition: transform`, `@keyframes`) | OK |
| `prefers-reduced-motion` and `prefers-color-scheme` media | OK |
| Corner marks via `::before` / `::after` | Not supported. Draw them in a small `FramedSection` widget with `snapshot`, about 30 lines |
| Geist and Geist Mono | Bundle both (OFL licence) in `/usr/share/kiekje/fonts` and register them at startup. Don't rely on them being installed: only the Nerd Font patched Geist Mono is installed on this machine |

The stylesheet is one `style.css` compiled into the binary with GResource. Tokens are written once on `:root`; components only use tokens and `color-mix()`, following the "don't add colours" rule from `DESIGN.md`.

### Where the look applies

| Surface | Styled by us? | Notes |
|---|---|---|
| Settings, gallery, About | Yes | Framed dashed column, `caps` section labels, chips for gallery filters, cards for thumbnails with the hover lift and accent wash |
| Selection overlay | Yes | Dimmed frozen frame, 1px accent selection border, corner handles as the 7px squares, mono size readout (`1280 × 720`) |
| Toasts in our windows | Yes | `DESIGN.md` toast recipe |
| Tray popup (left click) | Yes, our own window | See below |
| Native tray menu (right click) | **No** | The StatusNotifierItem menu is drawn by Plasma, Waybar or the GNOME extension in the system theme. No app can style it |
| Desktop notifications | **No** | Drawn by the notification daemon (Plasma, mako, GNOME) |
| Portal and KDE permission dialogs | **No** | System UI |

### Tray dropdown

Because the native tray menu can't be themed, the styled dropdown is a separate window that Kiekje draws itself:

- **Left click** on the tray icon opens a Kiekje popover:
  - Capture actions with their shortcuts shown in mono.
  - Recent captures as a row of 3 to 4 thumbnails.
  - Folder, Gallery, Settings, Quit.
- **Placement**: on KWin, Hyprland and Sway it is a transparent layer-shell surface over the monitor with the panel in the corner nearest the click, closed by a click outside the panel or Esc. The `Activate(x, y)` coordinates the tray host sends decide which corner. **Unverified**: whether Waybar on Omarchy sends usable coordinates. If it doesn't, the popover uses the first monitor's top right.
- **GNOME and X11**: there is no layer-shell on GNOME, so the popover is a small undecorated window. GNOME places it, not us, so it may appear centred. On X11 it is positioned at the click.
- **Right click** keeps the native SNI menu with the same actions. It's the accessible fallback, and it's what tray hosts without click events use.

This is new work for Phase 1: about 1 extra day for the popover, plus testing on each tray host.

### Component mapping

- **Primary button**: one per view, for example "Capture" in the popover and "Save" in a dialog.
- **Outline and ghost buttons**: every other action.
- **Chips**:
  - Gallery filters: mode, type, date range.
  - Selected state is a tint, never inverted.
- **Status pills**: shortcut registration status (`active`, `needs system dialog`, `conflict`, `unavailable`) and the doctor's capability states, always with a text label.
- **Switches**: accent track when on, used for every boolean setting.
- **Empty states and skeletons**: dashed empty gallery; thumbnail skeletons the same size as the real card.
- **Icons**: monochrome symbolic icons only. A small icon set is bundled so the app looks the same under Breeze and Adwaita icon themes.
- **Wordmark (optional)**: the facehash pixel font (Geist Pixel) for the About header only. Never in UI text.

### Design acceptance checks

- Every screen keyboard-navigable with a visible accent focus ring.
- Contrast: the muted text `#8a8a8a` on `#0a0a0a` has a contrast ratio of about 5.7:1, which meets WCAG AA. I worked that out, I didn't measure it.
- With reduced motion enabled (`gtk-enable-animations=false` or the media query), no movement animations remain.
- Screenshots of the popover, gallery and settings on Plasma, Hyprland and GNOME look the same apart from the system-drawn parts listed above.

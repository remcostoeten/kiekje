# Changelog

All notable changes to Kiekje. The full notes for each release, including what was tested, are in [`docs/releases/`](docs/releases/) and on [GitHub Releases](https://github.com/remcostoeten/kiekje/releases).

## [Unreleased]

### Added

- Gallery search tolerates a typo in the file name and ignores accents.
- Check, cross and emoji stamp tools. A faint preview follows the pointer, and a click places the stamp. From the keyboard, the arrow keys move the preview and Space places it.
- The emoji tool has a picker with 64 common emoji and a search field over every Unicode emoji by its English and Dutch names and keywords from CLDR, with typo tolerance. The field also accepts a pasted emoji. A Recent row keeps the last eight stamped emoji, and the picker is fully keyboard driven: type from anywhere, Down into the rows, arrows between emoji, Enter or Space to use one, Escape to close. Pressing `:` in the editor opens it, and chat emoticons such as `:p`, `xD`, `<3` and `(y)` find their emoji. Open it from the toolbar, by right-clicking the image with the emoji tool, or with Shift+F10.
- Several shapes can be selected at once by dragging a frame on empty canvas with Select, with Ctrl+click, or with Ctrl+A. Move, nudge, turn, restyle and delete then apply to all of them and undo as one step. A group has its own frame and turn handle and swings around its centre.
- Text, checks, crosses and emoji can be turned: drag the handle above a selected one or just outside a corner, where a curved arrow shows (Shift keeps 15 degree steps), or press `,` and `.` for 15 degree steps. Both keys can be changed under Drawing keys.
- Scroll resizes the active stamp tool or a selected stamp. A placed stamp is selected right away and the tool returns to Select.
- The stamp tools can get keys under Settings, Shortcuts, Drawing keys. They have none by default.

## [0.1.0-alpha.6] - 2026-10-16

### Added

- Screen recording with `kiekje record`, Record screen in the tray menu or the capture menu. The desktop's picker chooses a screen or window, and running it again saves a WebM file to `~/Videos/Kiekje`. Needs `gst-plugin-pipewire` and `gst-plugins-good`.
- Opt-in temporary upload (off by default): Upload and copy link (Ctrl+U) in the gallery sends a capture to Litterbox, and the link expires after 1, 12, 24 or 72 hours.
- Window capture as its own mode: `kiekje capture window`, a tray item and a shortcut. Works on Plasma 6 and Hyprland.
- Gallery filters for kind, period, dimensions, file size, annotated and unsaved.
- With saving off, captures are copied and kept for 7 days under an Unsaved badge, with a Keep action.
- Digits pick a colour in the editor. Ctrl+scroll changes the size, Shift+scroll the opacity and Alt+scroll the colour.
- An opt-in update check (off by default) and a Check now button in About.
- Settings and `kiekje doctor` say whether a clipboard manager keeps copies after Quit.

### Fixed

- Unplugging a monitor during a selection cancels the capture with one notification.

## [0.1.0-alpha.5] - 2026-10-11

### Added

- Hold Shift while drawing for straight lines at 45 degree steps (line, arrow, pen) and squares or circles (box, ellipse).
- PNG or JPEG saving under Settings > Saving > Format, with a quality slider for JPEG. The clipboard always gets PNG.
- The gallery shows each capture's file type next to its size.
- Rename a capture from the gallery with F2.

### Changed

- GTK 4.14 systems (Ubuntu 24.04, Linux Mint 22) get the full styling. The stylesheet is flattened to plain values at build time.
- Kiekje plays the capture sound itself, so it also works with notification daemons that ignore sound hints, such as mako. Needs the GStreamer base plugins.

### Fixed

- Without the GStreamer base plugins Kiekje logs a warning instead of crashing on capture.

## [0.1.0-alpha.4] - 2026-10-11

### Changed

- The tools, undo, redo and clear are icons in one rounded palette, with colours, size and opacity on the line below. Hover a tool to see its name and key.
- Colours are round dots, and the active one has a ring.
- The grey band behind the editor's toolbar is gone.
- The overlay's inline editor uses the same palette.

## [0.1.0-alpha.3] - 2026-10-10

### Added

- Builds for x86_64 and aarch64: a tarball, a `.deb` and an `.rpm` for each, plus `SHA256SUMS`.
- PKGBUILDs for `kiekje` and `kiekje-bin` attached to the release.
- The binary and packages are checked on Arch, Fedora, openSUSE Tumbleweed, Debian trixie, Ubuntu 24.04 and Ubuntu 25.10 before each release.

### Changed

- gtk4-layer-shell is linked into the binary, so the only runtime dependency is GTK 4.14 or newer.

### Fixed

- GNOME and X11: rendering and copying a capture no longer freezes the UI.
- A second toast on a pinned capture no longer crashes the app.
- Undo, redo or clear while typing text no longer corrupts the drawing.
- Settings saved at the same time no longer overwrite each other.
- A stalled KWin capture times out instead of blocking capture until restart.
- Frames with impossible sizes from the compositor are rejected instead of crashing.
- Save As saves to the path and format you picked.
- A KWin without ScreenShot2 falls back to the portal.
- Long filename templates are limited by the real filename length.
- `manage.sh clean` only deletes Kiekje's own captures.
- `manage.sh backup --with-library` includes the database journal.

## [0.1.0-alpha.2] - 2026-10-10

### Added

- Size (1 to 24 px) and Opacity (10 to 100%) sliders replace Thin, Regular and Bold.
- Right-click the canvas for a menu with every tool, line size, undo, redo and delete.
- Every tool and thinner/thicker lines have a key, changeable under Settings, Shortcuts, Drawing keys.
- Releases ship an x86_64 Linux build with its sha256.

### Changed

- Thinner lines by default: 3 px on a 1080p capture, scaled up to 2x on HiDPI.
- Colour, size and opacity are remembered between editors.
- Changing a slider restyles the selected shape, and a whole drag is one undo step.

## [0.1.0-alpha.1] - 2026-10-06

First alpha: region capture with an overlay, a gallery, an editor with shapes, text and crop, a tray, and settings.

[Unreleased]: https://github.com/remcostoeten/kiekje/compare/v0.1.0-alpha.6...HEAD
[0.1.0-alpha.6]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.6
[0.1.0-alpha.5]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.5
[0.1.0-alpha.4]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.4
[0.1.0-alpha.3]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.3
[0.1.0-alpha.2]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.1

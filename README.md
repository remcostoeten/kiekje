<p align="center">
  <img src="docs/branding/kiekje-icon-720.png" width="88" alt="Kiekje logo" />
</p>

<h1 align="center">Kiekje</h1>

**Kiekje** *(noun)*
/ˈkik.jə/, *Dutch slang, "a quick snapshot or photo."*

A screenshot tray app for Linux. **Kiekje** grabs the screen the moment you trigger it, shows that frozen frame so you can pick the part you want, then copies it, saves it and keeps it in a gallery. It targets KDE Plasma and Hyprland (Omarchy) on Wayland.

<p align="center">
  <img src="https://shieldcn.dev/github/remcostoeten/kiekje/ci.svg?font=jetbrains-mono" alt="CI" />
  <img src="https://shieldcn.dev/github/remcostoeten/kiekje/license.svg?font=jetbrains-mono" alt="license" />
  <img src="https://shieldcn.dev/badge/core-Rust-black.svg?font=jetbrains-mono&logo=rust" alt="core: Rust" />
  <img src="https://shieldcn.dev/badge/toolkit-GTK%204-black.svg?font=jetbrains-mono" alt="toolkit: GTK 4" />
  <img src="https://shieldcn.dev/badge/display-Wayland-black.svg?font=jetbrains-mono" alt="display: Wayland" />
</p>

<p align="center">
  <img src="docs/assets/editor-and-gallery.png" width="100%" alt="The Kiekje gallery on the left, a grid of captures with search and filters, and the annotation editor on the right with a box, an arrow, an ellipse and two numbered steps drawn on a capture." />
</p>
<p align="center">
  <sub>Recorded in a headless Sway session with a lab build.</sub>
</p>

Capture happens first and selection second: the overlay always shows a frame grabbed at trigger time, so what you crop is what was on screen when you pressed the key.

- **Selects on a frozen frame**, across monitors and at fractional scales.
- Copies to the clipboard and saves a PNG named from a template you edit in settings.
- Marks up with boxes, ellipses, arrows, lines, pen, marker, numbered steps, pixelate, text and crop, on the overlay or in a separate editor.
- Keeps every capture in a gallery with grid and list views, search, filters and sorting.
- Sits in the tray with a menu, and binds Print and Shift+Print through the GlobalShortcuts portal, or through a managed bind file on Hyprland.
- Delays a capture, repeats the last region and sends a notification with an optional sound.

Kiekje is pre-release. It has been run in virtual machines running Plasma, Hyprland and Sway. Runs on real hardware with Omarchy, Plasma and GNOME are still open, and GNOME is unverified. Results are in [docs/phase-0-results.md](docs/phase-0-results.md), the architecture is in [docs/proposal.md](docs/proposal.md).

## Usage

Run `kiekje daemon` to start Kiekje in the tray, and turn on Start Kiekje at login in Settings to have it at login. Click the tray icon for the capture menu.

Press Print to capture a region or Shift+Print to capture every screen. Drag to select, then let go: the region is copied and saved. Escape cancels.

A loupe next to the pointer shows the pixels under it, the position in physical pixels and the colour as hex. Z hides or shows it and C copies the hex colour. Turn it off in Settings to start with it hidden.

On KDE Plasma 6 and Hyprland the window under the pointer lights up; click without dragging to select that window. Turn it off in Settings if you prefer a plain drag.

To keep a capture in view while you work, pin it: use Pin on the overlay action bar, Pin on screen in the gallery menu, or `kiekje pin <path>`. Drag a pin to move it, scroll to zoom and press Escape or double-click to close it. On GNOME and X11 a pin is a normal window that may end up behind others.

Turn on "Edit a region on screen before copying and saving it" in settings, or run `kiekje capture region --edit`, to mark up the selection on the overlay. Enter accepts and Escape cancels.

`kiekje gallery` opens the gallery and `kiekje settings` opens settings, where you set the save folder, the file name template, notifications and shortcuts. Settings are stored in `~/.config/kiekje/config.toml`.

On KDE, run `kiekje install-desktop-file` once, because KWin only allows screenshots from an app with a desktop file. `kiekje doctor` reports what the current session supports and how to fix what is missing.

## Install

### Release packages

Each [release](https://github.com/remcostoeten/kiekje/releases) has a `.deb`, an `.rpm` and a tarball for x86_64 and aarch64, plus PKGBUILDs for `kiekje` and `kiekje-bin`. gtk4-layer-shell is built in, so they need only GTK 4.16 or newer. CI installs and starts them on Arch, Fedora, openSUSE Tumbleweed, Debian 13, Ubuntu 24.04 and Ubuntu 25.10 (Fedora, Debian 13 and Ubuntu 24.04 on aarch64) before a release ships. On GTK 4.14 (Ubuntu 24.04, Mint 22) it starts, but the stylesheet does not load.

```bash
sudo apt install ./kiekje_VERSION-1_amd64.deb             # Ubuntu, Debian, Mint
sudo dnf install ./kiekje-VERSION-1.x86_64.rpm            # Fedora
sudo zypper install --allow-unsigned-rpm ./kiekje-VERSION-1.x86_64.rpm   # openSUSE
```

The tarball works anywhere else:

```bash
tar -xzf kiekje-VERSION-x86_64-linux.tar.gz
cd kiekje-VERSION-x86_64-linux
sudo packaging/install.sh ./kiekje
```

### From source

```bash
sudo pacman -S --needed rust gtk4 gtk4-layer-shell
git clone https://github.com/remcostoeten/kiekje.git
cd kiekje
cargo install --path . --locked
kiekje install-desktop-file
```

On other distributions you need GTK 4.16 or newer, because GTK 4.14 rejects the app's stylesheet, and gtk4-layer-shell 1.1. [.github/workflows/ci.yml](.github/workflows/ci.yml) shows how to build gtk4-layer-shell from source.

## Privacy

**Kiekje's code makes no network requests.** Captures stay on your machine: PNGs go to `~/Pictures/Kiekje` by default, the gallery index lives under `$XDG_DATA_HOME/kiekje`, thumbnails and unsaved captures under `$XDG_CACHE_HOME/kiekje`, and settings under `$XDG_CONFIG_HOME/kiekje`. There are no analytics and no update check, and [Cargo.toml](Cargo.toml) lists no HTTP client. The guarantee stops at your desktop: a copied image sits on the clipboard where other apps can read it, and notifications, the screenshot portal and the tray go through your desktop's own services.

## Development

Needs Rust 1.85 or newer (edition 2024), GTK 4.16 and gtk4-layer-shell 1.1.

```bash
cargo build                                                    # debug build
cargo run -- doctor                                            # what this session supports
cargo test                                                     # unit tests
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings   # what CI checks
```

Layout, architecture rules and code style are in [AGENTS.md](AGENTS.md).

<br/>

xxx,<br/>
[Remco Stoeten](https://remcostoeten.com)<br/>
<small>MIT</small><br/>
<small>PS The bundled Geist fonts are under the SIL Open Font License, see [resources/fonts/OFL.txt](resources/fonts/OFL.txt).</small>

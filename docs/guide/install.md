---
title: Install
description: Packages for Arch, Debian, Ubuntu, Fedora, openSUSE, Nix and Flatpak, plus the tarball and a source build.
---

# Install

The release binary links gtk4-layer-shell statically, so the only runtime dependency is GTK 4.14 or newer. CI installs and starts every package below on Arch, Fedora, openSUSE Tumbleweed, Debian 13, Ubuntu 25.10 and Ubuntu 24.04 before a release ships.

## Arch Linux

```bash
yay -S kiekje-bin
```

`kiekje-bin` installs the release binary. `yay -S kiekje` builds the same version from source. Both PKGBUILDs are also attached to every release.

## One line

```bash
curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/master/get.sh | bash
```

The script picks the `.deb`, `.rpm` or tarball for your system from the newest release, checks it against `SHA256SUMS` and installs it with sudo. Two environment variables change what it does:

| Variable | Effect |
| --- | --- |
| `KIEKJE_VERSION=v0.1.0-alpha.6` | Pins a release instead of taking the newest |
| `KIEKJE_FORMAT=tarball` | Skips the native package and installs the tarball |

## Release packages

Download from [Releases](https://github.com/remcostoeten/kiekje/releases). Each release carries x86_64 and aarch64 builds and a `SHA256SUMS` file.

::: code-group

```bash [Debian, Ubuntu, Mint]
sudo apt install ./kiekje_VERSION-1_amd64.deb
```

```bash [Fedora]
sudo dnf install ./kiekje-VERSION-1.x86_64.rpm
```

```bash [openSUSE]
sudo zypper install --allow-unsigned-rpm ./kiekje-VERSION-1.x86_64.rpm
```

```bash [Tarball]
tar -xzf kiekje-VERSION-x86_64-linux.tar.gz
cd kiekje-VERSION-x86_64-linux
sudo packaging/install.sh ./kiekje
```

:::

The tarball works on any distribution with GTK 4.14. Its `install.sh` puts the binary, the desktop file, the icon and the fonts in place.

## Flatpak

Download `kiekje.flatpak` from the [latest release](https://github.com/remcostoeten/kiekje/releases/latest), then:

```bash
flatpak install --user ./kiekje.flatpak
```

The Flatpak uses the desktop portals for capture and global shortcuts. KWin ScreenShot2 and the direct Hyprland protocols are not reachable from the sandbox, so it is slower than the native package on Plasma and Hyprland. See [Desktops](/guide/desktops).

## Nix

```bash
nix profile install github:remcostoeten/kiekje
```

This needs the `nix-command` and `flakes` experimental features. Without them, add `--extra-experimental-features 'nix-command flakes'`.

## From source

Needs Rust 1.85 or newer (edition 2024), GTK 4.14 and gtk4-layer-shell 1.1.

```bash
sudo pacman -S --needed rust gtk4 gtk4-layer-shell
git clone https://github.com/remcostoeten/kiekje.git
cd kiekje
cargo install --path . --locked
kiekje install-desktop-file
```

Ubuntu 24.04 does not package gtk4-layer-shell. The [CI workflow](https://github.com/remcostoeten/kiekje/blob/master/.github/workflows/ci.yml) shows how to build it from source with meson.

## Optional dependencies

| Package | Used for |
| --- | --- |
| GStreamer base plugins | The capture sound. Without them Kiekje logs a warning and stays quiet |
| GStreamer PipeWire plugin and good plugins (`gst-plugin-pipewire`, `gst-plugins-good` on Arch) | Screen recording. `kiekje doctor` names what is missing |
| A clipboard manager (Klipper, GPaste, CopyQ, cliphist) | Keeping a copied capture after Kiekje quits |
| AppIndicator extension | The tray icon on GNOME |

## Packages at a glance

| Format | Architectures | Status |
| --- | --- | --- |
| AUR `kiekje`, `kiekje-bin` | x86_64 | Published from every tagged release |
| `.deb` | x86_64, aarch64 | Attached to each release |
| `.rpm` | x86_64, aarch64 | Attached to each release |
| Tarball with `install.sh` | x86_64, aarch64 | Attached to each release |
| Flatpak bundle | x86_64 | Attached to each release, portal only |
| Nix flake | any | `nix profile install github:remcostoeten/kiekje` |
| Cargo | any | Builds from source |

Next: [Getting started](/guide/getting-started).

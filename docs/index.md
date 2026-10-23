---
layout: home
title: Kiekje
titleTemplate: Screenshots for KDE Plasma and Hyprland

hero:
  name: Kiekje
  text: The frame freezes when you press the shortcut.
  tagline: A screenshot tray app for Linux. It grabs the screen the moment you trigger it, shows that still frame so you can pick the part you want, then copies it, saves it and keeps it in a gallery.
  image:
    src: /kiekje-icon.png
    alt: Kiekje
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Install
      link: /guide/install
    - theme: alt
      text: GitHub
      link: https://github.com/remcostoeten/kiekje

features:
  - icon: ▣
    title: Frozen frame
    details: Capture happens before selection. The overlay always shows the screen as it was at trigger time, so a 3 second delay or a slow drag changes nothing.
    link: /guide/capture
    linkText: Capture
  - icon: ✎
    title: Annotate in place
    details: Draw on the region right on the overlay, or in the editor window. Boxes, arrows, pen, marker, numbered steps, text, pixelate and crop, with undo and redo.
    link: /guide/annotate
    linkText: Annotate
  - icon: ▦
    title: A gallery that keeps up
    details: Grid and list views, search, filters, missing-file detection and file actions. Thumbnails load only for what is on screen, tested at 10,000 captures.
    link: /guide/gallery
    linkText: Gallery
  - icon: ◈
    title: Native on Plasma and Hyprland
    details: KWin ScreenShot2 over D-Bus, ext-image-copy-capture and wlr-screencopy on wlroots, the portal on GNOME. No grim, slurp or wl-copy subprocesses.
    link: /guide/desktops
    linkText: Desktops
  - icon: ●
    title: Record the screen
    details: kiekje record starts a WebM recording through the ScreenCast portal. Run it again to stop and save to your Videos folder.
    link: /guide/record-and-share
    linkText: Record
  - icon: ◌
    title: Quiet by default
    details: No analytics and no network requests unless you turn on the update check or temporary uploads. Captures live in XDG Pictures.
    link: /guide/privacy
    linkText: Privacy
---

<div class="home-shot">
  <figure>
    <img src="/editor-and-gallery.png" alt="The Kiekje gallery on the left, a grid of captures with search and filters, and the annotation editor on the right with a box, an arrow, an ellipse and two numbered steps drawn on a capture." />
    <figcaption>Gallery and editor, recorded in a headless Sway session with a lab build</figcaption>
  </figure>
</div>

<div class="status-strip">
  <span>Rust and GTK 4</span>
  <span>Wayland first</span>
  <span>Pre-release</span>
  <span>MIT</span>
</div>

## Install

::: code-group

```bash [Arch]
yay -S kiekje-bin
```

```bash [One line]
curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/master/get.sh | bash
```

```bash [Debian, Ubuntu]
sudo apt install ./kiekje_VERSION-1_amd64.deb
```

```bash [Fedora]
sudo dnf install ./kiekje-VERSION-1.x86_64.rpm
```

```bash [Nix]
nix profile install github:remcostoeten/kiekje
```

:::

The [install page](/guide/install) covers openSUSE, the tarball, Flatpak and building from source.

::: warning Pre-release
Kiekje ships as alphas from GitHub Releases. Everything has been tested in virtual machines and headless sessions. Runs on real hardware with Omarchy, Plasma and GNOME are still open and tracked in [issue #41](https://github.com/remcostoeten/kiekje/issues/41). The [support matrix](/reference/support) says what is lab tested and what is unverified.
:::

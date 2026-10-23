---
title: Design system
description: The tokens, type, components and GTK constraints behind the Kiekje look, and how this site reuses them.
---

# Design system

Kiekje is dark only. The look is a near-black ground, monochrome greys mixed from the foreground into the background, one orange accent, Geist for content and uppercase Geist Mono for labels and buttons. Section 10 of the [proposal](/project/proposal#_10-design-and-look-and-feel) records the source and the GTK measurements; this page is the working reference.

## Tokens

Tokens live once on `:root` in `resources/style.css`. Every tint is a `color-mix()` of an existing token. No other colours are added.

| Token | Value | Use |
| --- | --- | --- |
| `--bg` | `#0a0a0a` | Window ground |
| `--fg` | `#ededed` | Text and icons |
| `--muted` | `#8a8a8a` | Secondary text, labels |
| `--accent` | `#fe5101` | Primary button, selection border, focus ring, active marks |
| `--surface` | 4% `--fg` in `--bg` | Panels and cards |
| `--raised` | 7% `--fg` in `--bg` | Controls at rest |
| `--raised-hi` | 11% `--fg` in `--bg` | Controls on hover |
| `--stroke`, `--stroke-hi` | 11% and 18% `--fg` in `--bg` | 1 px borders |
| `--line` | 16% `--fg`, transparent | Dividers, the dot grid |
| `--wash`, `--accent-wash` | 6% `--fg`, 14% `--accent` | Hover and selection tints |
| `--specular` | inset 1 px, 6% `--fg` | The faint top highlight on raised surfaces |
| `--radius`, `--radius-lg` | 8 px, 12 px | Controls, panels |

GTK's CSS parser needs a space after the colon in a custom property. `--accent: #fe5101;` parses, `--accent:#fe5101;` does not. `build.rs` flattens the tokens and every `color-mix()` into plain values for GTK 4.14 and fails the build on any other form.

## Type

Geist for content and uppercase Geist Mono with letter spacing for section labels, chips, buttons and readouts. Both are bundled under the SIL Open Font License and registered with fontconfig for the process at startup, so the app looks the same whether or not they are installed. This site loads the same two files.

## Components

- **One primary button per view.** A solid accent fill. Every other action is outline or ghost.
- **Selection is a tint, never an inverted fill.** Chips, list rows and gallery cards show selection with `--accent-wash` and an accent stroke.
- **Depth from layered greys.** Settings sections sit in raised groups with a 1 px stroke and the specular highlight. Filters are segmented controls. Dashed lines are for empty states only.
- **Status pills** always carry a text label: `active`, `needs system dialog`, `conflict`, `unavailable`.
- **Switches** use the accent track when on.
- **Monochrome symbolic icons** only, bundled so Breeze and Adwaita render the same set. No emoji anywhere in the UI.
- **Corner marks** are drawn by a widget, since GTK has no `::before` or `::after`.
- **Motion** is short ease-out. With animations disabled, no movement remains.
- **Every interactive element** is keyboard reachable with a visible accent focus ring.

## Where it applies

| Surface | Styled by Kiekje |
| --- | --- |
| Settings, gallery, About, editor | Yes |
| Selection overlay | Yes: dimmed frame, accent border, 7 px corner handles, mono size readout |
| Tray popover on left click | Yes, a layer-shell window Kiekje draws |
| Native tray menu on right click | No, drawn by Plasma, Waybar or the GNOME extension |
| Desktop notifications | No, drawn by the notification daemon |
| Portal and KDE permission dialogs | No, system UI |

## This site

The documentation theme maps the same tokens onto VitePress: `--vp-c-bg` is `--bg`, `--vp-c-brand-1` is `--accent`, panels use `--surface` and `--raised` with `--stroke` borders, labels use the mono caps treatment and the home page carries the dot grid. The stylesheet lives in `docs/.vitepress/theme/style.css`.

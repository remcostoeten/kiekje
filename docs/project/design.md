---
title: Design system
description: The tokens, type, components and GTK constraints behind the Kiekje look, and how this site reuses them.
---

# Design system

Kiekje follows a "Crisp tool" direction: warm graphite surfaces, one orange accent, Geist in sentence case, 1 px borders and 6 to 10 px radii. It reads like a developer tool: dense, calm and precise. There is a dark theme and a warm paper light one; the tokens below list both.

## Tokens

Tokens live once on `:root` in `resources/style.css`, with the light values in `:root.light`. Every tint is a `color-mix()` of an existing token.

| Token | Dark | Light | Use |
| --- | --- | --- | --- |
| `--bg` | `#090908` | `#f8f6f2` | Window and popover ground |
| `--deep` | `#050504` | `#f1eee8` | Sidebar, hint bar |
| `--surface` | `#111110` | `#fffdf9` | Cards and groups |
| `--raised`, `--raised-hi` | `#161615`, `#1e1e1c` | `#f3f0ea`, `#e8e3d9` | Controls at rest, on hover |
| `--stroke`, `--stroke-hi`, `--stroke-top` | `#1b1b19`, `#252523`, `#323230` | `#e6e0d5`, `#d7d0c3`, `#c2baab` | 1 px borders, light to strong |
| `--fg`, `--muted`, `--faint` | `#efeeea`, `#a3a099`, `#6b6962` | `#1d1b17`, `#686256`, `#857e71` | Text, secondary text, captions |
| `--accent`, `--accent-fill`, `--accent-text` | `#fe5101`, `#fe5101`, `#ff8a52` | `#fe5101`, `#f04c00`, `#c23d00` | Focus tint, fills, accent text |
| `--danger`, `--warn`, `--ok` | `#e5484d`, `#e3b25d`, `#5cc48e` | `#c43d36`, `#a66b10`, `#2a8655` | Danger and errors, warnings, success |
| `--wash`, `--accent-wash`, `--accent-line` | 5% `--fg`, 14% and 45% `--accent` | same | Hover, selection tint, badge stroke |
| `--radius`, `--radius-lg`, `--radius-xl` | 6 px, 8 px, 10 px | same | Controls, cards, popover and windows |

GTK's CSS parser needs a space after the colon in a custom property. `--accent: #fe5101;` parses, `--accent:#fe5101;` does not. `build.rs` flattens the tokens and every `color-mix()` into plain values for GTK 4.14 and fails the build on any other form.

## Type

Geist for everything, in sentence case: 13px for rows and actions, 12px for captions, 17px for page titles. Geist Mono only for values that are code: file name templates and paths. Both fonts are bundled under the SIL Open Font License and registered with fontconfig for the process at startup, so the app looks the same whether or not they are installed.

## Components

- **One primary button per view.** A solid accent fill with white text. Every other action is outline or ghost.
- **Selection is a tint, never an inverted fill.** The selected popover row, list rows and gallery cards use `--accent-wash` or `--accent-strong` as a background, with the icon and text brightened. They never get a border.
- **Command palette popover.** A search field, sections for Capture and Record, one row per action with an icon, a label and right-aligned keycaps. Arrow keys move the selection, Enter runs it, Escape clears the search and then closes.
- **Keycaps** show every shortcut, in the popover and in the shortcut recorder.
- **Settings** is a sidebar with icon navigation and a page header with a one-line description. Sections sit in cards with a 1 px stroke.
- **Danger** is red outline; armed, it is a solid red fill.
- **Switches** are 32 by 18 px with the accent track when on.
- **Monochrome symbolic icons** only, bundled so Breeze and Adwaita render the same set. They are filled paths, because GTK recolours fills and ignores strokes. No emoji anywhere in the UI.
- **Motion** is short ease-out. With animations disabled, no movement remains.
- **State feedback is colour only.** Hover, focus, focus-visible, focus-within, selected, checked and active change the background colour, an icon fill or the text colour. They never add or change an outline, a border, a border colour or a box-shadow ring. A border at rest stays as it is.
- **Every interactive element** is keyboard reachable with a visible focus state. Hover is the faint `--wash`; focus-visible and selected use a stronger tint (`--focus-wash`, `--accent-strong`, `--accent-focus`) and brighter text or icon, so the two can be told apart.

## Where it applies

| Surface | Styled by Kiekje |
| --- | --- |
| Settings, gallery, About, editor | Yes |
| Selection overlay | Yes: dimmed frame, accent border, 7 px corner handles, size readout |
| Tray popover on left click | Yes, a layer-shell window Kiekje draws |
| Native tray menu on right click | No, drawn by Plasma, Waybar or the GNOME extension |
| Desktop notifications | No, drawn by the notification daemon |
| Portal and KDE permission dialogs | No, system UI |

## This site

The documentation theme is separate. It shares the orange accent and keeps its mono caps labels and dot grid. Its stylesheet lives in `docs/.vitepress/theme/style.css`.

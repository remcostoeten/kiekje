---
title: Configuration
description: Every key in config.toml with its default, and the filename template tokens.
---

# Configuration

Settings live in `$XDG_CONFIG_HOME/kiekje/config.toml`, normally `~/.config/kiekje/config.toml`. Kiekje writes the file with defaults on first start, saves it through a temporary file and a rename, and applies most changes immediately. The Settings window edits the same file. Unknown keys are an error: the file is rejected, defaults apply, and `kiekje doctor` names the key.

The full default file:

```toml
version = 1

[general]
check_for_updates = false

[storage]
filename = "kiekje-{mode}-{date}-{time}"
format = "png"
quality = 90

[saving]
enabled = true
keep_unsaved_days = 7

[capture]
after_selection = "finish"

[overlay]
magnifier = true
magnifier_pixels = 15
slow_pointer = "eighth"
window_snap = true
hints = true

[notifications]
enabled = true
sound = true

[drawing]
color = "orange"
size = 3
opacity = 100

[drawing.keys]
select = "v"
box = "r"
ellipse = "e"
arrow = "a"
line = "l"
pen = "p"
marker = "m"
step = "n"
text = "t"
check = "y"
cross = "u"
emoji = "i"
pixelate = "x"
crop = "c"
thinner = "["
thicker = "]"

[upload]
enabled = false
expiry = "24h"

[shortcuts]
capture-region = "Print"
capture-fullscreen = "SHIFT+Print"
```

## `version`

The config format version. Kiekje migrates older files on load and refuses newer ones. Currently `1`.

## `[general]`

| Key | Default | Meaning |
| --- | --- | --- |
| `check_for_updates` | `false` | Ask the GitHub releases API for a newer version at startup and at most once a day, with one notification per new version |

## `[storage]`

| Key | Default | Meaning |
| --- | --- | --- |
| `folder` | unset | Where captures are saved. Absolute, or starting with `~/`. Unset means `$XDG_PICTURES_DIR/Kiekje` |
| `filename` | `"kiekje-{mode}-{date}-{time}"` | The filename template, without extension |
| `format` | `"png"` | `"png"` or `"jpeg"` (`"jpg"` is accepted). The clipboard always gets PNG |
| `quality` | `90` | JPEG quality from 1 to 100. Ignored for PNG |

### Filename tokens

| Token | Value |
| --- | --- |
| `{mode}` | `region`, `fullscreen`, `window` or `repeat` |
| `{date}` | `YYYY-MM-DD` |
| `{time}` | `HH-MM-SS` |
| `{year}`, `{month}`, `{day}` | The date parts |
| `{hour}`, `{minute}`, `{second}` | The time parts |
| `{width}`, `{height}` | The capture size in pixels |

Text around the tokens is kept as is. A template that would exceed the file system's name length is rejected. Files are created exclusively, so two captures in the same second never overwrite each other. Settings shows a live preview.

## `[saving]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Write captures to the folder. Off: copy only, keep them in the cache as Unsaved |
| `keep_unsaved_days` | `7` | Days an unsaved capture stays in `$XDG_CACHE_HOME/kiekje/unsaved` before it is pruned at startup |

## `[capture]`

| Key | Default | Meaning |
| --- | --- | --- |
| `after_selection` | `"finish"` | `"finish"` copies and saves when you let go. `"edit"` keeps the overlay open with the drawing tools |

## `[overlay]`

| Key | Default | Meaning |
| --- | --- | --- |
| `magnifier` | `true` | Show the loupe with the pixel colour. <kbd>Z</kbd> hides it for one capture |
| `magnifier_pixels` | `15` | How many pixels the loupe shows across |
| `slow_pointer` | `"eighth"` | Pointer speed while holding <kbd>Shift</kbd>: `"quarter"`, `"eighth"` or `"sixteenth"` |
| `window_snap` | `true` | Highlight the window under the pointer and take it with one click. Plasma 6 and Hyprland |
| `hints` | `true` | Show the key hint bar at the bottom of the overlay |

## `[notifications]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Show a notification after each capture, with Open and Show in folder |
| `sound` | `true` | Play the capture sound. Uses the sound theme's shutter or a bundled fallback and needs the GStreamer base plugins |

## `[drawing]`

| Key | Default | Meaning |
| --- | --- | --- |
| `color` | `"orange"` | `"orange"`, `"red"`, `"yellow"`, `"green"`, `"blue"`, `"white"` or `"black"` |
| `size` | `3` | Line width in pixels, 1 to 24 |
| `opacity` | `100` | 10 to 100 |

All three are written back when you change them in an editor.

### `[drawing.keys]`

One key per drawing action, pressed without modifiers. A letter, a digit or punctuation; an empty string turns the key off. Two actions can't share a key, and a config that does is rejected.

| Key | Default | Action |
| --- | --- | --- |
| `select` | `"v"` | Select tool |
| `box` | `"r"` | Box |
| `ellipse` | `"e"` | Ellipse |
| `arrow` | `"a"` | Arrow |
| `line` | `"l"` | Line |
| `pen` | `"p"` | Pen |
| `marker` | `"m"` | Marker |
| `step` | `"n"` | Numbered step |
| `text` | `"t"` | Text |
| `check` | `"y"` | Check |
| `cross` | `"u"` | Cross |
| `emoji` | `"i"` | Emoji |
| `pixelate` | `"x"` | Pixelate |
| `crop` | `"c"` | Crop, editor window only |
| `thinner` | `"["` | Thinner lines |
| `thicker` | `"]"` | Thicker lines |

## `[upload]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Show Upload and copy link in the gallery, which sends the file to Litterbox |
| `expiry` | `"24h"` | How long the link lives: `"1h"`, `"12h"`, `"24h"` or `"72h"` |

## `[shortcuts]`

| Key | Default | Action |
| --- | --- | --- |
| `capture-region` | `"Print"` | Freeze the screen and select a region |
| `capture-fullscreen` | `"SHIFT+Print"` | Capture every monitor |
| `capture-window` | off | Freeze the screen and pick a window |

A shortcut is modifiers and a key joined with `+`: `CTRL`, `ALT`, `SHIFT` and `LOGO` (also written `SUPER` or `META`), then a key name such as `Print`, `F9` or `s`. An empty string turns a shortcut off. The recorder in Settings enforces the same rules the parser does:

- A letter, digit or punctuation key needs <kbd>Ctrl</kbd>, <kbd>Alt</kbd> or <kbd>Super</kbd>, or it would fire while typing. <kbd>Print</kbd> and the function keys work on their own.
- Two actions can't share a trigger.
- Combinations the desktop reserves are refused.

Shortcuts are registered through the GlobalShortcuts portal, kglobalaccel or the Hyprland include, whichever the session offers. See [Desktops](/guide/desktops).

---
title: Capture
description: Region, full screen, window, repeat and text modes, the overlay, delay, multi-monitor handling, the loupe and the colour picker.
---

# Capture

Every capture starts with a frozen frame. Kiekje grabs all monitors at trigger time, keeps them as raw buffers and opens one overlay surface per monitor showing its own frame. Whatever you pick comes out of those buffers, cropped at native resolution and encoded once.

## Modes

| Mode | How to start it | What it takes |
| --- | --- | --- |
| Region | <kbd>Print</kbd>, `kiekje capture`, Capture region in the tray | The rectangle you drag on the frozen frame |
| Full screen | <kbd>Shift</kbd>+<kbd>Print</kbd>, `kiekje capture fullscreen` | Every monitor stitched into one image, no overlay |
| Window | `kiekje capture window`, Capture window in the tray, a shortcut without a default key | The window you click or hover and confirm with <kbd>Enter</kbd> |
| Repeat | `kiekje capture repeat`, Repeat last region in the tray | The last selected region again, without the overlay |
| Text | `kiekje capture text`, Copy text from region in the tray, a shortcut without a default key, Text in the overlay's top bar | The text in the region you drag, copied as plain text |

Window mode needs the compositor to report where its windows are. That exists on KDE Plasma 6 and Hyprland. Elsewhere the tray item and shortcut are disabled with the reason, the CLI exits non-zero and `doctor` lists window capture as unavailable. Window capture crops the frozen frame, so anything on top of the window ends up in the capture.

## The overlay

The overlay dims the frozen frame and shows the selection with a 1 px accent border, corner handles and a size readout. These keys work before you select:

| Key | Action |
| --- | --- |
| Drag | Select a region |
| Click | Take the window under the pointer (Plasma 6, Hyprland) |
| <kbd>F</kbd> | Take the monitor under the pointer |
| <kbd>W</kbd>, <kbd>Enter</kbd> | Take the hovered window |
| <kbd>Shift</kbd>+<kbd>R</kbd> | Take the last region |
| <kbd>G</kbd> | Cycle Screenshot, Video, GIF and Text; a selection then records or copies its text instead of taking a screenshot |
| <kbd>D</kbd>, a tool key or a tool in the top bar | Start drawing first, pick the region afterwards with <kbd>S</kbd> |
| <kbd>Z</kbd> | Show or hide the loupe for this capture |
| <kbd>C</kbd> | Copy the colour under the pointer |
| <kbd>?</kbd> | Show or hide the hint bar |
| <kbd>Esc</kbd> | Cancel, nothing is saved |

A small bar at the top of the overlay shows the same choice as four buttons: Screenshot, Video, GIF and Text. Above it sits the inline editor's drawing palette, with Region picked because a drag selects a region. Clicking another tool starts drawing on that monitor with it, like <kbd>D</kbd> does with the pen, and the colour, size and opacity picked there carry over into the editor. With Video or GIF selected, a drag, a click on a window, <kbd>F</kbd> or <kbd>Enter</kbd> starts a recording of that region instead of taking a screenshot, and the inline editor is skipped. See [Record and share](/guide/record-and-share#record-a-region). The choice resets to Screenshot for every capture.

Hold <kbd>Shift</kbd> while moving to slow the pointer to an eighth of its speed, which makes single pixels easy to hit. The factor is set under Settings, Capture, Magnifier.

With **Snap to windows** on, the window under the pointer lights up and one click takes it. Without a window geometry source the overlay stays a plain drag.

### Window snapping and the hint bar

The hint bar lists the keys at the bottom of the overlay, and every key that does something is also a button. It stays while you draw, with <kbd>S</kbd> for the region, <kbd>Enter</kbd> to accept, <kbd>G</kbd> to record, <kbd>Z</kbd> for the loupe and <kbd>C</kbd> to copy a colour while the loupe shows. <kbd>G</kbd> records the region when there is one and otherwise goes back to selecting with Video on; the annotations are dropped either way. `[overlay] hints = false` hides the bar for good; <kbd>?</kbd> hides it once.

## Copy text from a region

Text mode reads the words in a region and puts them on the clipboard as plain text instead of an image. Start it with `kiekje capture text`, Copy text from region in the tray, or a shortcut you set under Settings, Shortcuts. In an open overlay, pick Text in the top bar or press <kbd>G</kbd> until it is chosen, then select a region the usual way. While drawing on a region, Copy text in the action bar or <kbd>Ctrl</kbd>+<kbd>T</kbd> does the same with the region as it stands; the annotations are ignored.

[Tesseract](https://github.com/tesseract-ocr/tesseract) reads the text on your computer and nothing is sent anywhere. Before reading, Kiekje turns the region grey, inverts it when the background is dark and doubles it in size when it is at most 4 megapixels, which is what Tesseract reads screen text best from. A notification shows the first lines of what was copied. When nothing is found the clipboard stays as it was.

Kiekje loads `libtesseract.so.5` when it starts. Install Tesseract with English data (`tesseract` and `tesseract-data-eng` on Arch, `libtesseract5` and `tesseract-ocr-eng` on Debian and Ubuntu, `tesseract-libs` and `tesseract-langpack-eng` on Fedora). Without it the Text button, the tray item and the shortcut are disabled with the reason, `kiekje capture text` exits non-zero and `doctor` says what is missing.

Text is read in English until you switch on more languages under Settings, Capture, Text, which lists every language Tesseract has data for. Accented letters such as ë and ö come out right only with that language on, so Dutch text needs `nld`. Each extra language makes reading a little slower. Indentation in code is not kept.

## Selecting across monitors

The selection is kept in global logical coordinates and converted per monitor on confirm: `(logical - monitor_origin) * monitor_scale`, rounded outward to whole pixels. That handles negative origins, rotation through the output transform, and fractional scale through `wp_fractional_scale_v1` or the output size against the buffer size. A selection that crosses a monitor edge is cropped pixel-exactly from each frame and stitched. Lab tested with 1.0 + 1.5, 1.0 + 2.0 and rotated layouts on Plasma, sway and Hyprland.

If a monitor disconnects while the overlay is open, every overlay surface closes and the capture is cancelled with one notification. No file is written and nothing lands in the gallery.

## Delay

```bash
kiekje capture --delay 5
kiekje capture fullscreen --delay 10
```

`--delay` waits up to 60 seconds before freezing the screen. The tray popover has the same delay for each mode. Use it to open a menu or a tooltip first.

## The loupe and the colour picker

A magnifier follows the pointer with the pixel grid, the position and the hex colour under it. Scrolling zooms it, <kbd>Z</kbd> hides it for one capture, and `[overlay] magnifier = false` turns it off. `magnifier_pixels` sets how many pixels it shows across.

<kbd>C</kbd> copies the hex colour under the pointer to the clipboard. Picked colours are listed under Colours in the gallery, with a copy action and a Clear colours entry in the danger zone.

## After the selection

By default the region is copied and saved when you let go. Two things change that:

- `kiekje capture --edit`, the tray's Annotate region item, or `[capture] after_selection = "edit"` (the default) keep the overlay open with the drawing tools above the region and Accept, Cancel, Save, Save as and Copy below it. See [Annotate](/guide/annotate).
- `--no-save` copies to the clipboard only. With `[saving] enabled = false` every capture is treated that way and listed in the gallery as Unsaved. See [Gallery](/guide/gallery).

A capture can also be pinned on screen as a floating window from the overlay, the gallery or `kiekje pin <path>`. Drag it around, scroll to zoom, <kbd>Esc</kbd> closes it. On GNOME a pinned window is not guaranteed to stay on top.

## Output

- **Clipboard.** PNG, also from the background daemon on Plasma through `ext_data_control_manager_v1`. The clipboard always gets PNG, whatever the save format.
- **File.** PNG or JPEG in the capture folder, named by the [filename template](/reference/configuration#storage), written with an exclusive create so two captures in the same second never collide.
- **Notification.** With the image and Open and Show in folder actions. The capture sound comes from the sound theme's shutter or a bundled fallback and needs the GStreamer base plugins.

Backends are chosen at runtime from the Wayland registry, D-Bus names, portal versions and session variables. `--backend kwin|wlroots|portal` forces one for a single capture; `auto` is the default. The [desktops page](/guide/desktops) lists what each compositor gets.

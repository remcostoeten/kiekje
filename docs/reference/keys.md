---
title: Keys
description: Every key on the overlay, in window capture, in the inline editor and in the editor window.
---

# Keys

## Overlay, before selecting

| Key | Action |
| --- | --- |
| Drag | Select a region |
| Click | Take the window under the pointer (Plasma 6, Hyprland) |
| <kbd>F</kbd> | Take the monitor under the pointer |
| <kbd>W</kbd>, <kbd>Enter</kbd> | Take the hovered window |
| <kbd>Shift</kbd>+<kbd>R</kbd> | Take the last region |
| <kbd>D</kbd>, or a tool key | Start drawing first, pick the region afterwards with <kbd>S</kbd> |
| <kbd>Shift</kbd> while moving | Slow the pointer for pixel-exact selection |
| <kbd>Z</kbd> | Show or hide the loupe |
| Scroll | Zoom the loupe |
| <kbd>C</kbd> | Copy the colour under the pointer |
| <kbd>?</kbd> | Show or hide the hint bar |
| <kbd>Esc</kbd> | Cancel |

## Overlay, window capture

| Key | Action |
| --- | --- |
| Click | Take the window under the pointer |
| <kbd>Enter</kbd>, <kbd>W</kbd> | Take the hovered window |
| <kbd>?</kbd> | Show or hide the hint bar |
| <kbd>Esc</kbd> | Cancel |

## Inline editor

| Key | Action |
| --- | --- |
| <kbd>Enter</kbd> | Accept: copy and save |
| <kbd>Esc</kbd> | Cancel |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy only |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | Save only |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> | Save as |
| Arrows | Move the region |
| <kbd>Ctrl</kbd>+arrows | Resize the region |
| <kbd>Ctrl</kbd>+<kbd>Z</kbd> | Undo |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd>, <kbd>Ctrl</kbd>+<kbd>Y</kbd> | Redo |
| <kbd>Shift</kbd> while drawing | Straight lines at 45 degree steps (line, arrow, pen), squares and circles (box, ellipse) |

## Drawing tools

Defaults, rebindable under Settings, Shortcuts, Drawing keys or in [`[drawing.keys]`](/reference/configuration#drawing-keys).

| Key | Tool |
| --- | --- |
| <kbd>V</kbd> | Select |
| <kbd>R</kbd> | Box |
| <kbd>E</kbd> | Ellipse |
| <kbd>A</kbd> | Arrow |
| <kbd>L</kbd> | Line |
| <kbd>P</kbd> | Pen |
| <kbd>M</kbd> | Marker |
| <kbd>N</kbd> | Numbered step |
| <kbd>T</kbd> | Text |
| <kbd>Y</kbd> | Check |
| <kbd>U</kbd> | Cross |
| <kbd>I</kbd> | Emoji |
| <kbd>:</kbd> | Emoji picker |
| <kbd>X</kbd> | Pixelate |
| <kbd>C</kbd> | Crop (editor window) |
| <kbd>[</kbd>, <kbd>]</kbd> | Thinner and thicker lines |
| <kbd>1</kbd> to <kbd>7</kbd> | Pick a colour |
| <kbd>Ctrl</kbd>+scroll | Size |
| <kbd>Shift</kbd>+scroll | Opacity |
| <kbd>Alt</kbd>+scroll | Colour |
| <kbd>Delete</kbd> | Delete the selected shape |
| Scroll | Size, with a check, cross or emoji tool or stamp selected |
| Arrow keys, <kbd>Space</kbd> | Move the stamp cursor and place the stamp |
| <kbd>Menu</kbd>, <kbd>Shift</kbd>+<kbd>F10</kbd> | Open the context menu, or the emoji picker with the Emoji tool |

## Gallery

| Key | Action |
| --- | --- |
| <kbd>F2</kbd> | Rename the selected capture |
| <kbd>Ctrl</kbd>+<kbd>U</kbd> | Upload and copy link, with temporary upload turned on |

## Global shortcuts

| Shortcut | Action |
| --- | --- |
| <kbd>Print</kbd> | Capture a region |
| <kbd>Shift</kbd>+<kbd>Print</kbd> | Capture every screen |
| none | Capture a window; bind one in Settings, Shortcuts |

All three are rebindable with the recorder in Settings or in [`[shortcuts]`](/reference/configuration#shortcuts).

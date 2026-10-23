---
title: Annotate
description: The inline editor on the overlay, the editor window, every tool and how shapes are edited.
---

# Annotate

Kiekje has two places to draw. The **inline editor** keeps the overlay open after a selection, Spectacle style, with the tools above the region and the actions below it. The **editor window** opens from the gallery, or straight after a capture in modes that have no overlay. Both share the same tools, keys and palette.

## Opening the editor

| Where | How |
| --- | --- |
| On the overlay | `kiekje capture --edit`, Annotate region in the tray, or `[capture] after_selection = "edit"` |
| On the overlay, before selecting | Press <kbd>D</kbd> or a tool key to draw on the whole frame first, then <kbd>S</kbd> to pick the region |
| Editor window | Edit in the gallery, or `capture fullscreen --edit` and `capture window --edit` |

The inline editor works on the whole frozen monitor frame with the region as its crop. The region keeps its handles, arrows move it and <kbd>Ctrl</kbd>+arrows resize it, so you can draw outside the current crop and widen it afterwards.

## Tools

| Tool | Key | Notes |
| --- | --- | --- |
| Select | <kbd>V</kbd> | Move, resize, restyle and delete placed shapes |
| Box | <kbd>R</kbd> | <kbd>Shift</kbd> makes it square |
| Ellipse | <kbd>E</kbd> | <kbd>Shift</kbd> makes it a circle |
| Arrow | <kbd>A</kbd> | <kbd>Shift</kbd> snaps to 45 degree steps |
| Line | <kbd>L</kbd> | <kbd>Shift</kbd> snaps to 45 degree steps |
| Pen | <kbd>P</kbd> | Freehand; <kbd>Shift</kbd> draws straight segments |
| Marker | <kbd>M</kbd> | Translucent highlight |
| Numbered step | <kbd>N</kbd> | Each click places the next number |
| Text | <kbd>T</kbd> | Click, type, click outside to finish |
| Check | <kbd>Y</kbd> | Places a check mark in the current colour |
| Cross | <kbd>U</kbd> | Places a cross in the current colour |
| Emoji | <kbd>I</kbd> | Places the chosen emoji; needs an emoji font; uses Apple Color Emoji when installed, otherwise the system emoji font |
| Pixelate | <kbd>X</kbd> | Drag over the area to hide |
| Crop | <kbd>C</kbd> | Editor window only; the overlay crops with the region |

Every tool key and the thinner and thicker keys can be rebound under Settings, Shortcuts, Drawing keys, or in `[drawing.keys]`. Keys are single characters pressed without modifiers.

## Stamps

Check, Cross and Emoji place one stamp per click. Press and drag to adjust the position before releasing. A faded preview follows the pointer and shows the size the stamp will have. Once placed, the stamp is selected and the tool returns to Select, so scroll sizes it and the arrows move it; press the tool key again for another.

A selected text, check, cross or emoji has a round handle above it. Drag it, or drag just outside any corner where a curved arrow appears as in Photoshop's transform tool, to turn the shape freely around its centre; near a right angle it snaps, and holding <kbd>Shift</kbd> while dragging keeps it on 15 degree steps, as in Photoshop. <kbd>,</kbd> and <kbd>.</kbd> turn in steps of 15 degrees.

| Input | Effect |
| --- | --- |
| Scroll | Size of the stamp, also for a selected stamp |
| Arrow keys | Move the stamp cursor, <kbd>Shift</kbd> moves it ten times further |
| <kbd>Space</kbd> | Place the stamp at the cursor |
| Right-click, <kbd>Menu</kbd> or <kbd>Shift</kbd>+<kbd>F10</kbd> | Open the emoji picker with the Emoji tool |
| <kbd>:</kbd> | Open the emoji picker with any tool, with `:` already typed |

The emoji picker opens with the search field focused. It shows a Recent row with the last eight emoji you stamped, then 64 common emoji, and searches all of Unicode's emoji by their English and Dutch names and keywords, so `fire`, `vuur`, `vlam` and `insect` all work, and a small typo such as `rockett` still finds the rocket. The names come from Unicode CLDR. Paste or type any other emoji into the field and press <kbd>Enter</kbd> to use it. The button next to the Emoji tool opens the same picker and shows the current choice. With an emoji selected, picking from that button changes the selected emoji. The recent row is kept in the library; the chosen emoji itself is not remembered between editors.

| Key in the picker | Effect |
| --- | --- |
| Letters | Search, wherever the focus is |
| Emoticons | `:)`, `:p`, `xD`, `<3`, `(y)` and the other MSN Messenger ones pick their emoji; `:fire:` works too |
| <kbd>Enter</kbd> in the field | Use the best match, or the pasted emoji |
| <kbd>Down</kbd> | From the field into the Recent row, then the grid |
| Arrow keys | Move between emoji; <kbd>Up</kbd> from the first row goes back to the field |
| <kbd>Enter</kbd> or <kbd>Space</kbd> on an emoji | Use it |
| <kbd>Escape</kbd> | Close the picker |

A line under the grid names the emoji under the pointer or focus.

While a stamp tool is active the arrow keys move the stamp cursor instead of the region on the inline editor. Pick another tool to move the region again.

## Colour, size and opacity

The palette below the tools holds seven colours as round dots, a size slider from 1 to 24 px and an opacity slider from 10 to 100%. All three are remembered between editors in `[drawing]`. The default size is 3 px on a 1080p capture, scaled up to 2x on HiDPI.

| Input | Effect |
| --- | --- |
| Digits <kbd>1</kbd> to <kbd>7</kbd> | Pick a colour |
| <kbd>[</kbd> and <kbd>]</kbd> | Thinner and thicker lines |
| <kbd>Ctrl</kbd>+scroll | Size |
| <kbd>Shift</kbd>+scroll | Opacity |
| <kbd>Alt</kbd>+scroll | Colour |

Changing a slider with a shape selected restyles that shape. A whole slider drag is one undo step.

## Editing shapes

With Select active, click a shape to select it. Drag it to move, drag a handle to resize, use the arrow keys to nudge, <kbd>Delete</kbd> removes it. <kbd>Ctrl</kbd>+click adds a shape to the selection or takes it out again, and <kbd>Ctrl</kbd>+<kbd>A</kbd> selects everything. Moving, nudging, turning, recolouring, resizing and deleting then apply to the whole selection and undo as one step; the handles only show for a single shape. Right-click the canvas for a menu with every tool, the line sizes, undo, redo and delete.

<kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes and <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> or <kbd>Ctrl</kbd>+<kbd>Y</kbd> redoes. Every change is one step, including text edits and slider drags.

## Finishing

| Key | Inline editor | Editor window |
| --- | --- | --- |
| <kbd>Enter</kbd> | Accept: copy and save | |
| <kbd>Esc</kbd> | Cancel, nothing is saved | Close |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy only | Copy |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | Save only | Save |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> | Save as | Save as |

Saving is non-destructive. An edit from the gallery writes a new file with an `-edited` suffix and the original stays. Save and Save as always write to the capture folder, also when saving is switched off.

## Status

The tools, the inline editor, the editor window, shape editing, the right-click menu and the rebindable keys are lab tested on sway. Undo and redo are unit tested. Everything is unverified on a real desktop. See the [support matrix](/reference/support).

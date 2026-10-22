---
title: Gallery
description: Browse, search and filter captures, keep unsaved ones, relink moved files, pin captures and reuse picked colours.
---

# Gallery

`kiekje gallery`, the tray or the capture menu opens the gallery. It lists every capture Kiekje made, newest first, from an SQLite index in `$XDG_DATA_HOME/kiekje`. Thumbnails are generated on a worker pool, cached in `$XDG_CACHE_HOME/kiekje/thumbs` and decoded only for the items on screen. Scrolling stays smooth with 10,000 captures in the lab.

## Views, search and filters

Switch between a grid of cards and a list with details. The search box matches the file name. Filters sit in a row of chips:

| Filter | Options |
| --- | --- |
| Kind | Region, full screen, window, recording |
| Period | Today, this week, this month, a date range |
| Dimensions | Width and height bounds |
| File size | Size bounds |
| Annotated | Captures with an edited copy |
| Unsaved | Captures kept in the cache |

Sorting covers date, name, size and dimensions. The filters are unit tested and unverified on a real desktop.

## File actions

Right-click a capture, or use the buttons on a selected card:

| Action | What it does |
| --- | --- |
| Open | Opens the file with the default app |
| Show in folder | Reveals it in the file manager |
| Copy image, Copy path | Puts the PNG or the path on the clipboard |
| Edit | Opens the [editor](/guide/annotate); saves a new `-edited` file |
| Pin | Keeps the capture floating above other windows |
| Rename | <kbd>F2</kbd> renames the file in place |
| Upload and copy link | <kbd>Ctrl</kbd>+<kbd>U</kbd>, only with [temporary upload](/guide/record-and-share#temporary-upload-links) turned on |
| Remove from gallery | Drops the record, keeps the file |
| Move to trash | Trashes the file through the desktop's trash and drops the record |

Permanent deletion is never the default.

## Unsaved captures

With `[saving] enabled = false`, or **Save captures to the folder** switched off, captures are copied but not written to the capture folder. Each one still lands in the gallery with an Unsaved badge, kept in `$XDG_CACHE_HOME/kiekje/unsaved`. **Keep** moves it to the capture folder. Anything older than `keep_unsaved_days` (7 by default) is pruned at the first start after that.

The editor's Save and Save as always write to the capture folder, whatever this setting says.

## Missing files

On startup and when the gallery opens, every record is checked against disk. A file that moved or disappeared gets a Missing badge. A moved file that reappears inside the capture folders is relinked by size and hash. You can also relink one by hand or remove the record.

## Pinned captures

Pin from the overlay, the gallery or the command line:

```bash
kiekje pin ~/Pictures/Kiekje/kiekje-region-2026-10-16-14-02-11.png
```

A pinned capture floats above other windows. Drag it, scroll to zoom, <kbd>Esc</kbd> closes it. On KWin, Hyprland and Sway it is a layer-shell surface and stays on top. On GNOME the window manager decides, and `doctor` says `pins: on top not guaranteed`.

## Colours

Colours copied from the loupe with <kbd>C</kbd> are listed under Colours in the gallery, newest first, each with the hex value and a copy action. Clear colours in the danger zone empties the list.

## Danger zone

The About page in Settings holds the destructive actions behind a two-click guard: reset settings, clear gallery, clear colours and clear thumbnails. Clearing the gallery drops the index only; files stay where they are.

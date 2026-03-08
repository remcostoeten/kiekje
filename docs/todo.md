# Screeny TODO

This is the actionable backlog derived from the release spec.

Status legend:
- `[x]` done
- `[ ]` not started
- `[~]` partial

## Ship blockers

- `[x]` Dim region-selection overlay and improve selection contrast.
- `[x]` Add keyboard shortcuts for primary editor actions.
- `[x]` Make large screenshots navigable in the editor.
- `[x]` Add `Save As`.
- `[x]` Let users choose and persist a default save folder.
- `[x]` Save actual drawn annotations into the exported PNG.
- `[x]` Save actual text annotations into the exported PNG.
- `[x]` Add color selection for annotations.
- `[x]` Add adjustable annotation size.
- `[~]` Surface save failures in the UI.
  Current state: editor save failures update the status label.
  Missing: tray and non-editor flows should show visible errors too.
- `[~]` Make the editor keyboard-accessible end to end.
  Current state: tool shortcuts, save, undo, clear, close, visible color presets, and keyboard-focusable size controls exist.
  Missing: focus-order polish, shortcut coverage for secondary controls, and a proper screen-reader pass.
- `[x]` Add redo support.
- `[x]` Add unsaved-changes warning on close.
- `[x]` Add selection and resize handles for existing annotations.
- `[x]` Copy the annotated result back to clipboard on demand or after save.
- `[x]` Add capture delay presets to the GUI/tray.
- `[~]` Make tray and CLI expose the same settings consistently.
  Current state: tray and launcher now expose capture modes, delay presets, default mode, and the main clipboard/editor/auto-save toggles.
  Missing: folder selection and secondary export toggles still live only in the editor shell.
- `[~]` Make cancellation behavior explicit and user-visible for region/window capture.
  Current state: region selection shows cancel/fullscreen affordances and returns a user-visible cancel error.
  Missing: the window/CLI/TUI flows still need clearer cancellation messaging and parity.
- `[ ]` Validate screen-reader behavior for toolbar, status text, and dialogs.

## Nice to have

- `[ ]` Blur / pixelate tool for sensitive information.
- `[ ]` Crop after capture.
- `[ ]` Numbered callouts / step markers.
- `[ ]` Stroke presets or a compact size slider in addition to scroll.
- `[ ]` Filename presets per capture mode.
- `[ ]` Pin favorite save folders.
- `[ ]` Recent captures list with reopen action.
- `[ ]` Desktop notifications for save/copy completion.
- `[ ]` Optional cursor include/exclude toggle when backend supports it.
- `[ ]` Better compositor support beyond Hyprland-specific window capture.
- `[~]` Shortcut setup for Hyprland, Sway, and GNOME.
  Current state: launcher can record per-mode shortcuts and generate/install a Hyprland include file.
  Missing: Sway and GNOME workflows are still undocumented and unimplemented.

## Post-release backlog

- `[ ]` Refactor capture backends behind a compositor/platform abstraction.
- `[ ]` Broaden Linux desktop integration beyond the current Wayland-first path.
- `[ ]` Add stronger automated coverage around export rendering and editor interactions.
- `[ ]` Revisit full accessibility audit after interaction model stabilizes.
- `[ ]` Investigate cross-platform capture backends only after Linux flow is solid.
- `[ ]` Consider optional upload/share integrations only after core capture/edit/save is stable.

## Suggested next order

1. Improve tray parity and cancellation UX.
2. Add stronger UI feedback for non-editor save and dependency failures.
3. Revisit screen-reader and focus behavior after the editor layout stabilizes.
4. Add blur / pixelate and crop.
5. Add automated coverage around editor interactions and export rendering.

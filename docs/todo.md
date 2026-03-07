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
  Current state: tool shortcuts, save, undo, clear, and close exist.
  Missing: a fully keyboard-driven color/size workflow and better focus handling.
- `[ ]` Add redo support.
- `[ ]` Add unsaved-changes warning on close.
- `[ ]` Add selection and resize handles for existing annotations.
- `[ ]` Copy the annotated result back to clipboard on demand or after save.
- `[ ]` Add capture delay presets to the GUI/tray.
- `[ ]` Make tray and CLI expose the same settings consistently.
- `[ ]` Make cancellation behavior explicit and user-visible for region/window capture.
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
- `[ ]` Shortcut setup examples for Hyprland, Sway, and GNOME.

## Post-release backlog

- `[ ]` Refactor capture backends behind a compositor/platform abstraction.
- `[ ]` Broaden Linux desktop integration beyond the current Wayland-first path.
- `[ ]` Add stronger automated coverage around export rendering and editor interactions.
- `[ ]` Revisit full accessibility audit after interaction model stabilizes.
- `[ ]` Investigate cross-platform capture backends only after Linux flow is solid.
- `[ ]` Consider optional upload/share integrations only after core capture/edit/save is stable.

## Suggested next order

1. Add redo and unsaved-changes detection.
2. Add annotation selection and resizing.
3. Add clipboard export of the annotated image.
4. Add blur / pixelate and crop.
5. Improve tray parity and cancellation UX.

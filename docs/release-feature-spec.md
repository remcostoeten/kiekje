# Kiekje release feature spec

See also: [docs/todo.md](/home/remcostoeten/projects/screeny/docs/todo.md) for the actionable backlog with current status.

## Goal

Make Kiekje good enough to ship as a daily-use Linux screenshot tool, not just a proof of concept for Hyprland power users.

## Release bar

- Fast capture flow with obvious success and failure states.
- Accessible editor that works with keyboard-first users and remains usable on large screenshots.
- Reliable save and clipboard behavior with recoverable errors.
- A small but polished feature set that covers the main screenshot jobs people actually repeat.

## Current status

- The editor already supports redo, unsaved-change warnings, Save As, annotation selection/resizing, and explicit annotated-image copy.
- Tray and launcher entry surfaces now exist for capture actions, delay presets, doctor output, and the main shared toggles.
- The remaining release risk is concentrated in deeper tray parity, keyboard-only access for secondary controls, clearer cancellation/error paths, and general UI polish.

## Must-have before release

### 1. Capture experience

- Dimmed region-selection overlay with a high-contrast border and selection fill.
- Optional capture delay presets: `0s`, `3s`, `5s`, `10s`.
- Clear cancellation behavior for region/window capture.
- Tray and CLI actions should expose the same capture modes and settings.

### 2. Annotation editor

- Keyboard shortcuts for every primary action.
- Scrollable canvas for oversized screenshots.
- Tool state that is always visible and never ambiguous.
- Editable text annotations instead of the current placeholder text.
- Resize handles and selection for existing annotations.
- Redo support in addition to undo.
- Copy annotated result back to clipboard after saving or via explicit action.

### 3. Accessibility

- Every toolbar control has an explicit label, tooltip, and shortcut.
- Visible help text for shortcuts and current mode.
- High-contrast default colors for selection and annotations.
- Keyboard-only workflow for open, annotate, save, and close.
- Screen reader pass over toolbar labels, status text, and error dialogs.
- Configurable annotation sizes for users who need larger targets.

### 4. Reliability

- Save failures surfaced in the UI, not only stderr.
- Unsaved-changes warning on close.
- Save As flow in addition to default-path save.
- Dependency doctor and recovery hints wired into the tray/editor too, not only CLI/TUI.
- Basic automated coverage for settings, save-path rendering, and capture command construction.

## Should-have for a worthwhile 1.0

### Workflow polish

- Recent captures list with quick reopen.
- Pin/favorite save destinations.
- Optional filename presets per capture mode.
- Toggle to include or exclude cursor when backend supports it.

### Annotation depth

- Blur / pixelate tool for sensitive information.
- Numbered steps / callouts.
- Color and stroke-size picker.
- Crop after capture.

### Platform fit

- Global shortcut examples for Hyprland, Sway, and GNOME.
- Better window capture abstraction so non-Hyprland compositors degrade gracefully.
- Desktop notifications on save or copy.

## Not for the first public release

- Cloud upload or account features.
- Team collaboration.
- Cross-platform backends before Linux is stable.
- Template-heavy image composition.

## Suggested implementation order

1. Finish editor accessibility and keyboard flow.
2. Replace placeholder text annotations with inline text editing.
3. Unify tray/TUI/CLI parity and dependency recovery messaging.
4. Add blur/pixelate and crop.
5. Expand compositor support and desktop integration.

## Success metrics

- First capture to saved file in under 5 seconds for a new user following README instructions.
- Keyboard-only users can complete a full annotate-and-save flow.
- Failed dependencies or save paths always produce a visible recovery path.

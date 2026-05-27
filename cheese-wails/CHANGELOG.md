# Changelog

All notable changes to Kiekje are documented here.

Commit messages should follow [Conventional Commits](https://www.conventionalcommits.org) for semantic versioning:

- `feat:` minor release
- `fix:` patch release
- `feat!:` or `BREAKING CHANGE:` major release
## [0.1.0] - 2026-05-27

### Changes

- Add Wails capture editor app

- Add Hyprland floating rules for Cheese

- Center and size Cheese Wails window

- Start Cheese hidden until capture completes

- Add shortcut recorder and post-capture UI

- Wire saved binds into action dispatcher

- Separate capture and recapture flows

- Add region capture overlay, install script, and tray sidecar integration.

Ship window-pick preview, managed Hyprland bindings, second-instance IPC, and the current screenshot editor UI before frontend modularization.

Co-authored-by: Cursor <cursoragent@cursor.com>

- Modularize frontend into ES modules with central app state.

Split the monolithic main.js into capture, editor, settings, input, and UI modules, unify keyboard handling, and deduplicate the color picker logic.

Co-authored-by: Cursor <cursoragent@cursor.com>

- Use slurp for capture with Hyprland window pick support.

Cheese hides before slurp runs on the live desktop, avoiding the blurred Wails overlay while still allowing region drag or window click via piped client geometry.

Co-authored-by: Cursor <cursoragent@cursor.com>

- Add undo/redo, blur redaction, and stroke width to the editor.

Annotations and baked image edits share a history stack so blur redacts persist on export, with toolbar controls and keyboard shortcuts for the new tools.

Co-authored-by: Cursor <cursoragent@cursor.com>

- Add resize handles, highlight, ellipse, steps, and crop to the editor.

Select tool corner handles resize supported shapes, new annotation tools extend post-capture markup, and crop trims the baked image layer with annotation offsets preserved in undo history.

Co-authored-by: Cursor <cursoragent@cursor.com>


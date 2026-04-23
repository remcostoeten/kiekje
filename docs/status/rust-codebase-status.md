# Rust codebase status

This page summarizes the current Rust application in `src/`. It focuses on
what the code offers today, what is working, what is only partial, and how the
main modules fit together.

## Architecture

The codebase is small enough to follow, but it already has clear domain splits.

- `src/main.rs`: CLI entry, mode dispatch, interactive menu, error rendering,
  and dependency recovery prompts.
- `src/capture/`: capture mode orchestration for region, fullscreen, and
  active-window capture.
- `src/platform/linux/`: Linux-specific integrations for `grim`, `hyprctl`,
  shortcut file generation, and tray autostart.
- `src/app/`: GTK and libadwaita surfaces, including launcher, tray-backed
  windows, editor shell, and region selector overlay.
- `src/editor/`: annotation canvas state and tool definitions.
- `src/settings/`: persisted config loading, defaults, migration from legacy
  config path, and save behavior.
- `src/storage/`: save-path rendering and file output.
- `src/clipboard/`: Wayland clipboard integration through `wl-copy`.
- `src/diagnostics/`: doctor report, dependency checks, portal inspection, and
  limited auto-repair helpers.

## Working features

The following features are implemented in code and exposed through at least one
user surface.

- Region capture by taking a fullscreen screenshot and cropping it after a GTK
  area selection.
- Fullscreen capture through `grim`.
- Active-window capture through `hyprctl activewindow -j` plus `grim -g`.
- Annotation editor with rectangle, arrow, freehand, text, and highlight
  tools.
- Annotation selection, resizing, delete, undo, redo, clear, and copy.
- Save, Save As, default save folder, filename templates, and optional
  `xdg-open` after save.
- Shared settings in `~/.config/kiekje/config.json`.
- GTK launcher with capture buttons, delay presets, default mode controls,
  doctor output, portal repair, tray autostart, and Hyprland shortcut setup.
- StatusNotifier tray service with capture actions, toggles, delay radio
  options, default-mode radio options, launcher access, and doctor output.
- CLI modes `region`, `fullscreen`, `window`, `--interactive`, `--launcher`,
  `--tray`, and `--doctor`.

## Partial features

These areas exist, but the repo itself marks them as incomplete or uneven.

- Tray and launcher parity is partial. Folder selection and secondary export
  toggles still live mainly in the editor shell.
- Save and error feedback is partial outside the editor. The editor updates a
  status label, but non-editor flows still need stronger visible feedback.
- Keyboard accessibility is improved, but not complete end to end.
- Cancellation handling exists, especially for region selection, but user
  messaging is not yet uniform across CLI, tray, launcher, and TUI flows.
- Portal diagnostics and repair exist, but the app still depends on backend
  tools like `grim` and `hyprctl` for capture.

## Missing or not yet solved

These features are not finished in current code.

- A compositor abstraction that is not tightly tied to `grim` and Hyprland.
- Broader Linux desktop support beyond the current Wayland-first path.
- Strong automated coverage for GTK surfaces and annotation workflows.
- Screen-reader validation across toolbar labels, status text, and dialogs.
- Advanced editor features such as blur, crop, numbered callouts, and richer
  save presets.

## Health assessment

The codebase is functional and already shippable for a narrow audience. The
main problem is architecture coupling, not feature absence. GTK views own too
much workflow logic, Linux backend assumptions are embedded in the app layer,
and there is no clean application service boundary for future Tauri reuse.

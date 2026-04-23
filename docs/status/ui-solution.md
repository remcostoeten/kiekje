# Current UI solution

This page describes how the product presents itself today. Kiekje does not have
one single UI shell. It has several overlapping surfaces built with different
stacks.

## Current surfaces

The current user-facing and contributor-facing surfaces are listed below.

- CLI in `src/main.rs` for direct command execution.
- Interactive terminal menu in Rust for lightweight recoverable flows.
- Bubble Tea TUI in `cmd/kiekje-tui` for contributor workflow and some shared
  settings.
- GTK launcher in `src/app/tray.rs`.
- StatusNotifier tray service in `src/app/tray.rs`.
- GTK annotation editor in `src/app/window.rs` and `src/editor/`.
- Fullscreen GTK region selector overlay in `src/app/region_selector.rs`.

## UI stack

The implementation is split across multiple toolkits and runtime models.

- Rust plus GTK4 plus libadwaita power the launcher, editor, and region
  selector.
- Rust plus `ksni` power the tray service.
- Go plus Bubble Tea plus Lip Gloss power the TUI helper.
- Shared settings live in JSON and are read by both Rust and Go binaries.

## What works well

The current solution already proves several product ideas.

- Capture and annotation are integrated into a single local workflow.
- The editor exposes a serious set of controls instead of a throwaway preview.
- The launcher and tray both surface useful capture actions and doctor output.
- The region selector is simple and direct.
- Shared config keeps the different surfaces mostly aligned on defaults.

## What is weak

The UI story is fragmented and expensive to keep coherent.

- There is no single design system or shared component model.
- State and behavior are duplicated across CLI, TUI, launcher, tray, and
  editor.
- The GTK launcher and tray own workflow logic that should live in reusable
  application services.
- The Go TUI duplicates product concerns that may disappear once a stronger GUI
  shell exists.
- Accessibility and cancellation behavior are uneven across surfaces.
- A future Tauri shell cannot yet plug into a clean Rust backend API.

## Recommendation

Treat the current GTK editor and launcher as reference behavior, not as final
architecture. The next step is not a visual rewrite first. The next step is to
separate backend services from presentation so the current GTK UI and a future
Tauri UI can consume the same capture, settings, diagnostics, and export logic.

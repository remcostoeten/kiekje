# Project status hub

This folder is working map for Kiekje as of April 23, 2026. It keeps current
product status, architecture notes, Linux support boundaries, and overhaul
planning in one place so contributors and agents can pick work without first
reverse-engineering the repository.

## Summary

Kiekje is already more than a screenshot proof of concept. The Rust core can
capture, annotate, save, copy, launch from GTK surfaces, and expose a tray.
The current weakness is not raw feature count. The weakness is product shape:
multiple UI surfaces overlap, Hyprland assumptions leak through the stack, and
the codebase is not yet split into a clean backend that a future Tauri shell
can reuse.

## Documents

Use these pages as entry points for specific questions.

- [TUI status](./tui-status.md)
- [Rust codebase status](./rust-codebase-status.md)
- [Runtime support and flags](./runtime-support.md)
- [Current UI solution](./ui-solution.md)
- [Current work tracker](./current-status.md)
- [Pre-Tauri scope](./overhaul/pre-tauri-scope.md)
- [Tauri implementation plan](./overhaul/tauri-implementation.md)
- [Current Linux support](./future/current-supported-linux.md)
- [Ubuntu support path](./future/ubuntu.md)
- [Debian support path](./future/debian.md)
- [Fedora support path](./future/fedora.md)

## How to use this folder

Start with the Rust and runtime pages if you need factual context. Move to the
UI and Tauri pages if you are reshaping the product. Use the current status and
pre-Tauri scope pages as the operational backlog for follow-up work.

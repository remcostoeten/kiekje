# TUI status

This page tracks the Go Bubble Tea helper in `cmd/kiekje-tui`. That binary is
not the product's primary user interface. It is a contributor-facing helper
that also exposes a few end-user settings and capture actions.

## What it is

The TUI wraps the Rust binary and repository tooling in a terminal UI. It can
run captures, edit config values, launch Cargo and Go checks, inspect git
status, create git tags, and create GitHub releases through `gh`.

## Current status

The TUI works, but it sits in an awkward middle state.

- It is useful for contributors who live in terminal-first workflows.
- It is not a good long-term primary UX for the product itself.
- It mixes end-user actions with maintainer-only release automation.
- It still carries legacy `screeny` and `capture-app` compatibility paths.
- It depends on repository-local files such as `.kiekje-tui-features.json`.

## What it is useful for

The current implementation still has clear short-term value.

- Running capture flows without remembering CLI flags.
- Toggling shared settings in `~/.config/kiekje/config.json`.
- Running `cargo build`, `cargo test`, `cargo clippy`, `go test`, and
  `go vet`.
- Running `git status`, `git push`, release packaging, and `gh release create`.
- Bootstrapping local development when `APP_BIN` is not set.

## Where it adds friction

The current design creates overlap and maintenance cost.

- User workflow and maintainer workflow live in one binary.
- Release automation inside the TUI duplicates work better handled by CI or a
  dedicated maintainer script.
- The helper shells out to `git`, `gh`, `cargo`, and `go`, so failures surface
  late and in tool-specific ways.
- The file still references historical names such as `capture-app`,
  `.screeny-tui-features.json`, and `~/.config/screeny/config.json`.
- The TUI has no visible test coverage of its own behavior.

## Recommendation

Keep the TUI for contributor convenience in the short term, but stop treating
it as a core product surface. Before any Tauri migration, decide one of these
two paths and document it explicitly.

- Keep it as a maintainer tool only. Remove end-user framing and keep dev,
  release, and diagnostics tasks.
- Deprecate it after the GTK launcher and future Tauri shell fully cover
  day-to-day user tasks.

## Needed changes

The file does not need a redesign before every other task, but it does need
cleanup before the project can present a coherent surface.

- Remove stale `screeny` and `capture-app` naming once migration support is no
  longer required.
- Split user settings and capture actions from dev and release actions.
- Decide whether GitHub release creation belongs here at all.
- Add at least smoke coverage around config loading, app binary resolution, and
  feature-list handling.
- Align terminology with the rest of the repo so the binary reads as
  `kiekje-tui`, not a carry-over maintenance artifact.

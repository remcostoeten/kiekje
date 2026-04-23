# Pre-Tauri scope

This page defines work that must happen before introducing a Tauri shell. The
goal is not to rewrite everything first. The goal is to cut backend seams so a
new frontend can reuse stable logic instead of reimplementing GTK behavior.

## Exit criteria

Use these conditions to decide whether the repo is ready for a Tauri branch.

- [ ] Core capture, settings, diagnostics, and export flows are callable
  through reusable Rust services instead of GTK event handlers.
- [ ] User-visible errors and cancellations follow one consistent model across
  all surfaces.
- [ ] Linux backend assumptions are isolated behind explicit interfaces.
- [ ] Current GTK surfaces can consume the same backend APIs that Tauri will
  consume later.
- [ ] Legacy naming drift does not leak into new architecture work.

## Cross-cutting features

These tasks cut across files and should be treated as foundation work.

- [ ] Define `capture service` API for region, fullscreen, and window flows.
- [ ] Define `settings service` API for load, save, defaults, and migration.
- [ ] Define `diagnostics service` API for doctor output, dependency checks,
  portal state, and repair attempts.
- [ ] Define `export service` API for save, copy, open-after-save, and path
  templating.
- [ ] Define shared app error model with stable codes and user-facing messages.
- [ ] Decide support policy for GTK launcher, tray, and TUI during migration.

## File-by-file scope

This checklist is organized so agents can pick up bounded slices.

### `src/main.rs`

This file currently owns too much orchestration.

- [ ] Extract CLI dispatch into reusable application services.
- [ ] Replace ad hoc recovery loops with typed command results.
- [ ] Keep CLI parsing here, but move workflow logic out.
- [ ] Decide whether hidden `--startup-delay-ms` remains a CLI concern.

### `src/app/window.rs`

This file is current heaviest UI shell and carries too much product logic.

- [ ] Move save, copy, open-after-save, and recapture workflows into backend
  services.
- [ ] Reduce direct process spawning from widget callbacks.
- [ ] Separate editor view state from export side effects.
- [ ] Document which behavior must survive unchanged in Tauri.

### `src/app/tray.rs`

This file mixes tray menu behavior, launcher UI, process spawning, and settings
mutation.

- [ ] Extract shared launcher actions into non-GTK service layer.
- [ ] Normalize feedback handling for tray and launcher failures.
- [ ] Isolate autostart and Hyprland shortcut management.
- [ ] Remove logic duplication with `src/main.rs`.

### `src/app/region_selector.rs`

This file is simple, but it still deserves a clean seam.

- [ ] Keep selector as view-only surface that returns a typed selection result.
- [ ] Clarify cancellation and fullscreen fallback semantics.
- [ ] Decide whether future Tauri keeps this GTK overlay temporarily or
  replaces it immediately.

### `src/editor/canvas.rs`

This file contains valuable editor domain logic that should survive migration.

- [ ] Split annotation model and export rendering from GTK widget concerns.
- [ ] Introduce serializable editor state where practical.
- [ ] Add stronger tests around annotation transforms and export output.
- [ ] Identify canvas behavior that depends on GTK-specific gesture handling.

### `src/settings/config.rs`

This file is already useful foundation code, but it needs product decisions.

- [ ] Freeze config schema that both GTK and Tauri paths can share.
- [ ] Decide when legacy `screeny` reads stop being supported.
- [ ] Add migration notes for any future schema changes.

### `src/storage/save.rs`

This file is small and should become part of stable backend surface.

- [ ] Keep filename-template rendering backend-owned.
- [ ] Add more tests around invalid paths and save failure behavior.
- [ ] Return structured save results for UI consumption.

### `src/clipboard/mod.rs`

This file is narrow, but still part of export contract.

- [ ] Define clipboard failure shape in shared error model.
- [ ] Decide whether clipboard stays optional capability in future shells.

### `src/diagnostics/mod.rs`

This file is already rich, but it still returns mostly presentation-ready text.

- [ ] Split raw diagnostics data from rendered report text.
- [ ] Return machine-friendly structs that UI can render.
- [ ] Isolate portal repair side effects from report generation.

### `src/platform/linux/grim.rs`

This file is capture backend start point.

- [ ] Hide `grim` process details behind a Linux capture backend trait.
- [ ] Clarify backend capability detection for fullscreen versus region.

### `src/platform/linux/hyprctl.rs`

This file is current window-capture lock-in point.

- [ ] Isolate Hyprland-only logic behind optional backend interface.
- [ ] Prepare room for non-Hyprland window backends or explicit unsupported
  responses.

### `src/platform/linux/integration.rs`

This file mixes Linux desktop integration concerns.

- [ ] Split autostart, shortcut-file generation, and other desktop integration
  helpers into smaller modules.
- [ ] Keep file-writing logic reusable from future shells.

### `cmd/kiekje-tui/main.go`

This file is migration policy question as much as cleanup task.

- [ ] Decide keep-versus-deprecate status.
- [ ] Remove stale `capture-app` and `screeny` references if compatibility can
  end.
- [ ] Strip release automation if it no longer belongs in product repo.

### `README.md` and `docs/`

Documentation needs to match the support boundary before new UI work starts.

- [ ] Align README claims with actual compositor and distro support.
- [ ] Link this planning set from the main docs flow when ready.
- [ ] Keep support matrix explicit instead of implied.

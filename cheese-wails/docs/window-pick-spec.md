# Window Pick Spec

This spec defines the implementation target for full-window capture when the user holds `Ctrl` and clicks inside the capture overlay.

## Goal

When the capture overlay is active, holding `Ctrl` and clicking should capture the full window under the pointer if the compositor can provide that geometry. If the compositor cannot provide it, the app should fail gracefully and keep the existing manual region capture path.

## Scope

Included:

- `Ctrl + click` window capture from the overlay
- Hyprland support
- Sway/i3 support
- Plasma/KWin support if a compositor-side bridge is available
- GNOME support only if a shell-side bridge is available
- clean fallback to manual region capture

Excluded:

- general screenshot capture outside the app
- window enumeration UI
- OCR or post-processing

## Behavior

1. User enters capture mode.
2. User drags a region as usual, or holds `Ctrl` and clicks a window area while the overlay is active.
3. The app resolves a `WindowGeometry`.
4. The app runs the existing `CaptureRegionAt(x, y, w, h)` flow.
5. The rest of the copy/save/editor behavior stays unchanged.

## Resolver Rules

The resolver should try the available compositor integrations in order and return the first valid geometry:

1. Hyprland client geometry lookup
2. Sway/i3 tree lookup
3. Plasma/KWin lookup
4. GNOME Shell lookup

If a resolver is unavailable or fails, it should be skipped. The app should only fail when no resolver can produce a geometry.

## Platform Notes

### Hyprland

- external process integration is available
- client geometry can be resolved from compositor data
- this is the current working path

### Sway / i3

- external process integration is available
- tree-based window lookup is acceptable
- use the topmost matching window at the given point

### Plasma / KWin

- compositor-side scripting APIs expose cursor and window-at-point data
- this likely needs a KWin-specific helper or bridge
- do not assume an external app can query this directly

### GNOME / Mutter

- shell-side APIs exist, but they are compositor-internal
- this likely requires a GNOME Shell extension or shell helper
- if this work is too large for the current scope, keep the fallback path and document the limitation

## Implementation Notes

- Keep the UI unchanged except for the `Ctrl + click` window capture behavior.
- Keep the geometry resolver separate from the image capture logic.
- Normalize all backend results into a single `WindowGeometry` type.
- Avoid hard-failing when a compositor-specific resolver is missing.

## Acceptance Criteria

- `Ctrl + click` on a window captures the full window on supported desktops.
- Region drag capture still works.
- Unsupported compositors fall back cleanly instead of breaking capture mode.
- The resolver code stays isolated enough that another compositor can be added later without changing the UI flow.

## Suggested Task Breakdown

1. Stabilize the existing Hyprland and Sway/i3 path.
2. Add a KWin helper or bridge.
3. Add a GNOME bridge only if the effort is justified.
4. Refactor the resolver into a small strategy layer if the list grows.

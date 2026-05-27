# Plasma / KWin Window Pick Spec

## Goal

Resolve the full window geometry under the pointer in Plasma using KWin-side APIs or a small helper bridge.

## Constraint

This is not a pure external-process problem. KWin exposes the useful primitives inside compositor-side scripting APIs.

## Behavior

1. Receive the click point from the overlay.
2. Query KWin-side data for the window at that point.
3. Return the topmost matching window geometry.
4. Fall back cleanly when the KWin bridge is unavailable.

## Implementation Options

- a KWin script that exposes the lookup result
- a DBus bridge published by a KWin helper
- a small companion process if one is more practical for install/update flow

## Acceptance Criteria

- `Ctrl + click` on a window in Plasma captures the full window
- the app degrades to manual region capture if the KWin bridge is absent
- no Hyprland-only assumptions remain in this path

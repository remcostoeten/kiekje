# GNOME / Mutter Window Pick Spec

## Goal

Resolve the full window geometry under the pointer in GNOME using shell-side APIs or a GNOME Shell extension.

## Constraint

GNOME Shell exposes the relevant window data inside the shell process, not as a general-purpose external API.

## Behavior

1. Receive the click point from the overlay.
2. Ask a shell-side helper or extension for the topmost window at that point.
3. Return the geometry if available.
4. Fall back to region selection if not available.

## Implementation Options

- a GNOME Shell extension
- a shell-side DBus bridge
- a documented fallback-only implementation if first-class support is not worth the maintenance cost

## Acceptance Criteria

- if GNOME support is implemented, `Ctrl + click` on a window captures the full window
- if GNOME support is not implemented, the app still behaves correctly and predictably
- the user never gets a broken capture flow

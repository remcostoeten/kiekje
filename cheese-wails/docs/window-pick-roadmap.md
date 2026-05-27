# Window Pick Roadmap

This document tracks the work needed to support "hold `Ctrl` and click a window to capture the full window" across multiple desktop environments.

## Spec Handoff

The full implementation specs live in [capture-spec.md](./capture-spec.md), [window-pick-spec.md](./window-pick-spec.md), and the compositor-specific set under [distro-specs/](./distro-specs/). Use those files as the agent handoff for the next implementation pass.

## Current State

The capture overlay already supports:

- region capture
- `Ctrl + click` on the overlay to capture a full window
- Hyprland geometry lookup
- Sway/i3-style tree lookup as a fallback

That means the feature is already usable on some wlroots-based desktops, but it is not yet universal.

## Why This Is Not Fully Portable

Wayland does not expose a single standard external API for "window under cursor" that every compositor implements the same way.

In practice:

- Hyprland exposes client geometry through `hyprctl`
- Sway exposes the tree through `swaymsg`
- Plasma/KWin exposes the relevant data inside compositor-side scripting APIs
- GNOME Shell exposes similar data inside shell-side APIs and extensions

The app can query some desktops from the outside. Others need compositor-specific integration.

## Roadmap

### Phase 1: Keep the current portable external path

Goal:

- Keep the current point-based lookup for Hyprland and Sway/i3-compatible environments
- Treat this as the default external fallback path

Work:

- retain Hyprland lookup
- retain Sway/i3 tree lookup
- keep the click-position-based right-click flow in the overlay

Acceptance:

- `Ctrl + click` on a window captures the full window on supported wlroots desktops
- manual region capture still works everywhere

### Phase 2: Add Plasma/KWin support

Goal:

- support full-window capture on Plasma without relying on Hyprland-specific commands

Work:

- add a KWin-specific resolver path
- use KWin window lookup at a point from the compositor-side API
- decide whether the integration lives in a helper script, a DBus bridge, or a packaged companion

Notes:

- KWin scripting can see cursor position and window-at-point data, but it runs inside KWin
- this is feasible, but it is not a drop-in external command replacement

Acceptance:

- `Ctrl + click` on a window in Plasma resolves the topmost window under that point
- if the KWin path is unavailable, the app falls back to region selection cleanly

### Phase 3: Decide on GNOME support

Goal:

- determine whether GNOME gets native full-window right-click capture or a graceful fallback

Work:

- evaluate a GNOME Shell extension or shell-side helper
- decide whether the extension should only expose window geometry or also trigger capture directly
- define installation and update flow if GNOME support is added

Notes:

- GNOME Shell extension APIs are compositor-internal
- this likely requires more surface area than the Plasma path

Acceptance:

- the app either captures a full window on GNOME through a supported shell-side bridge
- or it clearly falls back to manual region capture without broken UX

### Phase 4: Unify the resolver interface

Goal:

- keep platform-specific code isolated behind one resolver contract

Work:

- normalize all window-pick results into one geometry type
- keep capture and save logic independent from compositor detection
- make the feature easy to extend for future desktops

Acceptance:

- capture logic does not care which compositor supplied the geometry
- adding a new compositor path does not require changing the UI flow

## Suggested Implementation Order

1. Stabilize the current external fallback path.
2. Add a Plasma/KWin resolver.
3. Decide whether GNOME gets a shell-side bridge or region-only fallback.
4. Refactor the resolver into a small compositor strategy layer if more desktops are added.

## Open Questions

- Should Plasma and GNOME use separate helper binaries or a single resolver interface?
- Should GNOME get first-class support or a documented fallback only?
- Should window picking live in the app process, or should each compositor get its own helper package?

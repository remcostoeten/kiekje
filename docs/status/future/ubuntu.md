# Ubuntu support path

This page describes what Ubuntu support would require. The answer is "yes,
possible," but not through the current backend alone.

## Current reality

Ubuntu branding does not guarantee compatibility. Ubuntu commonly means GNOME on
Wayland, and that is not the same environment the current code targets.

- Ubuntu with Hyprland or another compatible wlroots session could work much
  sooner.
- Ubuntu GNOME as default desktop is not a clean fit for the current
  `grim` plus `hyprctl` backend.

## What is needed

These items are required before Ubuntu can be treated as supported target.

- Add capture backend that does not depend on Hyprland.
- Add portal-based or compositor-specific fullscreen and region capture for
  GNOME-first systems.
- Add generic window-capture strategy or explicitly disable that mode when
  unsupported.
- Validate tray behavior under Ubuntu desktop variants.
- Package and test dependency installation on `apt`.
- Document exact support matrix for Ubuntu sessions, not only for Ubuntu as a
  distro name.

## Recommended support policy

Use a staged policy instead of claiming broad Ubuntu support too early.

- Stage 1: support Ubuntu only when user runs a compatible Hyprland or similar
  session.
- Stage 2: support Ubuntu GNOME once portal-backed capture and better desktop
  integration land.

## Bottom line

Ubuntu support is possible, but official Ubuntu support should wait until the
backend is no longer effectively tied to `grim` plus Hyprland assumptions.

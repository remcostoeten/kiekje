# Hyprland Window Pick Spec

## Goal

Resolve the full window geometry under the pointer while the capture overlay is active, using Hyprland-native process access.

## Current State

This path is already implemented in the app through `hyprctl` client geometry lookup.

## Inputs

- a point in compositor coordinates
- current Hyprland client list

## Behavior

1. Resolve the pointer point from the overlay click.
2. Query Hyprland for the current client list.
3. Filter visible, mapped clients that contain the point.
4. Ignore the capture app’s own window.
5. Return the matching client geometry.

## Acceptance Criteria

- `Ctrl + click` on a window in Hyprland captures the full window
- the app does not capture itself
- fallback to region capture still works when no match is found

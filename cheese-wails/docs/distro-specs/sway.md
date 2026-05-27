# Sway / i3 Window Pick Spec

## Goal

Resolve the full window geometry under the pointer using the i3-compatible tree model exposed by Sway.

## Inputs

- a point in compositor coordinates
- the `swaymsg -t get_tree` JSON tree

## Behavior

1. Resolve the pointer point from the overlay click.
2. Query the tree from `swaymsg`.
3. Traverse the tree and floating nodes.
4. Find the topmost window whose rect contains the point.
5. Return its geometry.

## Notes

- This is the cleanest external fallback path on wlroots-based desktops.
- The tree resolver should ignore non-window container nodes.

## Acceptance Criteria

- `Ctrl + click` on a window in Sway captures the full window
- i3-compatible layouts behave the same way
- region capture still works when no match is found

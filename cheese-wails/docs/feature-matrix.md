# Feature Matrix

This page tracks what is already implemented versus what is still planned, grouped by desktop/compositor path.

## Hyprland

| Feature | Status | Notes |
| --- | --- | --- |
| Region capture | Implemented | Drag selection in the overlay captures a custom rectangle. |
| Full-window `Ctrl + click` capture | Implemented | Holding `Ctrl` inside capture mode resolves the window under the pointer. |
| Success toast when editor is skipped | Implemented | Shows a small native toast on successful clipboard-only capture. |
| Copy after capture | Implemented | The capture flow can copy the image after capture. |
| Close after capture | Implemented | The app can save and hide the editor after capture. |
| Save folder selection | Implemented | Save directory can be chosen from the settings menu. |
| Editor flow | Implemented | Captures can still open into the annotation editor. |

## Sway / i3

| Feature | Status | Notes |
| --- | --- | --- |
| Region capture | Implemented | Uses the same capture flow as Hyprland. |
| Full-window `Ctrl + click` capture | Implemented | Tree lookup fallback resolves the topmost window at the clicked point. |
| Success toast when editor is skipped | Implemented | Uses the same native toast path as other supported desktops. |
| Copy after capture | Implemented | Shared capture flow handles it. |
| Close after capture | Implemented | Shared capture flow handles it. |
| Editor flow | Implemented | Shared editor path remains available. |

## Plasma / KWin

| Feature | Status | Notes |
| --- | --- | --- |
| Region capture | Implemented | Manual region capture is compositor-independent. |
| Full-window `Ctrl + click` capture | Planned | Needs a KWin-side helper or bridge. |
| Success toast when editor is skipped | Implemented | This part is compositor-independent. |
| Copy after capture | Implemented | Shared capture flow handles it. |
| Close after capture | Implemented | Shared capture flow handles it. |
| Editor flow | Implemented | Shared editor path remains available. |

## GNOME / Mutter

| Feature | Status | Notes |
| --- | --- | --- |
| Region capture | Implemented | Manual region capture is compositor-independent. |
| Full-window `Ctrl + click` capture | Planned | Likely needs a GNOME Shell extension or shell-side helper. |
| Success toast when editor is skipped | Implemented | This part is compositor-independent. |
| Copy after capture | Implemented | Shared capture flow handles it. |
| Close after capture | Implemented | Shared capture flow handles it. |
| Editor flow | Implemented | Shared editor path remains available. |

## Summary

- The capture overlay and save/copy/editor flow are already in place.
- Hyprland and Sway/i3 have working full-window right-click capture paths.
- Plasma and GNOME still need compositor-specific resolver work for full-window picking.

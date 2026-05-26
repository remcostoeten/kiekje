# Cheese

Fast cross-distro Linux screenshot and annotation tool.

## Current scope
- region capture
- annotation tools
- clipboard copy
- file export
- Wayland and X11 compatibility

## Repo layout
- `plans/` design paper and implementation checklist
- `crates/core/` image and annotation logic
- `crates/app/` desktop shell and UI

## Quick start
- Run the editor locally: `./scripts/cheese.sh`
- Or from Cargo: `cargo run -- --edit --annotate --copy --preview`

## Hyprland example
Add a keybind like:

```ini
bind = SUPER, S, exec, /home/remcostoeten/dev/cheese/scripts/cheese.sh
```

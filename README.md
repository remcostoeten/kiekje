# Cheese Wails

Single-window capture and annotation tool for Hyprland and other Wayland desktops.

## Run

```bash
wails dev
```

## Build

```bash
wails build
```

## Hyprland floating rule

Add this to your Hyprland config so the window always opens floating:

```ini
windowrulev2 = float, title:^(Cheese)$
windowrulev2 = pin, title:^(Cheese)$
windowrulev2 = center, title:^(Cheese)$
```

If you want the app to stay above tiled windows, keep `AlwaysOnTop` enabled in `main.go`.

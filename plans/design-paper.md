# Cheese Screenshot Tool Design Paper

## Goal
Build a fast Linux screenshot tool that can:
- capture a region or full screen
- annotate with arrows, rectangles, text, pen, and highlights
- copy the result to clipboard
- save the result to file
- work across distros, Wayland, and X11 where possible

## Product Constraints
- Must feel instant from hotkey to editor
- Must avoid heavy browser-style overhead
- Must support cross-distro Linux use
- Must keep annotation latency low
- Must keep capture and editing separate

## Non-Goals For MVP
- OCR
- video or GIF capture
- cloud sync
- collaboration
- advanced image filters

## Architecture

### Core
The core should be a Rust library responsible for:
- image state
- annotation state
- undo and redo
- flattening output
- clipboard payload generation

### UI
The UI should be a thin shell responsible for:
- hotkey entry
- capture overlay
- region selection
- toolbar and canvas
- shortcut handling

### Capture Backends
Capture should be pluggable:
- Wayland portal path first
- X11 direct capture fallback
- optional compositor-specific fast paths later

## Data Model

### ScreenshotImage
- bitmap buffer
- width
- height
- format

### Annotation
- kind: arrow, rect, text, pen, highlight
- geometry
- style
- layer order

### EditorState
- active tool
- selected annotation
- undo stack
- redo stack
- current image
- annotation list

## Rendering Strategy
- Keep the base screenshot as a bitmap in memory
- Render annotations as vector overlays
- Flatten only on copy or save
- Avoid re-encoding on every stroke

## Capture Flow
1. Hotkey opens overlay
2. User selects region
3. Backend captures bitmap
4. Editor opens with bitmap in memory
5. User annotates
6. Final image is flattened
7. Result is copied or saved

## Performance Strategy
- Keep the app resident for fast launch
- Avoid expensive image conversions during drawing
- Delay PNG encoding until final output
- Keep the UI simple and direct

## Compatibility Strategy
- Detect Wayland or X11 at runtime
- Prefer portal capture where available
- Provide fallback capture paths
- Keep clipboard and hotkeys abstracted from the UI

## Distribution Strategy
- AppImage first for broad distro compatibility
- Flatpak second if portal integration is important
- Native distro packages later if needed

## MVP Definition
The MVP is complete when:
- a hotkey opens the overlay
- region selection works
- at least arrow, rectangle, and text tools work
- copy to clipboard works
- saving to file works
- it runs on both Wayland and X11 targets or degrades gracefully


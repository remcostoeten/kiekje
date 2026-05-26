# Cheese Screenshot Tool Checklist

## Phase 1: Project Setup
- [ ] Create repository layout
- [ ] Choose runtime shell
- [ ] Create Rust core crate
- [ ] Define shared image model
- [ ] Define annotation model
- [ ] Define capture backend trait

## Phase 2: Capture MVP
- [ ] Detect Wayland session
- [ ] Detect X11 session
- [ ] Implement one capture backend end to end
- [ ] Add transparent overlay window
- [ ] Add region selection by drag
- [ ] Return bitmap to editor

## Phase 3: Annotation MVP
- [ ] Add rectangle tool
- [ ] Add arrow tool
- [ ] Add text tool
- [ ] Add freehand pen tool
- [ ] Add highlight tool
- [ ] Add undo and redo
- [ ] Add delete and move interactions

## Phase 4: Output
- [ ] Flatten screenshot and annotations to bitmap
- [ ] Copy image to clipboard
- [ ] Save PNG file
- [ ] Optionally save WebP
- [ ] Verify paste into browser and chat apps

## Phase 5: Usability
- [ ] Add global hotkey
- [ ] Add keyboard shortcuts
- [ ] Add clear cursor feedback
- [ ] Add toolbar affordances
- [ ] Reduce startup latency
- [ ] Add error handling for unsupported capture paths

## Phase 6: Compatibility
- [ ] Test on Wayland
- [ ] Test on X11
- [ ] Test on multiple distros
- [ ] Test on NVIDIA systems
- [ ] Test on Intel systems
- [ ] Verify clipboard behavior across desktop environments

## Phase 7: Packaging
- [ ] Build AppImage
- [ ] Optionally build Flatpak
- [ ] Write install instructions
- [ ] Write troubleshooting notes

## MVP Acceptance
- [ ] Hotkey opens the tool
- [ ] Region capture works
- [ ] Arrow, rectangle, and text annotations work
- [ ] Clipboard copy works
- [ ] File save works
- [ ] The app is usable on the current Arch and Hyprland setup


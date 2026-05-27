# Capture Flow Spec

This spec defines the capture flow itself, independent from how the app decides which rectangle to capture.

## Goal

Make the capture experience predictable across desktops:

- enter capture mode
- let the user drag a region or pick a full window
- capture the chosen geometry
- keep the save/copy/editor behavior consistent

## Scope

Included:

- capture overlay
- region selection
- full-window selection via `Ctrl + click`
- copy/save behavior after capture
- success toast when the editor is skipped
- editor visibility rules

Excluded:

- compositor-specific window lookup logic
- OCR or annotation recognition
- any post-processing beyond the existing editor

## Behavior

1. User starts capture.
2. The overlay appears and blocks interaction with the underlying editor.
3. User either drags a region or holds `Ctrl` and clicks to pick a window geometry.
4. The app captures that geometry into an image.
5. The app either opens the editor or performs the configured post-capture action.

## Post-Capture Rules

- If `Clipboard only` is enabled, copy the image to the clipboard and show a small success toast.
- If `Close after capture` is enabled, save the image and hide the window.
- If neither of those settings is enabled, load the image into the editor.
- If `Copy after capture` is enabled, copy the captured image in addition to the main flow.

## Overlay Rules

- The capture overlay should be visually separate from the editor.
- Selection feedback should remain clear and readable on all desktops.
- Escape should cancel capture selection.
- Holding `Ctrl` should arm window-pick mode only while capture mode is active.

## Success Feedback

- When the editor is skipped and capture succeeds, show a small native success toast.
- The toast should not block the main flow.
- The toast should not require the editor window to be visible.

## Acceptance Criteria

- Region capture still works.
- Window capture still works.
- Capture results still route through the same save/copy/editor logic.
- The user gets a visible success signal when the editor is intentionally skipped.

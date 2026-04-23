# Tauri implementation plan

This page defines the preferred Tauri direction once the backend is ready. The
recommended target is Tauri v2 with a React and TypeScript frontend that talks
to a thin Rust command layer over explicit application services.

## Stack recommendation

The current repo can support this stack without fighting itself.

- React: yes. It is good fit for a multi-surface desktop shell with complex
  editor and settings state.
- TypeScript: yes. Use it from day one so command payloads and editor models
  stay typed.
- Tailwind: yes. It is fine for fast layout and tokenized styling.
- shadcn/ui: yes. Use it for primitives, not as design substitute.
- Framer Motion: yes, but selectively. Use CSS transitions by default and keep
  Framer Motion for transitions that truly benefit from layout choreography.

## Animation policy

The app does not need motion-heavy UI. It needs clear and fast UI.

- Prefer CSS transitions for hover, focus, toast, simple drawers, and small
  panel transitions.
- Use Framer Motion only for page transitions, overlay reveals, inspector
  drawer motion, and maybe stacked toast choreography.
- Do not put editor-canvas interaction on Framer Motion.
- Keep motion optional and short so screenshot workflows stay fast.

## Expected React components

This is the target component inventory for a serious desktop shell. Components
are grouped by domain, but the list is intentionally concrete.

### App shell

These components define global structure and app chrome.

- `AppShell`
- `SidebarNav`
- `TopBar`
- `WindowControls`
- `StatusBar`
- `CommandPalette`
- `GlobalShortcutsHelp`
- `GlobalErrorBoundary`

### Capture workflow

These components cover the first-run and repeat capture flow.

- `CaptureDashboard`
- `CaptureModeCards`
- `CaptureModeCard`
- `DelayPresetGroup`
- `QuickActionsPanel`
- `RecentCaptureStrip`
- `CaptureStatusBanner`
- `CaptureFailureDialog`
- `CaptureCancellationNotice`

### Editor workspace

These components replace the current GTK editor shell.

- `EditorWorkspace`
- `EditorToolbar`
- `AnnotationToolPicker`
- `CanvasViewport`
- `CanvasOverlay`
- `SelectionHandles`
- `ColorPalette`
- `StrokeSizeControl`
- `TextAnnotationInput`
- `InspectorPanel`
- `LayerActionsPanel`
- `HistoryControls`
- `ExportActionsPanel`
- `SaveLocationSummary`
- `SaveAsDialogTrigger`
- `CopyActionButton`
- `OpenAfterSaveToggle`
- `CloseAfterCopyToggle`

### Settings and integration

These components expose the current shared settings and Linux-only integration
controls.

- `SettingsPage`
- `CaptureDefaultsCard`
- `SaveDefaultsCard`
- `ClipboardSettingsCard`
- `TraySettingsCard`
- `AutostartControl`
- `ShortcutRecorderPanel`
- `ShortcutField`
- `HyprlandIntegrationCard`
- `PlatformSupportCard`

### Diagnostics

These components expose doctor and environment details.

- `DiagnosticsPage`
- `DoctorReportPanel`
- `DependencyChecklist`
- `PortalStatusPanel`
- `RepairActionsPanel`
- `EnvironmentFactsCard`

## React-side folder layout

The current repo already uses `src/` for Rust, so the React app should not use
that same top-level path. Use `ui/` as dedicated frontend workspace.

```text
ui/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── postcss.config.cjs
├── components.json
└── src/
    ├── main.tsx
    ├── app/
    │   ├── app.tsx
    │   ├── router.tsx
    │   ├── providers.tsx
    │   └── routes/
    │       ├── capture.tsx
    │       ├── editor.tsx
    │       ├── settings.tsx
    │       └── diagnostics.tsx
    ├── styles/
    │   ├── globals.css
    │   └── tokens.css
    ├── components/
    │   ├── shell/
    │   │   ├── app-shell.tsx
    │   │   ├── sidebar-nav.tsx
    │   │   ├── top-bar.tsx
    │   │   ├── status-bar.tsx
    │   │   └── command-palette.tsx
    │   ├── capture/
    │   │   ├── capture-dashboard.tsx
    │   │   ├── capture-mode-card.tsx
    │   │   ├── delay-preset-group.tsx
    │   │   ├── quick-actions-panel.tsx
    │   │   └── capture-status-banner.tsx
    │   ├── editor/
    │   │   ├── editor-workspace.tsx
    │   │   ├── editor-toolbar.tsx
    │   │   ├── annotation-tool-picker.tsx
    │   │   ├── canvas-viewport.tsx
    │   │   ├── canvas-overlay.tsx
    │   │   ├── selection-handles.tsx
    │   │   ├── color-palette.tsx
    │   │   ├── stroke-size-control.tsx
    │   │   ├── text-annotation-input.tsx
    │   │   ├── inspector-panel.tsx
    │   │   ├── history-controls.tsx
    │   │   └── export-actions-panel.tsx
    │   ├── settings/
    │   │   ├── capture-defaults-card.tsx
    │   │   ├── save-defaults-card.tsx
    │   │   ├── clipboard-settings-card.tsx
    │   │   ├── tray-settings-card.tsx
    │   │   ├── autostart-control.tsx
    │   │   ├── shortcut-recorder-panel.tsx
    │   │   └── hyprland-integration-card.tsx
    │   ├── diagnostics/
    │   │   ├── doctor-report-panel.tsx
    │   │   ├── dependency-checklist.tsx
    │   │   ├── portal-status-panel.tsx
    │   │   ├── repair-actions-panel.tsx
    │   │   └── environment-facts-card.tsx
    │   └── shared/
    │       ├── empty-state.tsx
    │       ├── error-view.tsx
    │       ├── loading-view.tsx
    │       ├── section-header.tsx
    │       └── status-pill.tsx
    ├── hooks/
    │   ├── use-app-settings.ts
    │   ├── use-capture-actions.ts
    │   ├── use-diagnostics.ts
    │   └── use-editor-session.ts
    ├── lib/
    │   ├── tauri/
    │   │   ├── commands.ts
    │   │   ├── events.ts
    │   │   └── types.ts
    │   ├── formatting.ts
    │   └── support-matrix.ts
    ├── store/
    │   ├── app-store.ts
    │   ├── editor-store.ts
    │   └── settings-store.ts
    └── test/
        ├── app-shell.test.tsx
        ├── capture-dashboard.test.tsx
        └── editor-workspace.test.tsx
```

## Rust-side expectation for Tauri

The frontend only works if the Rust side is intentionally thin.

- Keep Tauri entry in `src-tauri/src/lib.rs`.
- Expose commands for capture, settings, diagnostics, export, and Linux
  integration.
- Keep GTK-specific UI code out of that layer.
- Reuse backend services created in the pre-Tauri cleanup phase.

## Recommendation summary

If you want short answers to the stack questions, use these.

- Tailwind and shadcn/ui are fine.
- Framer Motion is fine, but not as default for every transition.
- React component count should be broad and explicit because current product has
  several overlapping surfaces.
- The React tree should live under `ui/`, not a top-level `src/` that collides
  with Rust.

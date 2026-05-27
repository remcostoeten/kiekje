export function createAppState() {
  return {
    tool: 'select',
    captureMode: true,
    image: new Image(),
    annotations: [],
    current: null,
    dragStart: null,
    penPoints: [],
    settings: { binds: {} },
    recording: {
      action: null,
      mods: new Set(),
      key: '',
    },
    input: {
      lastHotkeyTs: 0,
    },
    capture: {
      isCapturing: false,
      cancelled: false,
      selectionActive: false,
      selection: null,
      selectionStart: null,
      resolve: null,
      reject: null,
      windowPreview: null,
      windowPreviewPoint: null,
      windowPreviewReq: 0,
      screenOffset: { x: 0, y: 0 },
      raf: 0,
    },
    editor: {
      selectedIndices: [],
      hoveredIndex: -1,
      dragging: null,
      activeColor: '#ededed',
      inlineTextInput: null,
    },
  };
}

export function hasImage(state) {
  return state.image.complete && state.image.naturalWidth > 0;
}

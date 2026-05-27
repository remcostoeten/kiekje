import { EventsOn } from '../wailsjs/runtime/runtime';
import { mountTemplate } from './dom/template.js';
import { createAppState } from './state.js';
import { initTooltip } from './ui/tooltip.js';
import { initColors } from './ui/colors.js';
import { createCaptureOverlay } from './capture/overlay.js';
import { createCaptureFlow } from './capture/flow.js';
import { createSettingsMenu } from './settings/menu.js';
import { createSaveIO } from './io/save.js';
import { createEditorCanvas } from './editor/canvas.js';
import { initKeyboard } from './input/keyboard.js';

export function initApp() {
  const dom = mountTemplate(document.querySelector('#app'));
  const state = createAppState();

  const actions = {};
  const overlay = createCaptureOverlay({ dom, state, actions });
  const settings = createSettingsMenu({ dom, state });
  const saveIO = createSaveIO({ dom, state, settings });

  let editor;

  const colors = initColors({
    dom,
    state,
    onColorApplied: (color) => {
      state.editor.selectedIndices.forEach((i) => {
        if (i >= 0 && i < state.annotations.length) state.annotations[i].color = color;
      });
      editor.redraw();
    },
  });

  editor = createEditorCanvas({
    dom,
    state,
    colors,
    settings,
    saveIO,
  });

  Object.assign(actions, {
    setCapturing: editor.setCapturing,
    setIdle: editor.setIdle,
    resizeCanvas: editor.resizeCanvas,
    saveImageData: saveIO.saveImageData,
  });

  const captureFlow = createCaptureFlow({ dom, state, overlay, actions });

  dom.captureBtn.onclick = () => captureFlow.startCapture();

  initKeyboard({
    state,
    settings,
    overlay,
    captureFlow,
    editor,
    colors,
  });

  initTooltip(dom);

  EventsOn('cheese:capture', () => captureFlow.startCapture());
  EventsOn('cheese:choose-save-dir', () => settings.setMenuOpen(false));
  EventsOn('cheese:cancel-capture', () => {
    overlay.cancelCaptureOverlay('capture cancelled');
  });

  settings.loadState().then(() => {
    editor.setCapturing(false);
    editor.setIdle(true);
    editor.redraw();
  });

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden && !state.capture.isCapturing) editor.setCapturing(false);
  });

  window.addEventListener('resize', () => {
    if (!dom.menu.classList.contains('hidden')) settings.positionMenu();
    if (state.capture.selectionActive) overlay.resizeCaptureOverlay();
  });
}

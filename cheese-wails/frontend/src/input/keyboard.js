import { CancelCapture, QuitApp } from '../../wailsjs/go/main/App';
import { matchBind } from '../settings/binds.js';

export function initKeyboard({
  state,
  settings,
  overlay,
  captureFlow,
  editor,
  colors,
}) {
  function runAction(action) {
    if (action === 'capture') captureFlow.startCapture();
    else if (action === 'save') document.getElementById('save').click();
    else if (action === 'undo') document.getElementById('undo').click();
  }

  window.addEventListener('keydown', (evt) => {
    if (state.capture.selectionActive) {
      if (evt.key === 'Escape') {
        evt.preventDefault();
        overlay.cancelCaptureOverlay('capture cancelled');
      }
      return;
    }

    if (settings.handleRecordingKeydown(evt)) return;

    if (Date.now() - state.input.lastHotkeyTs < 120) return;
    for (const action of ['capture', 'save', 'undo']) {
      if (matchBind(state.settings.binds?.[action], evt)) {
        evt.preventDefault();
        state.input.lastHotkeyTs = Date.now();
        runAction(action);
        return;
      }
    }

    if (evt.key === 'Escape') {
      settings.setMenuOpen(false);
      colors.closeContextPicker();
      editor.setTool('select');
      return;
    }

    if ((evt.metaKey || evt.ctrlKey) && evt.key.toLowerCase() === 's') {
      evt.preventDefault();
      document.getElementById('save').click();
      return;
    }

    if ((evt.metaKey || evt.ctrlKey) && evt.key.toLowerCase() === 'z') {
      evt.preventDefault();
      document.getElementById('undo').click();
      return;
    }

    if ((evt.metaKey || evt.ctrlKey) && evt.key.toLowerCase() === 'c' && !evt.target.closest('input,textarea')) {
      evt.preventDefault();
      editor.copyToClipboardAndQuit();
      return;
    }

    if ((evt.metaKey || evt.ctrlKey) && evt.key.toLowerCase() === 'v' && !evt.target.closest('input,textarea')) {
      if (window.copyBuffer && window.copyBuffer.length > 0) {
        evt.preventDefault();
        window.copyBuffer.forEach((a) => {
          const copy = JSON.parse(JSON.stringify(a));
          const off = 15;
          if (copy.kind === 'pen' && copy.points) copy.points.forEach((p) => { p.x += off; p.y += off; });
          else { copy.x += off; copy.y += off; }
          state.annotations.push(copy);
        });
        state.editor.selectedIndices = [];
        editor.redraw();
      }
      return;
    }

    if (!state.recording.action
      && (evt.key === 'Delete' || evt.key === 'Backspace')
      && state.tool === 'select'
      && !evt.target.closest('input,textarea')
      && state.editor.selectedIndices.length > 0) {
      evt.preventDefault();
      state.editor.selectedIndices.sort((a, b) => b - a).forEach((i) => state.annotations.splice(i, 1));
      state.editor.selectedIndices = [];
      editor.redraw();
      return;
    }

    if (evt.target.closest('input,textarea')) return;

    if (evt.key === 'v') { editor.setTool('select'); return; }
    if (evt.key === 'r') { editor.setTool('rect'); return; }
    if (evt.key === 'a') { editor.setTool('arrow'); return; }
    if (evt.key === 'p') { editor.setTool('pen'); return; }
    if (evt.key === 't') { editor.setTool('text'); return; }
    if (evt.key === 'c' && !evt.ctrlKey && !evt.metaKey) { captureFlow.startCapture(); return; }

    if (evt.key.toLowerCase() === 'q') {
      evt.preventDefault();
      if (state.capture.isCapturing) {
        state.capture.cancelled = true;
        CancelCapture();
        window.runtime.WindowHide();
        return;
      }
      QuitApp();
    }
  });
}

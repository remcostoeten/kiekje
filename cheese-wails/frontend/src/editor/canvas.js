import { CopyImageToClipboard, QuitApp } from '../../wailsjs/go/main/App';
import { hasImage } from '../state.js';
import { pointerPos } from '../utils/geometry.js';
import {
  hitTest,
  moveAnnotation,
  annotationBounds,
  createAnnotationRenderer,
} from './annotations.js';
import { createInlineTextController } from './inline-text.js';
import { createEditorHistory } from './history.js';
import { createImageLayer } from './image-layer.js';
import {
  applyHandleResize,
  getResizeSnapshot,
  getSelectedResizeTarget,
  hitTestHandle,
  normalizeBoxAnnotation,
} from './transform.js';

export function createEditorCanvas({ dom, state, colors, settings, saveIO }) {
  const ctx = dom.canvas.getContext('2d');
  const imageLayer = createImageLayer(state);
  const { drawAnnotation } = createAnnotationRenderer(ctx, state);

  const history = createEditorHistory({
    state,
    getImageData: () => imageLayer.getImageData(),
    putImageData: (data) => imageLayer.putImageData(data),
    redraw: () => redraw(),
  });

  function redraw() {
    ctx.clearRect(0, 0, dom.canvas.width, dom.canvas.height);
    if (hasImage(state)) imageLayer.drawTo(ctx);
    state.annotations.forEach(drawAnnotation);
    if (state.current) drawAnnotation(state.current, true);
  }

  function setIdle(idle) {
    dom.canvas.classList.toggle('idle', idle);
    dom.editor.classList.toggle('editing', !idle);
    state.captureMode = idle;
    for (const id of ['undo', 'redo', 'save', 'copy']) {
      const el = document.getElementById(id);
      if (el) el.disabled = idle;
    }
    if (idle) {
      history.resetHistory();
      state.editor.stepCounter = 1;
    }
  }

  function setCapturing(on) {
    dom.editor.classList.toggle('capturing', on);
  }

  function resizeCanvas() {
    dom.canvas.width = state.image.naturalWidth || 1;
    dom.canvas.height = state.image.naturalHeight || 1;
    if (hasImage(state)) imageLayer.syncFromImage(state.image);
    setIdle(!hasImage(state));
    history.resetHistory();
    redraw();
  }

  function refreshImageFromLayer() {
    return new Promise((resolve) => {
      const src = imageLayer.toDataURL();
      if (!src) {
        resolve();
        return;
      }
      const img = new Image();
      img.onload = () => {
        state.image = img;
        dom.canvas.width = img.naturalWidth;
        dom.canvas.height = img.naturalHeight;
        resolve();
      };
      img.src = src;
    });
  }

  const inlineText = createInlineTextController({
    dom,
    state,
    redraw,
    recordBeforeChange: history.recordBeforeChange,
  });

  function setTool(name) {
    inlineText.commitInlineText();
    state.tool = name;
    document.querySelectorAll('[data-tool]').forEach((b) => b.classList.remove('on'));
    const btn = document.querySelector(`[data-tool="${name}"]`);
    if (btn) btn.classList.add('on');
    state.editor.selectedIndices = [];
    state.editor.hoveredIndex = -1;
    dom.canvas.style.cursor = name === 'select'
      ? 'default'
      : name === 'text'
        ? 'text'
        : 'crosshair';
  }

  function annotationDefaults() {
    return {
      color: state.editor.activeColor,
      strokeWidth: state.editor.strokeWidth,
    };
  }

  function findAnnotationAt(p) {
    for (let i = state.annotations.length - 1; i >= 0; i--) {
      if (hitTest(p, state.annotations[i], state)) return i;
    }
    return -1;
  }

  function finishBlurSelection() {
    if (!state.current || state.tool !== 'blur') return;
    const { x, y, w, h } = state.current;
    state.current = null;
    state.dragStart = null;
    if (Math.abs(w) < 4 || Math.abs(h) < 4) {
      redraw();
      return;
    }
    history.recordBeforeChange();
    if (imageLayer.applyBlurRect(x, y, w, h)) redraw();
  }

  async function finishCropSelection() {
    if (!state.current || state.tool !== 'crop') return;
    const { x, y, w, h } = state.current;
    state.current = null;
    state.dragStart = null;
    if (Math.abs(w) < 4 || Math.abs(h) < 4) {
      redraw();
      return;
    }
    history.recordBeforeChange();
    const crop = imageLayer.crop(x, y, w, h);
    if (!crop) {
      redraw();
      return;
    }
    state.annotations.forEach((a) => moveAnnotation(a, -crop.x, -crop.y));
    state.annotations = state.annotations.filter((a) => {
      const b = annotationBounds(a, state);
      return b.x + b.w > 0 && b.y + b.h > 0 && b.x < crop.w && b.y < crop.h;
    });
    state.editor.selectedIndices = [];
    await refreshImageFromLayer();
    redraw();
  }

  function finishAnnotation() {
    if (!state.current) return;
    if (state.tool === 'blur') {
      finishBlurSelection();
      return;
    }
    if (state.tool === 'crop') {
      finishCropSelection();
      return;
    }
    normalizeBoxAnnotation(state.current);
    history.recordBeforeChange();
    state.annotations.push(state.current);
    state.current = null;
    state.dragStart = null;
    redraw();
  }

  function isDragTool(name) {
    return ['rect', 'highlight', 'ellipse', 'blur', 'crop'].includes(name);
  }

  document.querySelectorAll('[data-tool]').forEach((btn) => {
    btn.addEventListener('click', () => {
      setTool(btn.dataset.tool);
    });
  });

  dom.strokeWidth?.addEventListener('input', (evt) => {
    state.editor.strokeWidth = Number(evt.target.value) || 3;
    redraw();
  });

  dom.canvas.addEventListener('pointerdown', (evt) => {
    if (state.capture.selectionActive || state.captureMode || !hasImage(state)) return;
    if (state.editor.inlineTextInput && evt.target !== state.editor.inlineTextInput) {
      inlineText.commitInlineText();
    }
    const p = pointerPos(dom.canvas, evt);

    if (state.tool === 'select') {
      const resizeTarget = getSelectedResizeTarget(state);
      if (resizeTarget) {
        const handle = hitTestHandle(p, resizeTarget.bounds);
        if (handle) {
          history.recordBeforeChange();
          state.editor.resizing = {
            handle,
            index: resizeTarget.index,
            snapshot: getResizeSnapshot(resizeTarget.annotation),
          };
          dom.canvas.setPointerCapture(evt.pointerId);
          redraw();
          return;
        }
      }

      const hit = findAnnotationAt(p);
      if (evt.ctrlKey || evt.metaKey) {
        if (hit >= 0) {
          const idx = state.editor.selectedIndices.indexOf(hit);
          if (idx >= 0) state.editor.selectedIndices.splice(idx, 1);
          else state.editor.selectedIndices.push(hit);
        }
      } else if (!(hit >= 0 && state.editor.selectedIndices.length === 1 && state.editor.selectedIndices[0] === hit)) {
        state.editor.selectedIndices = hit >= 0 ? [hit] : [];
      }
      if (hit >= 0 && state.editor.selectedIndices.includes(hit)) {
        history.recordBeforeChange();
        state.editor.dragging = { last: p };
      }
      dom.canvas.setPointerCapture(evt.pointerId);
      redraw();
      return;
    }

    if (state.tool === 'text') {
      evt.preventDefault();
      evt.stopPropagation();
      inlineText.openInlineText(p);
      return;
    }

    if (state.tool === 'step') {
      history.recordBeforeChange();
      state.annotations.push({
        kind: 'step',
        x: p.x,
        y: p.y,
        r: 18,
        n: state.editor.stepCounter++,
        ...annotationDefaults(),
      });
      redraw();
      return;
    }

    state.dragStart = p;
    if (state.tool === 'pen') {
      state.penPoints = [state.dragStart];
      state.current = { kind: 'pen', ...annotationDefaults(), points: state.penPoints };
    } else if (state.tool === 'blur' || state.tool === 'crop') {
      state.current = {
        kind: state.tool,
        x: state.dragStart.x,
        y: state.dragStart.y,
        w: 0,
        h: 0,
        ...annotationDefaults(),
      };
    } else {
      state.current = {
        kind: state.tool,
        x: state.dragStart.x,
        y: state.dragStart.y,
        w: 0,
        h: 0,
        text: '',
        ...annotationDefaults(),
      };
    }
    dom.canvas.setPointerCapture(evt.pointerId);
  });

  dom.canvas.addEventListener('pointermove', (evt) => {
    if (state.capture.selectionActive || state.captureMode || !hasImage(state)) return;
    const p = pointerPos(dom.canvas, evt);

    if (state.tool === 'select' && !state.editor.dragging && !state.editor.resizing) {
      const resizeTarget = getSelectedResizeTarget(state);
      if (resizeTarget) {
        const handle = hitTestHandle(p, resizeTarget.bounds);
        if (handle) {
          dom.canvas.style.cursor = `${handle}-resize`;
          return;
        }
      }
      const hit = findAnnotationAt(p);
      if (hit !== state.editor.hoveredIndex) {
        state.editor.hoveredIndex = hit;
        dom.canvas.style.cursor = hit >= 0
          ? (state.editor.selectedIndices.includes(hit) ? 'grabbing' : 'grab')
          : '';
        redraw();
      }
    } else if (!state.editor.dragging && !state.editor.resizing) {
      state.editor.hoveredIndex = -1;
      dom.canvas.style.cursor = state.tool === 'text' ? 'text' : 'crosshair';
    }

    if (state.editor.resizing) {
      const a = state.annotations[state.editor.resizing.index];
      if (a) {
        applyHandleResize(a, state.editor.resizing.handle, p, state.editor.resizing.snapshot);
        redraw();
      }
      return;
    }

    if (state.editor.dragging) {
      dom.canvas.style.cursor = 'grabbing';
      const dx = p.x - state.editor.dragging.last.x;
      const dy = p.y - state.editor.dragging.last.y;
      state.editor.selectedIndices.forEach((i) => {
        if (i >= 0 && i < state.annotations.length) moveAnnotation(state.annotations[i], dx, dy);
      });
      state.editor.dragging.last = p;
      redraw();
      return;
    }

    if (!state.dragStart || !state.current || state.tool === 'text' || state.tool === 'step') return;
    if (state.tool === 'pen') state.penPoints.push(p);
    else if (isDragTool(state.tool) || state.tool === 'arrow') {
      state.current.w = p.x - state.dragStart.x;
      state.current.h = p.y - state.dragStart.y;
    }
    redraw();
  });

  dom.canvas.addEventListener('pointerup', () => {
    if (state.capture.selectionActive) return;
    if (state.editor.resizing) {
      state.editor.resizing = null;
      dom.canvas.style.cursor = 'default';
      redraw();
      return;
    }
    if (state.editor.dragging) {
      state.editor.dragging = null;
      dom.canvas.style.cursor = 'default';
      redraw();
      return;
    }
    if (!state.current) return;
    finishAnnotation();
  });

  dom.canvas.addEventListener('contextmenu', (evt) => {
    if (state.captureMode || !hasImage(state)) return;
    evt.preventDefault();
    const p = pointerPos(dom.canvas, evt);
    const hit = findAnnotationAt(p);
    if (hit >= 0) {
      state.editor.selectedIndices = [hit];
      colors.openContextPicker(evt.clientX, evt.clientY, state.annotations[hit].color);
    } else {
      state.editor.selectedIndices = [];
      colors.openContextPicker(evt.clientX, evt.clientY, state.editor.activeColor);
    }
    redraw();
  });

  document.getElementById('undo').onclick = () => history.undo();
  document.getElementById('redo').onclick = () => history.redo();

  document.getElementById('save').onclick = async () => {
    if (!hasImage(state)) return;
    const saved = await saveIO.saveCanvas();
    if (saved.closeAfterSave) saveIO.closeAfterSave(setIdle);
  };

  document.getElementById('copy').onclick = async () => {
    if (!hasImage(state)) return;
    const blob = await new Promise((r) => dom.canvas.toBlob(r, 'image/png'));
    const reader = new FileReader();
    reader.onloadend = async () => {
      const tmp = await saveIO.saveImageData(reader.result, state.settings.outputPath || '');
      if (tmp.lastSavedPath) await CopyImageToClipboard(tmp.lastSavedPath);
    };
    reader.readAsDataURL(blob);
  };

  async function copyToClipboardAndQuit() {
    if (!hasImage(state)) return;
    const blob = await new Promise((r) => dom.canvas.toBlob(r, 'image/png'));
    const reader = new FileReader();
    reader.onloadend = async () => {
      const tmp = await saveIO.saveImageData(reader.result, state.settings.outputPath || '');
      if (tmp.lastSavedPath) await CopyImageToClipboard(tmp.lastSavedPath);
      await QuitApp();
    };
    reader.readAsDataURL(blob);
  }

  history.updateButtons();

  return {
    redraw,
    setIdle,
    setCapturing,
    resizeCanvas,
    setTool,
    copyToClipboardAndQuit,
    recordBeforeChange: history.recordBeforeChange,
    deleteSelected: () => {
      if (state.editor.selectedIndices.length === 0) return;
      history.recordBeforeChange();
      state.editor.selectedIndices.sort((a, b) => b - a).forEach((i) => state.annotations.splice(i, 1));
      state.editor.selectedIndices = [];
      redraw();
    },
    pasteOffset: (items) => {
      history.recordBeforeChange();
      items.forEach((a) => {
        const copy = JSON.parse(JSON.stringify(a));
        const off = 15;
        if (copy.kind === 'pen' && copy.points) copy.points.forEach((pt) => { pt.x += off; pt.y += off; });
        else { copy.x += off; copy.y += off; }
        state.annotations.push(copy);
      });
      state.editor.selectedIndices = [];
      redraw();
    },
  };
}

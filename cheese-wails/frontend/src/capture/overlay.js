import { GetWindowGeometryAtPoint } from '../../wailsjs/go/main/App';
import {
  clamp,
  capturePoint,
  captureSelectionRect,
  captureGeometryToScreen,
} from '../utils/geometry.js';

function setBodyCaptureState(on) {
  document.body.classList.toggle('capturing', on);
}

export function createCaptureOverlay({ dom, state, actions }) {
  const captureCtx = dom.captureCanvas.getContext('2d');
  const capture = state.capture;

  function scheduleCaptureOverlayDraw() {
    if (!capture.selectionActive) return;
    if (capture.raf) return;
    capture.raf = requestAnimationFrame(() => {
      capture.raf = 0;
      drawCaptureOverlay();
    });
  }

  function resizeCaptureOverlay() {
    const dpr = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.round(window.innerWidth));
    const height = Math.max(1, Math.round(window.innerHeight));
    dom.captureCanvas.width = Math.max(1, Math.round(width * dpr));
    dom.captureCanvas.height = Math.max(1, Math.round(height * dpr));
    dom.captureCanvas.style.width = `${width}px`;
    dom.captureCanvas.style.height = `${height}px`;
    captureCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
    scheduleCaptureOverlayDraw();
  }

  function drawRuler(axisLength, isHorizontal) {
    const ctx = captureCtx;
    const major = 100;
    const minor = 10;
    ctx.strokeStyle = 'rgba(237, 237, 237, 0.35)';
    ctx.fillStyle = 'rgba(237, 237, 237, 0.8)';
    ctx.font = '11px "JetBrains Mono", monospace';
    ctx.textBaseline = 'top';
    ctx.textAlign = 'center';

    for (let i = 0; i <= axisLength; i += minor) {
      const majorTick = i % major === 0;
      const midTick = i % (major / 2) === 0;
      const tick = majorTick ? 16 : midTick ? 11 : 7;
      ctx.beginPath();
      if (isHorizontal) {
        ctx.moveTo(i + 0.5, 0);
        ctx.lineTo(i + 0.5, tick);
      } else {
        ctx.moveTo(0, i + 0.5);
        ctx.lineTo(tick, i + 0.5);
      }
      ctx.stroke();

      if (majorTick && i > 0) {
        if (isHorizontal) {
          const label = `${i}`;
          const labelWidth = ctx.measureText(label).width + 8;
          const labelX = clamp(i - labelWidth / 2, 4, axisLength - labelWidth - 4);
          ctx.fillStyle = 'rgba(0, 0, 0, 0.72)';
          ctx.fillRect(labelX, 18, labelWidth, 18);
          ctx.fillStyle = 'rgba(237, 237, 237, 0.82)';
          ctx.fillText(label, labelX + labelWidth / 2, 21);
        } else {
          const label = `${i}`;
          const labelWidth = ctx.measureText(label).width;
          ctx.textAlign = 'left';
          ctx.fillStyle = 'rgba(0, 0, 0, 0.72)';
          ctx.fillRect(18, i - 9, labelWidth + 8, 18);
          ctx.fillStyle = 'rgba(237, 237, 237, 0.82)';
          ctx.fillText(label, 22, i - 6);
          ctx.textAlign = 'center';
        }
      }
    }
  }

  function drawLabelBox(text, x, y, align = 'left', accent = false) {
    const ctx = captureCtx;
    ctx.font = '11px "JetBrains Mono", monospace';
    const width = ctx.measureText(text).width + 12;
    const height = 20;
    let drawX = x;
    if (align === 'right') drawX = x - width;
    if (align === 'center') drawX = x - width / 2;
    drawX = clamp(drawX, 4, dom.captureCanvas.width / (window.devicePixelRatio || 1) - width - 4);
    ctx.fillStyle = accent ? 'rgba(11, 40, 20, 0.88)' : 'rgba(0, 0, 0, 0.78)';
    ctx.fillRect(drawX, y, width, height);
    ctx.strokeStyle = accent ? 'rgba(105, 219, 124, 0.85)' : 'rgba(237, 237, 237, 0.15)';
    ctx.strokeRect(drawX + 0.5, y + 0.5, width - 1, height - 1);
    ctx.fillStyle = accent ? '#d7ffe0' : '#ededed';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, drawX + width / 2, y + height / 2 + 0.5);
  }

  function resetWindowPickPreview() {
    capture.windowPreview = null;
    capture.windowPreviewPoint = null;
    capture.windowPreviewReq += 1;
  }

  function scheduleWindowPickPreview(point) {
    if (!capture.selectionActive || capture.selectionStart) return;
    const req = ++capture.windowPreviewReq;
    capture.windowPreviewPoint = point;
    GetWindowGeometryAtPoint(
      Math.round(capture.screenOffset.x + point.x),
      Math.round(capture.screenOffset.y + point.y),
    ).then((geom) => {
      if (req !== capture.windowPreviewReq || !capture.selectionActive || capture.selectionStart || !capture.windowPreviewPoint) return;
      capture.windowPreview = geom;
      scheduleCaptureOverlayDraw();
    }).catch(() => {
      if (req !== capture.windowPreviewReq) return;
      capture.windowPreview = null;
      scheduleCaptureOverlayDraw();
    });
  }

  async function confirmWindowPickAt(point) {
    const geom = await GetWindowGeometryAtPoint(
      Math.round(capture.screenOffset.x + point.x),
      Math.round(capture.screenOffset.y + point.y),
    );
    const resolve = capture.resolve;
    capture.resolve = null;
    capture.reject = null;
    resetWindowPickPreview();
    resetCaptureOverlay();
    if (resolve) resolve(geom);
  }

  function drawCaptureOverlay() {
    if (!capture.selectionActive) return;
    const width = Math.max(1, Math.round(window.innerWidth));
    const height = Math.max(1, Math.round(window.innerHeight));
    const ctx = captureCtx;
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = 'rgba(0, 0, 0, 0.38)';
    ctx.fillRect(0, 0, width, height);

    ctx.save();
    drawRuler(width, true);
    drawRuler(height, false);
    ctx.restore();

    if (capture.windowPreview) {
      const x = capture.windowPreview.x;
      const y = capture.windowPreview.y;
      const w = capture.windowPreview.w;
      const h = capture.windowPreview.h;

      ctx.save();
      ctx.fillStyle = 'rgba(105, 219, 124, 0.10)';
      ctx.fillRect(x, y, w, h);
      ctx.strokeStyle = '#69db7c';
      ctx.lineWidth = 1.8;
      ctx.setLineDash([10, 5]);
      ctx.strokeRect(x + 0.5, y + 0.5, Math.max(0, w - 1), Math.max(0, h - 1));
      ctx.strokeStyle = 'rgba(105, 219, 124, 0.85)';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.strokeRect(x + 2.5, y + 2.5, Math.max(0, w - 5), Math.max(0, h - 5));
      ctx.setLineDash([]);
      drawLabelBox('window', x, Math.max(8, y - 30), 'left', true);
      drawLabelBox(`${w} × ${h} px`, x + w, Math.max(8, y - 30), 'right', true);
      ctx.restore();
    }

    if (!capture.selectionStart || !capture.selection) return;

    const rect = captureSelectionRect(capture.selectionStart, capture.selection);
    if (!rect) return;
    const x = rect.x;
    const y = rect.y;
    const w = rect.w;
    const h = rect.h;

    ctx.save();
    ctx.fillStyle = 'rgba(237, 237, 237, 0.06)';
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = '#ededed';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([8, 4]);
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    ctx.strokeStyle = 'rgba(124, 92, 252, 0.85)';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.strokeRect(x + 1.5, y + 1.5, Math.max(0, w - 3), Math.max(0, h - 3));
    ctx.setLineDash([]);

    drawLabelBox(`x:${rect.x} y:${rect.y}`, x, Math.max(8, y - 30));
    drawLabelBox(`${rect.w} × ${rect.h} px`, x + w, Math.max(8, y - 30), 'right');

    ctx.restore();
  }

  function resetCaptureOverlay() {
    capture.selectionActive = false;
    capture.selection = null;
    capture.selectionStart = null;
    resetWindowPickPreview();
    dom.captureOverlay.classList.add('hidden');
    setBodyCaptureState(false);
    if (capture.raf) {
      cancelAnimationFrame(capture.raf);
      capture.raf = 0;
    }
    captureCtx.clearRect(0, 0, dom.captureCanvas.width, dom.captureCanvas.height);
  }

  function beginCaptureOverlay() {
    resetCaptureOverlay();
    capture.selectionActive = true;
    dom.captureOverlay.classList.remove('hidden');
    setBodyCaptureState(true);
    window.runtime.WindowSetBackgroundColour(0, 0, 0, 0);
    window.runtime.WindowShow();
    window.runtime.WindowSetAlwaysOnTop(true);
    window.runtime.WindowFullscreen();
    capture.screenOffset = {
      x: Math.round(window.screenX || window.screenLeft || 0),
      y: Math.round(window.screenY || window.screenTop || 0),
    };
    resizeCaptureOverlay();
    requestAnimationFrame(resizeCaptureOverlay);
  }

  function cancelCaptureOverlay(reason = 'capture cancelled') {
    if (!capture.selectionActive) return;
    const reject = capture.reject;
    capture.reject = null;
    capture.resolve = null;
    resetCaptureOverlay();
    dom.captureBtn.disabled = false;
    capture.isCapturing = false;
    capture.cancelled = true;
    actions.setCapturing(false);
    window.runtime.WindowSetBackgroundColour(0, 0, 0, 1);
    window.runtime.WindowUnfullscreen();
    window.runtime.WindowHide();
    if (reject) reject(new Error(reason));
  }

  async function endCaptureOverlay() {
    resetCaptureOverlay();
    window.runtime.WindowSetBackgroundColour(0, 0, 0, 1);
    window.runtime.WindowUnfullscreen();
  }

  function finalizeCaptureSelection() {
    const rect = captureSelectionRect(capture.selectionStart, capture.selection);
    if (!rect || rect.w <= 0 || rect.h <= 0) {
      cancelCaptureOverlay('no region selected');
      return;
    }
    const geom = captureGeometryToScreen(rect, capture.screenOffset);
    const resolve = capture.resolve;
    capture.resolve = null;
    capture.reject = null;
    resetCaptureOverlay();
    if (resolve) resolve(geom);
  }

  dom.captureCanvas.addEventListener('pointerdown', (evt) => {
    if (!capture.selectionActive) return;
    evt.preventDefault();
    evt.stopPropagation();
    if (evt.button === 0 && evt.ctrlKey) {
      confirmWindowPickAt(capturePoint(evt)).catch((err) => {
        console.error(err);
        cancelCaptureOverlay('no window found');
      });
      return;
    }
    resetWindowPickPreview();
    capture.selectionStart = capturePoint(evt);
    capture.selection = { ...capture.selectionStart };
    dom.captureCanvas.setPointerCapture(evt.pointerId);
    scheduleCaptureOverlayDraw();
  });

  dom.captureCanvas.addEventListener('pointermove', (evt) => {
    if (!capture.selectionActive) return;
    evt.preventDefault();
    if (evt.ctrlKey && !capture.selectionStart) {
      scheduleWindowPickPreview(capturePoint(evt));
      return;
    }
    if (capture.windowPreview) {
      resetWindowPickPreview();
      scheduleCaptureOverlayDraw();
    }
    if (!capture.selectionStart) return;
    capture.selection = capturePoint(evt);
    scheduleCaptureOverlayDraw();
  });

  dom.captureCanvas.addEventListener('pointerup', (evt) => {
    if (!capture.selectionActive || !capture.selectionStart) return;
    evt.preventDefault();
    capture.selection = capturePoint(evt);
    finalizeCaptureSelection();
  });

  dom.captureCanvas.addEventListener('pointercancel', () => {
    if (!capture.selectionActive) return;
    cancelCaptureOverlay('capture cancelled');
  });

  window.addEventListener('keyup', (evt) => {
    if (!capture.selectionActive) return;
    if (evt.key === 'Control') {
      resetWindowPickPreview();
      scheduleCaptureOverlayDraw();
    }
  });

  return {
    beginCaptureOverlay,
    cancelCaptureOverlay,
    endCaptureOverlay,
    resizeCaptureOverlay,
    resetWindowPickPreview,
    scheduleCaptureOverlayDraw,
  };
}

import {
  CaptureRegionAt,
  CopyImageToClipboard,
  FinishCapture,
  ShowCaptureSuccessToast,
} from '../../wailsjs/go/main/App';

export function createCaptureFlow({ dom, state, overlay, actions }) {
  async function processCaptureResult(res, saveImageData) {
    if (!res?.data) throw new Error('No image');

    if (state.settings.clipboardOnlyCapture) {
      await CopyImageToClipboard(res.path);
      ShowCaptureSuccessToast();
      window.runtime.WindowHide();
      return;
    }

    if (state.settings.copyAfterCapture) await CopyImageToClipboard(res.path);
    if (state.settings.closeAfterCapture) {
      await saveImageData(res.data, state.settings.outputPath || '');
      window.runtime.WindowHide();
      state.image = new Image();
      actions.setIdle(true);
      return;
    }

    state.annotations = [];
    state.current = null;
    state.dragStart = null;
    state.penPoints = [];
    state.image = new Image();
    state.image.onload = () => {
      actions.resizeCanvas();
      window.runtime.WindowShow();
      window.runtime.WindowCenter();
    };
    state.image.src = `data:image/png;base64,${res.data}`;
    state.captureMode = false;
  }

  async function startCapture() {
    const capture = state.capture;
    if (capture.isCapturing) return;
    capture.cancelled = false;
    capture.isCapturing = true;
    actions.setCapturing(true);
    actions.setMenuOpen(false);
    dom.captureBtn.disabled = true;
    try {
      const selection = await new Promise((resolve, reject) => {
        capture.resolve = resolve;
        capture.reject = reject;
        try {
          overlay.beginCaptureOverlay();
        } catch (err) {
          capture.resolve = null;
          capture.reject = null;
          reject(err);
        }
      });
      await overlay.endCaptureOverlay();
      window.runtime.WindowHide();
      const res = await CaptureRegionAt(selection.x, selection.y, selection.w, selection.h);
      await processCaptureResult(res, actions.saveImageData);
    } catch (err) {
      console.error(err);
      if (!capture.cancelled && !String(err).includes('cancelled')) {
        try {
          await overlay.endCaptureOverlay();
        } catch {
          // ignore restore failures
        }
        window.runtime.WindowShow();
        window.runtime.WindowCenter();
      }
    } finally {
      capture.isCapturing = false;
      actions.setCapturing(false);
      dom.captureBtn.disabled = false;
      if (!capture.cancelled) await FinishCapture();
    }
  }

  return { startCapture, processCaptureResult };
}

import {
  CaptureRegion,
  CaptureWindow,
  CopyImageToClipboard,
  FinishCapture,
  ShowCaptureSuccessToast,
  ShowWindow,
} from '../../wailsjs/go/main/App';

export function createCaptureFlow({ dom, state, actions, settings }) {
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
      ShowWindow();
    };
    state.image.src = `data:image/png;base64,${res.data}`;
    state.captureMode = false;
  }

  async function startCapture({ windowOnly = false } = {}) {
    const capture = state.capture;
    if (capture.isCapturing) return;
    capture.cancelled = false;
    capture.isCapturing = true;
    settings.setMenuOpen(false);
    dom.captureBtn.disabled = true;
    window.runtime.WindowHide();
    try {
      const res = windowOnly ? await CaptureWindow() : await CaptureRegion();
      await processCaptureResult(res, actions.saveImageData);
    } catch (err) {
      console.error(err);
      if (!capture.cancelled && !String(err).includes('cancelled')) {
        ShowWindow();
      } else {
        window.runtime.WindowHide();
      }
    } finally {
      capture.isCapturing = false;
      dom.captureBtn.disabled = false;
      if (!capture.cancelled) await FinishCapture();
    }
  }

  return { startCapture, processCaptureResult };
}

export function createImageLayer(state) {
  let layer = null;

  function ensureLayer(width, height) {
    if (!layer) {
      layer = document.createElement('canvas');
      state.editor.imageLayer = layer;
    }
    if (layer.width !== width || layer.height !== height) {
      layer.width = width;
      layer.height = height;
    }
    return layer;
  }

  function syncFromImage(image) {
    const canvas = ensureLayer(image.naturalWidth, image.naturalHeight);
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(image, 0, 0);
    return canvas;
  }

  function getImageData() {
    if (!layer) return null;
    return layer.getContext('2d').getImageData(0, 0, layer.width, layer.height);
  }

  function putImageData(data) {
    if (!layer || !data) return;
    layer.getContext('2d').putImageData(data, 0, 0);
  }

  function applyBlurRect(x, y, w, h) {
    if (!layer) return false;

    const x1 = Math.max(0, Math.floor(Math.min(x, x + w)));
    const y1 = Math.max(0, Math.floor(Math.min(y, y + h)));
    const x2 = Math.min(layer.width, Math.ceil(Math.max(x, x + w)));
    const y2 = Math.min(layer.height, Math.ceil(Math.max(y, y + h)));
    const rw = x2 - x1;
    const rh = y2 - y1;
    if (rw < 2 || rh < 2) return false;

    const slice = document.createElement('canvas');
    slice.width = rw;
    slice.height = rh;
    slice.getContext('2d').drawImage(layer, x1, y1, rw, rh, 0, 0, rw, rh);

    const blurred = document.createElement('canvas');
    blurred.width = rw;
    blurred.height = rh;
    const bctx = blurred.getContext('2d');
    bctx.filter = 'blur(14px)';
    bctx.drawImage(slice, 0, 0);

    layer.getContext('2d').drawImage(blurred, 0, 0, rw, rh, x1, y1, rw, rh);
    return true;
  }

  function drawTo(ctx) {
    if (layer) ctx.drawImage(layer, 0, 0);
  }

  function reset() {
    layer = null;
    state.editor.imageLayer = null;
  }

  return {
    syncFromImage,
    getImageData,
    putImageData,
    applyBlurRect,
    drawTo,
    reset,
  };
}

export function clamp(n, min, max) {
  return Math.max(min, Math.min(max, n));
}

export function capturePoint(evt) {
  return { x: evt.clientX, y: evt.clientY };
}

export function captureSelectionRect(selectionStart, selection) {
  if (!selectionStart || !selection) return null;
  const x1 = Math.min(selectionStart.x, selection.x);
  const y1 = Math.min(selectionStart.y, selection.y);
  const x2 = Math.max(selectionStart.x, selection.x);
  const y2 = Math.max(selectionStart.y, selection.y);
  return {
    x: Math.round(x1),
    y: Math.round(y1),
    w: Math.max(0, Math.round(x2 - x1)),
    h: Math.max(0, Math.round(y2 - y1)),
  };
}

export function captureGeometryToScreen(rect, screenOffset) {
  return {
    x: Math.max(0, Math.round(screenOffset.x + rect.x)),
    y: Math.max(0, Math.round(screenOffset.y + rect.y)),
    w: Math.max(0, Math.round(rect.w)),
    h: Math.max(0, Math.round(rect.h)),
  };
}

export function pointerPos(canvas, evt) {
  const rect = canvas.getBoundingClientRect();
  const sx = canvas.width / rect.width;
  const sy = canvas.height / rect.height;
  return { x: (evt.clientX - rect.left) * sx, y: (evt.clientY - rect.top) * sy };
}

export function canvasPointToEditor(canvas, editor, p) {
  const canvasRect = canvas.getBoundingClientRect();
  const editorRect = editor.getBoundingClientRect();
  return {
    x: canvasRect.left - editorRect.left + (p.x / canvas.width) * canvasRect.width,
    y: canvasRect.top - editorRect.top + (p.y / canvas.height) * canvasRect.height,
  };
}

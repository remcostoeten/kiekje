import { annotationBounds } from './annotations.js';

const MIN_SIZE = 8;
const HANDLE_SIZE = 8;
const FRAME_PAD = 6;

export function canResizeAnnotation(a) {
  return ['rect', 'highlight', 'ellipse', 'arrow', 'text', 'step'].includes(a.kind);
}

export function getHandlePoints(bounds, pad = FRAME_PAD) {
  const x = bounds.x - pad;
  const y = bounds.y - pad;
  const w = bounds.w + pad * 2;
  const h = bounds.h + pad * 2;
  return {
    nw: { x, y },
    ne: { x: x + w, y },
    sw: { x, y: y + h },
    se: { x: x + w, y: y + h },
  };
}

export function hitTestHandle(p, bounds, pad = FRAME_PAD) {
  const handles = getHandlePoints(bounds, pad);
  for (const [name, pt] of Object.entries(handles)) {
    if (Math.abs(p.x - pt.x) <= HANDLE_SIZE && Math.abs(p.y - pt.y) <= HANDLE_SIZE) {
      return name;
    }
  }
  return null;
}

function clampRect(x1, y1, x2, y2) {
  let left = Math.min(x1, x2);
  let right = Math.max(x1, x2);
  let top = Math.min(y1, y2);
  let bottom = Math.max(y1, y2);
  if (right - left < MIN_SIZE) {
    if (x2 >= x1) right = left + MIN_SIZE;
    else left = right - MIN_SIZE;
  }
  if (bottom - top < MIN_SIZE) {
    if (y2 >= y1) bottom = top + MIN_SIZE;
    else top = bottom - MIN_SIZE;
  }
  return { left, top, right, bottom };
}

export function normalizeBoxAnnotation(a) {
  if (!['rect', 'highlight', 'ellipse'].includes(a.kind)) return;
  const x1 = Math.min(a.x, a.x + a.w);
  const y1 = Math.min(a.y, a.y + a.h);
  a.x = x1;
  a.y = y1;
  a.w = Math.max(MIN_SIZE, Math.abs(a.w));
  a.h = Math.max(MIN_SIZE, Math.abs(a.h));
}

export function applyHandleResize(a, handle, pointer, snapshot) {
  if (a.kind === 'arrow') {
    if (handle === 'se') {
      a.w = pointer.x - snapshot.x;
      a.h = pointer.y - snapshot.y;
    } else if (handle === 'nw') {
      a.w = snapshot.x + snapshot.w - pointer.x;
      a.h = snapshot.y + snapshot.h - pointer.y;
      a.x = pointer.x;
      a.y = pointer.y;
    } else if (handle === 'ne') {
      a.w = pointer.x - snapshot.x;
      a.h = snapshot.y + snapshot.h - pointer.y;
      a.y = pointer.y;
    } else if (handle === 'sw') {
      a.w = snapshot.x + snapshot.w - pointer.x;
      a.h = pointer.y - snapshot.y;
      a.x = pointer.x;
    }
    return;
  }

  if (a.kind === 'step') {
    const bounds = annotationBounds(snapshot);
    const cx = bounds.x + bounds.w / 2;
    const cy = bounds.y + bounds.h / 2;
    const dx = Math.abs(pointer.x - cx);
    const dy = Math.abs(pointer.y - cy);
    a.r = Math.max(MIN_SIZE, Math.max(dx, dy));
    a.x = cx;
    a.y = cy;
    return;
  }

  if (a.kind === 'text') {
    const bounds = {
      x: Math.min(snapshot.x, snapshot.x + (snapshot.w || 120)),
      y: snapshot.y,
      w: Math.abs(snapshot.w || 120),
      h: Math.abs(snapshot.h || 24),
    };
    const { left, top, right, bottom } = clampRect(
      handle.includes('w') ? pointer.x : bounds.x,
      handle.includes('n') ? pointer.y : bounds.y,
      handle.includes('e') ? pointer.x : bounds.x + bounds.w,
      handle.includes('s') ? pointer.y : bounds.y + bounds.h,
    );
    const scale = Math.max(0.5, (bottom - top) / Math.max(bounds.h, 1));
    a.x = left;
    a.y = top;
    a.w = right - left;
    a.h = bottom - top;
    a.strokeWidth = Math.max(1, Math.round((snapshot.strokeWidth || 3) * scale));
    return;
  }

  const ox = Math.min(snapshot.x, snapshot.x + snapshot.w);
  const oy = Math.min(snapshot.y, snapshot.y + snapshot.h);
  const ow = Math.abs(snapshot.w);
  const oh = Math.abs(snapshot.h);
  const { left, top, right, bottom } = clampRect(
    handle.includes('w') ? pointer.x : ox,
    handle.includes('n') ? pointer.y : oy,
    handle.includes('e') ? pointer.x : ox + ow,
    handle.includes('s') ? pointer.y : oy + oh,
  );
  a.x = left;
  a.y = top;
  a.w = right - left;
  a.h = bottom - top;
}

export function getResizeSnapshot(a) {
  return structuredClone(a);
}

export function getSelectedResizeTarget(state) {
  if (state.editor.selectedIndices.length !== 1) return null;
  const a = state.annotations[state.editor.selectedIndices[0]];
  if (!a || !canResizeAnnotation(a)) return null;
  return { index: state.editor.selectedIndices[0], annotation: a, bounds: annotationBounds(a) };
}

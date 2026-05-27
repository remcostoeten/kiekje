export function hexToRgba(hex, alpha) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

export function textMetrics(a, state) {
  const size = Math.round((a.strokeWidth || state.editor.strokeWidth || 2) * 6);
  return { width: Math.max(40, a.w || 120), height: Math.max(20, a.h || size + 6), size };
}

export function hitTest(p, a, state) {
  const margin = 6;
  if (a.kind === 'rect' || a.kind === 'highlight') {
    const x1 = Math.min(a.x, a.x + a.w);
    const x2 = Math.max(a.x, a.x + a.w);
    const y1 = Math.min(a.y, a.y + a.h);
    const y2 = Math.max(a.y, a.y + a.h);
    return p.x >= x1 - margin && p.x <= x2 + margin && p.y >= y1 - margin && p.y <= y2 + margin;
  }
  if (a.kind === 'ellipse') {
    const cx = a.x + a.w / 2;
    const cy = a.y + a.h / 2;
    const rx = Math.abs(a.w) / 2 + margin;
    const ry = Math.abs(a.h) / 2 + margin;
    if (rx < 1 || ry < 1) return false;
    const nx = (p.x - cx) / rx;
    const ny = (p.y - cy) / ry;
    return nx * nx + ny * ny <= 1;
  }
  if (a.kind === 'step') {
    const r = a.r || 18;
    const dx = p.x - a.x;
    const dy = p.y - a.y;
    return dx * dx + dy * dy <= (r + margin) * (r + margin);
  }
  if (a.kind === 'arrow') {
    const dx = a.w;
    const dy = a.h;
    const len = Math.sqrt(dx * dx + dy * dy);
    if (len < 1) return Math.abs(p.x - a.x) < margin && Math.abs(p.y - a.y) < margin;
    const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / (len * len)));
    const cx = a.x + t * dx;
    const cy = a.y + t * dy;
    return Math.abs(p.x - cx) < margin && Math.abs(p.y - cy) < margin;
  }
  if (a.kind === 'pen' && a.points) {
    for (let i = 1; i < a.points.length; i++) {
      const p1 = a.points[i - 1];
      const p2 = a.points[i];
      const dx = p2.x - p1.x;
      const dy = p2.y - p1.y;
      const len = Math.sqrt(dx * dx + dy * dy);
      if (len < 1) {
        if (Math.abs(p.x - p1.x) < margin && Math.abs(p.y - p1.y) < margin) return true;
        continue;
      }
      const t = Math.max(0, Math.min(1, ((p.x - p1.x) * dx + (p.y - p1.y) * dy) / (len * len)));
      const cx = p1.x + t * dx;
      const cy = p1.y + t * dy;
      if (Math.abs(p.x - cx) < margin && Math.abs(p.y - cy) < margin) return true;
    }
    return false;
  }
  if (a.kind === 'text') {
    const { width, height } = textMetrics(a, state);
    return p.x >= a.x - margin && p.x <= a.x + width + margin &&
           p.y >= a.y - margin && p.y <= a.y + height + margin;
  }
  return false;
}

export function annotationBounds(a, state) {
  if (a.kind === 'rect' || a.kind === 'highlight' || a.kind === 'ellipse' || a.kind === 'arrow') {
    return {
      x: Math.min(a.x, a.x + a.w),
      y: Math.min(a.y, a.y + a.h),
      w: Math.abs(a.w),
      h: Math.abs(a.h),
    };
  }
  if (a.kind === 'step') {
    const r = a.r || 18;
    return { x: a.x - r, y: a.y - r, w: r * 2, h: r * 2 };
  }
  if (a.kind === 'pen' && a.points && a.points.length) {
    const xs = a.points.map((p) => p.x);
    const ys = a.points.map((p) => p.y);
    return {
      x: Math.min(...xs),
      y: Math.min(...ys),
      w: Math.max(...xs) - Math.min(...xs),
      h: Math.max(...ys) - Math.min(...ys),
    };
  }
  if (a.kind === 'text') {
    const { width, height } = textMetrics(a, state || { editor: { strokeWidth: 3 } });
    return { x: a.x, y: a.y, w: width, h: height };
  }
  return { x: a.x, y: a.y, w: 122, h: 24 };
}

export function moveAnnotation(a, dx, dy) {
  if (a.kind === 'pen' && a.points) {
    a.points.forEach((pt) => {
      pt.x += dx;
      pt.y += dy;
    });
    return;
  }
  a.x += dx;
  a.y += dy;
}

export function createAnnotationRenderer(ctx, state) {
  function drawSelectionFrame(bounds, selected) {
    const pad = selected ? 6 : 5;
    const x = bounds.x - pad;
    const y = bounds.y - pad;
    const w = bounds.w + pad * 2;
    const h = bounds.h + pad * 2;
    ctx.save();
    ctx.shadowColor = selected ? 'rgba(124, 92, 252, 0.55)' : 'rgba(237, 237, 237, 0.35)';
    ctx.shadowBlur = selected ? 14 : 10;
    ctx.fillStyle = selected ? 'rgba(124, 92, 252, 0.08)' : 'rgba(237, 237, 237, 0.04)';
    ctx.strokeStyle = selected ? '#a78bfa' : 'rgba(237, 237, 237, 0.9)';
    ctx.lineWidth = selected ? 2 : 1.5;
    ctx.setLineDash(selected ? [6, 4] : [2, 4]);
    ctx.fillRect(x, y, w, h);
    ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
    ctx.setLineDash([]);
    ctx.shadowBlur = 0;
    if (selected) {
      const size = 6;
      ctx.fillStyle = '#f5f3ff';
      ctx.strokeStyle = '#111';
      ctx.lineWidth = 1;
      [[x, y], [x + w, y], [x, y + h], [x + w, y + h]].forEach(([px, py]) => {
        ctx.fillRect(px - size / 2, py - size / 2, size, size);
        ctx.strokeRect(px - size / 2 + 0.5, py - size / 2 + 0.5, size - 1, size - 1);
      });
    }
    ctx.restore();
  }

  function drawAnnotation(a, preview = false) {
    const idx = state.annotations.indexOf(a);
    const isSelected = state.editor.selectedIndices.includes(idx) && !preview;
    const isHovered = idx === state.editor.hoveredIndex && !preview && state.tool === 'select';
    ctx.lineWidth = a.strokeWidth || state.editor.strokeWidth || 2;
    const color = a.color || '#ededed';
    ctx.strokeStyle = color;
    ctx.fillStyle = hexToRgba(color, preview ? 0.05 : 0.08);

    if (a.kind === 'rect') {
      ctx.strokeRect(a.x, a.y, a.w, a.h);
    } else if (a.kind === 'highlight') {
      ctx.fillStyle = hexToRgba(color, preview ? 0.22 : 0.35);
      ctx.fillRect(a.x, a.y, a.w, a.h);
    } else if (a.kind === 'ellipse') {
      const cx = a.x + a.w / 2;
      const cy = a.y + a.h / 2;
      ctx.beginPath();
      ctx.ellipse(cx, cy, Math.abs(a.w) / 2, Math.abs(a.h) / 2, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    } else if (a.kind === 'step') {
      const r = a.r || 18;
      ctx.save();
      ctx.fillStyle = color;
      ctx.strokeStyle = '#111';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(a.x, a.y, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = '#111';
      ctx.font = `600 ${Math.round(r * 0.95)}px Inter, system-ui, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(a.n || 1), a.x, a.y + 1);
      ctx.restore();
    } else if (a.kind === 'arrow') {
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(a.x + a.w, a.y + a.h);
      ctx.stroke();
      const angle = Math.atan2(a.h, a.w);
      const headLen = 10;
      ctx.beginPath();
      ctx.moveTo(a.x + a.w, a.y + a.h);
      ctx.lineTo(a.x + a.w - headLen * Math.cos(angle - 0.4), a.y + a.h - headLen * Math.sin(angle - 0.4));
      ctx.moveTo(a.x + a.w, a.y + a.h);
      ctx.lineTo(a.x + a.w - headLen * Math.cos(angle + 0.4), a.y + a.h - headLen * Math.sin(angle + 0.4));
      ctx.stroke();
    } else if (a.kind === 'pen' && a.points && a.points.length > 1) {
      ctx.beginPath();
      a.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.stroke();
    } else if (a.kind === 'text') {
      const { size } = textMetrics(a, state);
      ctx.font = `${size}px Inter, system-ui, sans-serif`;
      ctx.fillStyle = a.color || '#ededed';
      ctx.fillText(a.text || 'Text', a.x, a.y + size - 2);
    } else if (a.kind === 'blur' || a.kind === 'crop') {
      const x = Math.min(a.x, a.x + a.w);
      const y = Math.min(a.y, a.y + a.h);
      const w = Math.abs(a.w);
      const h = Math.abs(a.h);
      ctx.save();
      const accent = a.kind === 'crop' ? 'rgba(255, 212, 59, 0.95)' : 'rgba(151, 117, 250, 0.95)';
      ctx.strokeStyle = preview ? accent : accent.replace('0.95', '0.8');
      ctx.fillStyle = a.kind === 'crop' ? 'rgba(255, 212, 59, 0.12)' : 'rgba(151, 117, 250, 0.12)';
      ctx.setLineDash([6, 4]);
      ctx.fillRect(x, y, w, h);
      ctx.strokeRect(x + 0.5, y + 0.5, Math.max(0, w - 1), Math.max(0, h - 1));
      ctx.setLineDash([]);
      ctx.restore();
    }

    if (isHovered || isSelected) {
      drawSelectionFrame(annotationBounds(a, state), isSelected);
    }
  }

  return { drawAnnotation };
}

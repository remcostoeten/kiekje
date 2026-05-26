import './style.css';
import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import {
  ChooseSaveDir,
  CopyImageToClipboard,
  CaptureRegion,
  FinishCapture,
  LoadAppState,
  OpenSaveDir,
  QuitApp,
  ResetBinds,
  SaveImage,
  UpdateSettings,
  UpdateBind,
} from '../wailsjs/go/main/App';
import { EventsOn } from '../wailsjs/runtime/runtime';

const app = document.querySelector('#app');

app.innerHTML = `
  <div class="editor" id="editor">
    <div class="stage">
      <canvas id="canvas" class="idle"></canvas>
    </div>

    <div class="bar" id="bar">
      <button class="btn primary" id="capture" type="button" data-tip="Capture" data-kbd="C">
        <i class="ti ti-camera" aria-hidden="true"></i>
        <span>Capture</span>
      </button>
      <div class="sep"></div>
      <button class="btn on" data-tool="select" type="button" data-tip="Select" data-kbd="V">
        <i class="ti ti-pointer" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="rect" type="button" data-tip="Rectangle" data-kbd="R">
        <i class="ti ti-square" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="arrow" type="button" data-tip="Arrow" data-kbd="A">
        <i class="ti ti-arrow-up-right" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="pen" type="button" data-tip="Draw" data-kbd="P">
        <i class="ti ti-pencil" aria-hidden="true"></i>
      </button>
      <button class="btn" data-tool="text" type="button" data-tip="Text" data-kbd="T">
        <i class="ti ti-letter-t" aria-hidden="true"></i>
      </button>
      <div class="color-wrap" id="color-wrap">
        <button class="btn" id="color-trigger" type="button" data-tip="Stroke color">
          <span class="color-trigger-swatch" id="color-swatch"></span>
        </button>
        <div class="color-popper hidden" id="color-popper"></div>
      </div>
      <div class="sep"></div>
      <button class="btn" id="undo" type="button" data-tip="Undo" data-kbd="Ctrl Z">
        <i class="ti ti-arrow-back-up" aria-hidden="true"></i>
      </button>
      <button class="btn done" id="save" type="button" data-tip="Save" data-kbd="Ctrl S">
        <i class="ti ti-check" aria-hidden="true"></i>
      </button>
      <button class="btn" id="copy" type="button" data-tip="Copy to clipboard" data-kbd="Ctrl C">
        <i class="ti ti-copy" aria-hidden="true"></i>
      </button>
      <div class="sep"></div>
      <div class="color-picker" id="color-picker"></div>
      <div class="menu-wrap">
        <button class="btn" id="menu-toggle" type="button" data-tip="Settings" aria-expanded="false">
          <i class="ti ti-dots-vertical" aria-hidden="true"></i>
        </button>
        <div class="menu hidden" id="menu">
          <div class="menu-section">
            <div class="menu-label">Output</div>
            <div class="path-row">
              <div class="path-text" id="save-dir"></div>
              <button class="path-icon" id="choose-save-dir" type="button" title="Choose folder">
                <i class="ti ti-folder" aria-hidden="true"></i>
              </button>
              <button class="path-icon" id="open-save-dir" type="button" title="Open folder">
                <i class="ti ti-external-link" aria-hidden="true"></i>
              </button>
            </div>
          </div>
          <div class="menu-section">
            <div class="menu-item">
              <span>Copy after capture</span>
              <div class="tog" id="tog-copy" role="switch" tabindex="0"></div>
            </div>
            <div class="menu-item">
              <span>Clipboard only</span>
              <div class="tog" id="tog-clipboard-only" role="switch" tabindex="0"></div>
            </div>
            <div class="menu-item">
              <span>Close after capture</span>
              <div class="tog" id="tog-close" role="switch" tabindex="0"></div>
            </div>
          </div>
          <div class="menu-section">
            <div class="menu-label">Shortcuts</div>
            <div class="menu-item">
              <span>Capture</span>
              <span><span class="kbd" id="bind-capture">-</span>
              <button type="button" data-record="capture">Rec</button></span>
            </div>
            <div class="menu-item">
              <span>Save</span>
              <span><span class="kbd" id="bind-save">-</span>
              <button type="button" data-record="save">Rec</button></span>
            </div>
            <div class="menu-item">
              <span>Undo</span>
              <span><span class="kbd" id="bind-undo">-</span>
              <button type="button" data-record="undo">Rec</button></span>
            </div>
          </div>
          <div class="menu-section">
            <button class="menu-action" id="reset-binds" type="button">Reset defaults</button>
          </div>
        </div>
      </div>
      <div class="tooltip" id="tooltip" aria-hidden="true"></div>
    </div>
  </div>
`;

const editor = document.getElementById('editor');
const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d');
const captureBtn = document.getElementById('capture');
const menu = document.getElementById('menu');
const menuToggle = document.getElementById('menu-toggle');
const saveDirEl = document.getElementById('save-dir');

const tooltipEl = document.getElementById('tooltip');
let tooltipTimeout = null;

function showTooltip(btn) {
  const tip = btn.dataset.tip;
  const kbd = btn.dataset.kbd;
  if (!tip) return;
  tooltipEl.innerHTML = `<span class="tt-label">${tip}</span>${kbd ? `<kbd class="tt-kbd">${kbd}</kbd>` : ''}`;
  const barRect = document.getElementById('bar').getBoundingClientRect();
  const btnRect = btn.getBoundingClientRect();
  tooltipEl.style.top = (btnRect.top - barRect.top - 8) + 'px';
  const tooltipWidth = tooltipEl.offsetWidth;
  const center = (btnRect.left + btnRect.width / 2) - barRect.left;
  tooltipEl.style.left = Math.max(4, Math.min(center - tooltipWidth / 2, barRect.width - tooltipWidth - 4)) + 'px';
  tooltipEl.classList.remove('hidden');
  tooltipEl.style.transformOrigin = `${Math.min(tooltipWidth / 2, center)}px bottom`;
  requestAnimationFrame(() => tooltipEl.classList.add('is-open'));
}

function hideTooltip() {
  tooltipEl.classList.remove('is-open');
  clearTimeout(tooltipTimeout);
  tooltipTimeout = setTimeout(() => tooltipEl.classList.add('hidden'), 150);
}

document.querySelectorAll('#bar .btn[data-tip]').forEach(btn => {
  btn.addEventListener('mouseenter', () => {
    clearTimeout(tooltipTimeout);
    showTooltip(btn);
  });
  btn.addEventListener('mouseleave', hideTooltip);
});

const toggles = {
  copy: document.getElementById('tog-copy'),
  clipboardOnly: document.getElementById('tog-clipboard-only'),
  close: document.getElementById('tog-close'),
};

const bindEls = {
  capture: document.getElementById('bind-capture'),
  save: document.getElementById('bind-save'),
  undo: document.getElementById('bind-undo'),
};

let tool = 'select';
let captureMode = true;
let image = new Image();
let annotations = [];
let current = null;
let dragStart = null;
let penPoints = [];
let state = { binds: {} };
let recordingAction = null;
let recordingMods = new Set();
let recordingKey = '';
let lastHotkeyTs = 0;
let isCapturing = false;
let captureCancelled = false;
let selectedIndices = [];
let hoveredIndex = -1;
let dragging = null;
let activeColor = '#ededed';
let inlineTextInput = null;

const COLORS = ['#ff6b6b', '#ffa94d', '#ffd43b', '#69db7c', '#4dabf7', '#9775fa', '#f783ac', '#ededed'];

function selectToolbarColor(color, swatch) {
  activeColor = color;
  document.getElementById('color-swatch').style.background = color;
  document.querySelectorAll('#color-popper .clr').forEach((b) => {
    b.classList.toggle('clr-on', b.dataset.color === color);
  });
  document.querySelectorAll('.cp-swatch').forEach((b) => {
    b.classList.toggle('cp-active', b.dataset.color === color);
  });
  if (swatch) {
    swatch.classList.remove('clr-pop');
    void swatch.offsetWidth;
    swatch.classList.add('clr-pop');
  }
}

function buildColors() {
  const el = document.getElementById('color-popper');
  el.innerHTML = COLORS.map(c =>
    `<button class="clr${c === activeColor ? ' clr-on' : ''}" data-color="${c}" style="background:${c}" title="${c}"></button>`
  ).join('');
  el.addEventListener('click', (e) => {
    const swatch = e.target.closest('[data-color]');
    if (!swatch) return;
    selectToolbarColor(swatch.dataset.color, swatch);
    el.classList.add('hidden');
  });
  document.getElementById('color-swatch').style.background = activeColor;
}

document.getElementById('color-trigger').addEventListener('click', (e) => {
  e.stopPropagation();
  const popper = document.getElementById('color-popper');
  popper.classList.toggle('hidden');
});

document.addEventListener('click', (e) => {
  const wrap = document.getElementById('color-wrap');
  if (!wrap.contains(e.target)) {
    document.getElementById('color-popper').classList.add('hidden');
  }
});

function hasImage() {
  return image.complete && image.naturalWidth > 0;
}

function setIdle(idle) {
  canvas.classList.toggle('idle', idle);
  editor.classList.toggle('editing', !idle);
  captureMode = idle;
  for (const id of ['undo', 'save', 'copy']) {
    document.getElementById(id).disabled = idle;
  }
}

function formatBindDisplay(line) {
  const parsed = parseBindLine(line);
  if (!parsed) return '-';
  const modMap = { SUPER: '⌘', SHIFT: '⇧', CTRL: '⌃', ALT: '⌥' };
  const order = ['SHIFT', 'SUPER', 'CTRL', 'ALT'];
  const mods = order.filter((m) => parsed.modifiers.has(m)).map((m) => modMap[m] || m).join('');
  const key = parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key;
  return `${mods}${key}`;
}

function shortenPath(path) {
  if (!path) return '';
  return path.replace(/^\/home\/[^/]+/, '~');
}

function redraw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (hasImage()) ctx.drawImage(image, 0, 0);
  annotations.forEach(drawAnnotation);
  if (current) drawAnnotation(current, true);
}

function hitTest(p, a) {
  const margin = 6;
  if (a.kind === 'rect') {
    const x1 = Math.min(a.x, a.x + a.w);
    const x2 = Math.max(a.x, a.x + a.w);
    const y1 = Math.min(a.y, a.y + a.h);
    const y2 = Math.max(a.y, a.y + a.h);
    return p.x >= x1 - margin && p.x <= x2 + margin && p.y >= y1 - margin && p.y <= y2 + margin;
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
    return p.x >= a.x - margin && p.x <= a.x + 120 + margin &&
           p.y >= a.y - margin && p.y <= a.y + 24 + margin;
  }
  return false;
}

function hexToRgba(hex, alpha) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

function drawAnnotation(a, preview = false) {
  const idx = annotations.indexOf(a);
  const isSelected = selectedIndices.includes(idx) && !preview;
  const isHovered = idx === hoveredIndex && !preview && tool === 'select';
  ctx.lineWidth = 2;
  const color = a.color || '#ededed';
  ctx.strokeStyle = color;
  ctx.fillStyle = hexToRgba(color, preview ? 0.05 : 0.08);
  if (a.kind === 'rect') {
    ctx.strokeRect(a.x, a.y, a.w, a.h);
    if (isSelected) {
      ctx.strokeStyle = '#7c5cfc';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.strokeRect(a.x - 3, a.y - 3, a.w + 6, a.h + 6);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    } else if (isHovered) {
      ctx.strokeStyle = 'rgba(255,255,255,0.3)';
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 4]);
      ctx.strokeRect(a.x - 2, a.y - 2, a.w + 4, a.h + 4);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
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
    if (isSelected) {
      ctx.strokeStyle = '#7c5cfc';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.strokeRect(Math.min(a.x, a.x + a.w) - 3, Math.min(a.y, a.y + a.h) - 3,
        Math.abs(a.w) + 6, Math.abs(a.h) + 6);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    } else if (isHovered) {
      ctx.strokeStyle = 'rgba(255,255,255,0.3)';
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 4]);
      ctx.strokeRect(Math.min(a.x, a.x + a.w) - 2, Math.min(a.y, a.y + a.h) - 2,
        Math.abs(a.w) + 4, Math.abs(a.h) + 4);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
  } else if (a.kind === 'pen' && a.points && a.points.length > 1) {
    ctx.beginPath();
    a.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
    ctx.stroke();
    if (isHovered || isSelected) {
      const minX = Math.min(...a.points.map(p => p.x));
      const maxX = Math.max(...a.points.map(p => p.x));
      const minY = Math.min(...a.points.map(p => p.y));
      const maxY = Math.max(...a.points.map(p => p.y));
      ctx.strokeStyle = isSelected ? '#7c5cfc' : 'rgba(255,255,255,0.3)';
      ctx.lineWidth = 1;
      ctx.setLineDash(isSelected ? [4, 4] : [2, 4]);
      ctx.strokeRect(minX - 3, minY - 3, maxX - minX + 6, maxY - minY + 6);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
  } else if (a.kind === 'text') {
    ctx.font = '18px Inter, system-ui, sans-serif';
    ctx.fillStyle = a.color || '#ededed';
    ctx.fillText(a.text || 'Text', a.x, a.y + 18);
    if (isHovered || isSelected) {
      ctx.strokeStyle = 'rgba(255,255,255,0.3)';
      if (isSelected) ctx.strokeStyle = '#7c5cfc';
      ctx.lineWidth = 1;
      ctx.setLineDash(isSelected ? [4, 4] : [2, 4]);
      ctx.strokeRect(a.x - 2, a.y - 2, 122, 24);
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
  }
}

function moveAnnotation(a, dx, dy) {
  if (a.kind === 'pen' && a.points) {
    a.points.forEach(pt => {
      pt.x += dx;
      pt.y += dy;
    });
    return;
  }
  a.x += dx;
  a.y += dy;
}

function canvasPointToEditor(p) {
  const canvasRect = canvas.getBoundingClientRect();
  const editorRect = editor.getBoundingClientRect();
  return {
    x: canvasRect.left - editorRect.left + (p.x / canvas.width) * canvasRect.width,
    y: canvasRect.top - editorRect.top + (p.y / canvas.height) * canvasRect.height,
  };
}

function commitInlineText() {
  if (!inlineTextInput) return;
  const input = inlineTextInput;
  const text = input.value.trim();
  const p = { x: Number(input.dataset.canvasX), y: Number(input.dataset.canvasY) };
  input.remove();
  inlineTextInput = null;
  if (text) {
    annotations.push({ kind: 'text', x: p.x, y: p.y, w: 0, h: 0, color: activeColor, text });
    redraw();
  }
}

function cancelInlineText() {
  if (!inlineTextInput) return;
  inlineTextInput.remove();
  inlineTextInput = null;
  redraw();
}

function openInlineText(p) {
  cancelInlineText();
  const editorPoint = canvasPointToEditor(p);
  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'inline-text-input';
  input.dataset.canvasX = String(p.x);
  input.dataset.canvasY = String(p.y);
  input.style.left = `${editorPoint.x}px`;
  input.style.top = `${editorPoint.y}px`;
  input.style.color = activeColor;
  editor.appendChild(input);
  inlineTextInput = input;
  input.addEventListener('pointerdown', (evt) => evt.stopPropagation());
  input.addEventListener('click', (evt) => evt.stopPropagation());
  requestAnimationFrame(() => {
    input.focus();
    input.setSelectionRange(input.value.length, input.value.length);
  });
  input.addEventListener('keydown', (evt) => {
    if (evt.key === 'Enter') {
      evt.preventDefault();
      commitInlineText();
    } else if (evt.key === 'Escape') {
      evt.preventDefault();
      cancelInlineText();
    }
  });
  input.addEventListener('blur', commitInlineText);
}

function resizeCanvas() {
  canvas.width = image.naturalWidth || 1;
  canvas.height = image.naturalHeight || 1;
  setIdle(!hasImage());
  redraw();
}

function setCapturing(on) {
  editor.classList.toggle('capturing', on);
}

function positionMenu() {
  const gap = 8;
  const margin = 8;
  const wasHidden = menu.classList.contains('hidden');

  if (wasHidden) {
    menu.classList.remove('hidden');
    menu.style.visibility = 'hidden';
  }

  menu.classList.remove('menu-above', 'menu-below', 'menu-align-left', 'menu-align-right');
  menu.classList.add('menu-above', 'menu-align-right');

  const toggleRect = menuToggle.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const menuHeight = menuRect.height;
  const menuWidth = menuRect.width;

  const spaceAbove = toggleRect.top - margin;
  const spaceBelow = window.innerHeight - toggleRect.bottom - margin;
  const fitsAbove = spaceAbove >= menuHeight + gap;
  const fitsBelow = spaceBelow >= menuHeight + gap;

  let vertical = 'menu-below';
  if (fitsAbove && (!fitsBelow || spaceAbove >= spaceBelow)) {
    vertical = 'menu-above';
  }

  const overflowRight = toggleRect.right - menuWidth < margin;
  const overflowLeft = toggleRect.left + menuWidth > window.innerWidth - margin;

  let horizontal = 'menu-align-right';
  if (overflowRight && !overflowLeft) {
    horizontal = 'menu-align-left';
  } else if (overflowLeft && !overflowRight) {
    horizontal = 'menu-align-right';
  } else if (overflowRight && overflowLeft) {
    horizontal =
      toggleRect.left + menuWidth / 2 <= window.innerWidth / 2
        ? 'menu-align-left'
        : 'menu-align-right';
  }

  menu.classList.remove('menu-above', 'menu-below', 'menu-align-left', 'menu-align-right');
  menu.classList.add(vertical, horizontal);

  if (wasHidden) {
    menu.classList.add('hidden');
    menu.style.visibility = '';
  }
}

function setMenuOpen(open) {
  if (open) positionMenu();
  menu.classList.toggle('hidden', !open);
  menuToggle.classList.toggle('on', open);
  menuToggle.setAttribute('aria-expanded', String(open));
}

function setToggle(el, on) {
  el.classList.toggle('on', on);
}

function getToggle(el) {
  return el.classList.contains('on');
}

function renderBinds(binds = {}) {
  state.binds = binds;
  for (const [action, el] of Object.entries(bindEls)) {
    el.textContent = formatBindDisplay(binds[action]);
    el.classList.remove('recording');
  }
}

function renderSettings(nextState = {}) {
  state = { ...state, ...nextState };
  saveDirEl.textContent = shortenPath(nextState.saveDir || '');
  saveDirEl.title = nextState.saveDir || '';
  setToggle(toggles.copy, Boolean(nextState.copyAfterCapture));
  setToggle(toggles.clipboardOnly, Boolean(nextState.clipboardOnlyCapture));
  setToggle(toggles.close, Boolean(nextState.closeAfterCapture));
}

function parseBindLine(line) {
  if (!line) return null;
  const raw = line.replace(/^bind\d*\s*=\s*/i, '');
  const parts = raw.split(',').map((p) => p.trim()).filter(Boolean);
  if (parts.length < 2) return null;
  const modifiers = new Set(parts[0].split(/\s+/).map((m) => m.toUpperCase()).filter(Boolean));
  return { modifiers, key: parts[1].toUpperCase() };
}

function matchBind(line, evt) {
  const parsed = parseBindLine(line);
  if (!parsed) return false;
  const mods = new Set();
  if (evt.metaKey) mods.add('SUPER');
  if (evt.ctrlKey) mods.add('CTRL');
  if (evt.altKey) mods.add('ALT');
  if (evt.shiftKey) mods.add('SHIFT');
  if (mods.size !== parsed.modifiers.size) return false;
  for (const mod of parsed.modifiers) {
    if (!mods.has(mod)) return false;
  }
  return evt.key.toUpperCase() === parsed.key;
}

function formatRecordingPreview() {
  const modMap = { SUPER: '⌘', SHIFT: '⇧', CTRL: '⌃', ALT: '⌥' };
  const order = ['SHIFT', 'SUPER', 'CTRL', 'ALT'];
  const mods = order.filter((m) => recordingMods.has(m)).map((m) => modMap[m] || m).join('');
  const key = recordingKey ? recordingKey.toUpperCase() : '';
  return mods + key || '…';
}

function formatRecorderLine() {
  const order = ['SUPER', 'CTRL', 'ALT', 'SHIFT'];
  const mods = order.filter((m) => recordingMods.has(m)).join(' ');
  const commands = {
    capture: 'exec, cheese-wails --capture',
    save: 'save',
    undo: 'undo',
  };
  return `bind = ${mods}${mods ? ', ' : ''}${recordingKey}, ${commands[recordingAction] || ''}`;
}

function setRecording(action) {
  recordingAction = action;
  recordingMods = new Set();
  recordingKey = '';
  document.querySelectorAll('[data-record]').forEach((btn) => {
    btn.classList.toggle('recording', btn.dataset.record === action);
  });
  bindEls[action].textContent = '…';
}

async function persistRecording() {
  if (!recordingAction || !recordingKey) return;
  const updated = await UpdateBind(recordingAction, formatRecorderLine());
  recordingAction = null;
  document.querySelectorAll('[data-record]').forEach((b) => b.classList.remove('recording'));
  renderBinds(updated.binds || {});
}

function cancelRecording() {
  recordingAction = null;
  document.querySelectorAll('[data-record]').forEach((b) => b.classList.remove('recording'));
  renderBinds(state.binds || {});
}

async function loadState() {
  const loaded = await LoadAppState();
  state = loaded;
  renderBinds(loaded.binds || {});
  renderSettings(loaded);
}

function runAction(action) {
  if (action === 'capture') startCapture();
  else if (action === 'save') document.getElementById('save').click();
  else if (action === 'undo') document.getElementById('undo').click();
}

async function startCapture() {
  if (isCapturing) return;
  captureCancelled = false;
  isCapturing = true;
  setCapturing(true);
  setMenuOpen(false);
  captureBtn.disabled = true;
  try {
    window.runtime.WindowHide();
    const res = await CaptureRegion();
    if (!res?.data) throw new Error('No image');

    if (state.clipboardOnlyCapture) {
      await CopyImageToClipboard(res.path);
      window.runtime.WindowHide();
      return;
    }

    if (state.copyAfterCapture) await CopyImageToClipboard(res.path);
    if (state.closeAfterCapture) {
      await saveImageData(res.data, state.outputPath || '');
      window.runtime.WindowHide();
      image = new Image();
      setIdle(true);
      return;
    }

    annotations = [];
    current = null;
    dragStart = null;
    penPoints = [];
    image = new Image();
    image.onload = () => {
      resizeCanvas();
      window.runtime.WindowShow();
      window.runtime.WindowCenter();
    };
    image.src = `data:image/png;base64,${res.data}`;
    captureMode = false;
  } catch (err) {
    console.error(err);
    if (!captureCancelled && !String(err).includes('cancelled')) {
      window.runtime.WindowShow();
      window.runtime.WindowCenter();
    }
  } finally {
    isCapturing = false;
    setCapturing(false);
    captureBtn.disabled = false;
    if (!captureCancelled) await FinishCapture();
  }
}

function pointerPos(evt) {
  const rect = canvas.getBoundingClientRect();
  const sx = canvas.width / rect.width;
  const sy = canvas.height / rect.height;
  return { x: (evt.clientX - rect.left) * sx, y: (evt.clientY - rect.top) * sy };
}

document.querySelectorAll('[data-tool]').forEach((btn) => {
  btn.addEventListener('click', () => {
    setTool(btn.dataset.tool);
  });
});

canvas.addEventListener('pointerdown', (evt) => {
  if (captureMode || !hasImage()) return;
  if (inlineTextInput && evt.target !== inlineTextInput) commitInlineText();
  const p = pointerPos(evt);

  if (tool === 'select') {
    let hit = -1;
    for (let i = annotations.length - 1; i >= 0; i--) {
      if (hitTest(p, annotations[i])) { hit = i; break; }
    }
    if (evt.ctrlKey || evt.metaKey) {
      if (hit >= 0) {
        const idx = selectedIndices.indexOf(hit);
        if (idx >= 0) selectedIndices.splice(idx, 1);
        else selectedIndices.push(hit);
      }
    } else {
      if (hit >= 0 && selectedIndices.length === 1 && selectedIndices[0] === hit) {
      } else {
        selectedIndices = hit >= 0 ? [hit] : [];
      }
    }
    if (hit >= 0 && selectedIndices.includes(hit)) {
      dragging = { last: p };
    }
    canvas.setPointerCapture(evt.pointerId);
    redraw();
    return;
  }

  if (tool === 'text') {
    evt.preventDefault();
    evt.stopPropagation();
    openInlineText(p);
    return;
  }

  dragStart = p;
  if (tool === 'pen') {
    penPoints = [dragStart];
    current = { kind: 'pen', color: activeColor, points: penPoints };
  } else {
    current = {
      kind: tool,
      x: dragStart.x,
      y: dragStart.y,
      w: 0,
      h: 0,
      color: activeColor,
      text: '',
    };
  }
  canvas.setPointerCapture(evt.pointerId);
  return;
});

canvas.addEventListener('pointermove', (evt) => {
  if (captureMode || !hasImage()) return;
  const p = pointerPos(evt);

  if (tool === 'select' && !dragging) {
    let hit = -1;
    for (let i = annotations.length - 1; i >= 0; i--) {
      if (hitTest(p, annotations[i])) { hit = i; break; }
    }
    if (hit !== hoveredIndex) {
      hoveredIndex = hit;
      canvas.style.cursor = hit >= 0 ? 'grab' : '';
      redraw();
    }
  } else if (!dragging) {
    hoveredIndex = -1;
    canvas.style.cursor = tool === 'text' ? 'text' : 'crosshair';
  }

  if (dragging) {
    canvas.style.cursor = 'grabbing';
    const dx = p.x - dragging.last.x;
    const dy = p.y - dragging.last.y;
    selectedIndices.forEach(i => {
      if (i >= 0 && i < annotations.length) moveAnnotation(annotations[i], dx, dy);
    });
    dragging.last = p;
    redraw();
    return;
  }

  if (!dragStart || !current || tool === 'text') return;
  if (tool === 'pen') penPoints.push(p);
  else {
    current.w = p.x - dragStart.x;
    current.h = p.y - dragStart.y;
  }
  redraw();
});

canvas.addEventListener('pointerup', () => {
  if (dragging) {
    dragging = null;
    canvas.style.cursor = 'default';
    redraw();
    return;
  }
  if (!current) return;
  annotations.push(current);
  current = null;
  dragStart = null;
  redraw();
});

function setTool(name) {
  commitInlineText();
  tool = name;
  document.querySelectorAll('[data-tool]').forEach(b => b.classList.remove('on'));
  const btn = document.querySelector(`[data-tool="${name}"]`);
  if (btn) btn.classList.add('on');
  selectedIndices = [];
  hoveredIndex = -1;
  canvas.style.cursor = name === 'select' ? 'default' : name === 'text' ? 'text' : 'crosshair';
}

async function copyToClipboardAndQuit() {
  if (!hasImage()) return;
  const blob = await new Promise(r => canvas.toBlob(r, 'image/png'));
  const reader = new FileReader();
  reader.onloadend = async () => {
    const tmp = await saveImageData(reader.result, state.outputPath || '');
    if (tmp.lastSavedPath) await CopyImageToClipboard(tmp.lastSavedPath);
    await QuitApp();
  };
  reader.readAsDataURL(blob);
}

captureBtn.onclick = () => startCapture();
document.getElementById('undo').onclick = () => {
  annotations.pop();
  redraw();
};
document.getElementById('save').onclick = async () => {
  if (!hasImage()) return;
  await saveCanvas();
};
document.getElementById('copy').onclick = async () => {
  if (!hasImage()) return;
  const blob = await new Promise((r) => canvas.toBlob(r, 'image/png'));
  const reader = new FileReader();
  reader.onloadend = async () => {
    const tmp = await saveImageData(reader.result, state.outputPath || '');
    if (tmp.lastSavedPath) await CopyImageToClipboard(tmp.lastSavedPath);
  };
  reader.readAsDataURL(blob);
};

async function saveCanvas() {
  const blob = await new Promise((r) => canvas.toBlob(r, 'image/png'));
  const dataUrl = await new Promise((resolve) => {
    const reader = new FileReader();
    reader.onloadend = () => resolve(reader.result);
    reader.readAsDataURL(blob);
  });
  return saveImageData(dataUrl, state.outputPath || '');
}

async function saveImageData(data, outputPath) {
  await SaveImage(data, outputPath);
  const loaded = await LoadAppState();
  renderSettings(loaded);
  return loaded;
}

async function persistSettings() {
  const updated = await UpdateSettings(
    state.saveDir || '',
    getToggle(toggles.copy),
    getToggle(toggles.close),
    getToggle(toggles.clipboardOnly),
  );
  renderSettings(updated);
}

function wireToggle(el) {
  const flip = () => {
    setToggle(el, !getToggle(el));
    persistSettings();
  };
  el.addEventListener('click', flip);
  el.addEventListener('keydown', (e) => {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      flip();
    }
  });
}

wireToggle(toggles.copy);
wireToggle(toggles.clipboardOnly);
wireToggle(toggles.close);

document.getElementById('choose-save-dir').onclick = async () => {
  setMenuOpen(false);
  renderSettings(await ChooseSaveDir());
};
document.getElementById('open-save-dir').onclick = () => OpenSaveDir();
document.querySelectorAll('[data-record]').forEach((btn) => {
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    setRecording(btn.dataset.record);
  });
});
document.getElementById('reset-binds').onclick = async () => {
  renderBinds((await ResetBinds()).binds || {});
};

menuToggle.onclick = (e) => {
  e.stopPropagation();
  setMenuOpen(menu.classList.contains('hidden'));
};

document.addEventListener('click', () => setMenuOpen(false));
menu.addEventListener('click', (e) => e.stopPropagation());

EventsOn('cheese:capture', () => startCapture());
EventsOn('cheese:choose-save-dir', () => setMenuOpen(false));
EventsOn('cheese:cancel-capture', () => {
  captureCancelled = true;
  isCapturing = false;
  captureBtn.disabled = false;
  setCapturing(false);
});

function handleRecordingKeydown(evt) {
  if (!recordingAction) return false;
  evt.preventDefault();
  if (evt.key === 'Escape') {
    cancelRecording();
    return true;
  }
  if (evt.key === 'Enter') {
    if (recordingKey) persistRecording();
    return true;
  }
  if (evt.key === 'Backspace') {
    recordingKey = '';
    bindEls[recordingAction].textContent = '…';
    return true;
  }
  if (['Meta', 'Control', 'Alt', 'Shift'].includes(evt.key)) {
    if (evt.key === 'Meta') recordingMods.add('SUPER');
    if (evt.key === 'Control') recordingMods.add('CTRL');
    if (evt.key === 'Alt') recordingMods.add('ALT');
    if (evt.key === 'Shift') recordingMods.add('SHIFT');
    bindEls[recordingAction].textContent = formatRecordingPreview();
    return true;
  }
  recordingKey = evt.key.length === 1 ? evt.key.toUpperCase() : evt.key.toUpperCase();
  bindEls[recordingAction].textContent = formatRecordingPreview();
  return true;
}

window.addEventListener('keydown', (evt) => {
  if (handleRecordingKeydown(evt)) return;
  if (Date.now() - lastHotkeyTs < 120) return;
  for (const action of ['capture', 'save', 'undo']) {
    if (matchBind(state.binds?.[action], evt)) {
      evt.preventDefault();
      lastHotkeyTs = Date.now();
      runAction(action);
      return;
    }
  }
  if (evt.key === 'Escape') {
    setMenuOpen(false);
    closeColorPicker();
    setTool('select');
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
    copyToClipboardAndQuit();
    return;
  }
  if ((evt.metaKey || evt.ctrlKey) && evt.key.toLowerCase() === 'v' && !evt.target.closest('input,textarea')) {
    if (window.copyBuffer && window.copyBuffer.length > 0) {
      evt.preventDefault();
      window.copyBuffer.forEach(a => {
        const copy = JSON.parse(JSON.stringify(a));
        const off = 15;
        if (copy.kind === 'pen' && copy.points) copy.points.forEach(p => { p.x += off; p.y += off; });
        else { copy.x += off; copy.y += off; }
        annotations.push(copy);
      });
      selectedIndices = [];
      redraw();
    }
    return;
  }
  if (evt.target.closest('input,textarea')) return;
  if (evt.key === 'v') { setTool('select'); return; }
  if (evt.key === 'r') { setTool('rect'); return; }
  if (evt.key === 'a') { setTool('arrow'); return; }
  if (evt.key === 'p') { setTool('pen'); return; }
  if (evt.key === 't') { setTool('text'); return; }
  if (evt.key === 'c' && !evt.ctrlKey && !evt.metaKey) { startCapture(); return; }
  if (evt.key === 'q') { QuitApp(); return; }
});

/* ─── Color Picker ─── */

const colorPicker = document.getElementById('color-picker');

function closeColorPicker() {
  colorPicker.classList.remove('is-open');
}

function openColorPicker(clientX, clientY, currentColor) {
  colorPicker.innerHTML = COLORS.map(c =>
    `<button class="cp-swatch${c === currentColor ? ' cp-active' : ''}" data-color="${c}" style="background:${c}"></button>`
  ).join('');
  const editorRect = editor.getBoundingClientRect();
  const pickerWidth = 232;
  const pickerHeight = 42;
  const margin = 8;
  const x = Math.max(margin, Math.min(clientX - editorRect.left, editorRect.width - pickerWidth - margin));
  const y = Math.max(margin, Math.min(clientY - editorRect.top, editorRect.height - pickerHeight - margin));
  colorPicker.style.left = x + 'px';
  colorPicker.style.top = y + 'px';
  closeColorPicker();
  requestAnimationFrame(() => {
    colorPicker.classList.add('is-open');
  });
}

colorPicker.addEventListener('click', (e) => {
  const swatch = e.target.closest('[data-color]');
  if (!swatch) return;
  const color = swatch.dataset.color;
  selectToolbarColor(color, document.querySelector(`#color-popper .clr[data-color="${color}"]`));
  selectedIndices.forEach(i => { if (i >= 0 && i < annotations.length) annotations[i].color = color; });
  closeColorPicker();
  redraw();
});

canvas.addEventListener('contextmenu', (evt) => {
  if (captureMode || !hasImage()) return;
  evt.preventDefault();
  const p = pointerPos(evt);
  let hit = -1;
  for (let i = annotations.length - 1; i >= 0; i--) {
    if (hitTest(p, annotations[i])) { hit = i; break; }
  }
  if (hit >= 0) {
    selectedIndices = [hit];
    openColorPicker(evt.clientX, evt.clientY, annotations[hit].color);
  } else {
    selectedIndices = [];
    openColorPicker(evt.clientX, evt.clientY, activeColor);
  }
  redraw();
});

document.addEventListener('click', (e) => {
  if (!colorPicker.contains(e.target)) {
    closeColorPicker();
  }
});

document.addEventListener('keydown', (evt) => {
  if (recordingAction) return;
  if ((evt.key === 'Delete' || evt.key === 'Backspace') && tool === 'select' && !evt.target.closest('input,textarea')) {
    if (selectedIndices.length > 0) {
      evt.preventDefault();
      selectedIndices.sort((a, b) => b - a).forEach(i => annotations.splice(i, 1));
      selectedIndices = [];
      redraw();
    }
  }
});

buildColors();

loadState().then(() => {
  setCapturing(false);
  setIdle(true);
  redraw();
});

document.addEventListener('visibilitychange', () => {
  if (!document.hidden && !isCapturing) setCapturing(false);
});

window.addEventListener('resize', () => {
  if (!menu.classList.contains('hidden')) positionMenu();
});

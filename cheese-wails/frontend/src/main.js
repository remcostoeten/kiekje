import './style.css';
import {
  CaptureRegion,
  LoadAppState,
  ResetBinds,
  SaveImage,
  UpdateBind,
} from '../wailsjs/go/main/App';

const app = document.querySelector('#app');

app.innerHTML = `
  <div class="shell">
    <div class="toolbar">
      <button id="capture" class="primary">Capture</button>
      <button id="recapture" class="secondary">Re-capture</button>
      <button data-tool="select" class="active">Select</button>
      <button data-tool="rect">Rect</button>
      <button data-tool="arrow">Arrow</button>
      <button data-tool="pen">Pen</button>
      <button data-tool="text">Text</button>
      <button id="undo">Undo</button>
      <button id="save">Save PNG</button>
      <input id="output" class="output" value="/tmp/cheese-wails.png" />
    </div>

    <div id="mask" class="mask">Capture the region…</div>

    <div class="content-grid">
      <section class="workspace">
        <canvas id="canvas"></canvas>
      </section>

      <aside class="panel">
        <div class="panel-head">
          <div>
            <p class="eyebrow">Shortcuts</p>
            <h2>Recorder</h2>
          </div>
          <button id="reset-binds" class="secondary">Reset defaults</button>
        </div>

        <div class="shortcut-grid">
          <div class="shortcut-card">
            <span>Capture</span>
            <strong id="bind-capture">-</strong>
            <button data-record="capture">Record</button>
          </div>
          <div class="shortcut-card">
            <span>Re-capture</span>
            <strong id="bind-recapture">-</strong>
            <button data-record="recapture">Record</button>
          </div>
          <div class="shortcut-card">
            <span>Save</span>
            <strong id="bind-save">-</strong>
            <button data-record="save">Record</button>
          </div>
          <div class="shortcut-card">
            <span>Undo</span>
            <strong id="bind-undo">-</strong>
            <button data-record="undo">Record</button>
          </div>
        </div>

        <div class="recorder" id="recorder" tabindex="0">
          <div class="recorder-state" id="recorder-state">Click a bind row, then press keys</div>
          <div class="recorder-keyline" id="recorder-keys">Waiting for input</div>
          <div class="hint">Press Enter to save the bind. Esc cancels.</div>
        </div>

        <div class="panel-foot">
          <code id="state-path">State: ~/.cheese-wails/state.json</code>
        </div>
      </aside>
    </div>
  </div>
`;

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d');
const mask = document.getElementById('mask');
const captureBtn = document.getElementById('capture');
const outputInput = document.getElementById('output');
const recorder = document.getElementById('recorder');
const recorderState = document.getElementById('recorder-state');
const recorderKeys = document.getElementById('recorder-keys');

const bindEls = {
  capture: document.getElementById('bind-capture'),
  recapture: document.getElementById('bind-recapture'),
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

function redraw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (image.complete && image.naturalWidth) ctx.drawImage(image, 0, 0);
  annotations.forEach(drawAnnotation);
  if (current) drawAnnotation(current, true);
}

function drawAnnotation(a, preview = false) {
  ctx.lineWidth = 3;
  ctx.strokeStyle = preview ? 'rgba(255,215,0,0.95)' : 'rgba(255,90,95,0.95)';
  ctx.fillStyle = preview ? 'rgba(255,215,0,0.15)' : 'rgba(255,90,95,0.15)';
  if (a.kind === 'rect') {
    ctx.strokeRect(a.x, a.y, a.w, a.h);
  } else if (a.kind === 'arrow') {
    ctx.beginPath();
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(a.x + a.w, a.y + a.h);
    ctx.stroke();
  } else if (a.kind === 'pen') {
    if (a.points.length > 1) {
      ctx.beginPath();
      a.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.stroke();
    }
  } else if (a.kind === 'text') {
    ctx.font = '20px system-ui';
    ctx.fillStyle = 'rgba(255,255,255,0.95)';
    ctx.fillText(a.text || 'Text', a.x, a.y + 20);
  }
}

function resizeCanvas() {
  canvas.width = image.naturalWidth || 1280;
  canvas.height = image.naturalHeight || 720;
  redraw();
}

function setCaptureMask(visible, text = 'Capture the region…') {
  mask.textContent = text;
  mask.classList.toggle('hidden', !visible);
  document.querySelector('.shell').classList.toggle('capturing', visible);
}

function renderBinds(binds = {}) {
  state.binds = binds;
  bindEls.capture.textContent = binds.capture || '-';
  bindEls.recapture.textContent = binds.recapture || '-';
  bindEls.save.textContent = binds.save || '-';
  bindEls.undo.textContent = binds.undo || '-';
}

function parseBindLine(line) {
  if (!line) return null;
  const raw = line.replace(/^bind\d*\s*=\s*/i, '');
  const parts = raw.split(',').map((part) => part.trim()).filter(Boolean);
  if (parts.length < 2) return null;
  const combo = parts[0];
  const action = (parts[2] || parts[1] || '').toLowerCase();
  const command = parts.slice(3).join(', ');
  const modifiers = new Set(combo.split(/\s+/).map((item) => item.toUpperCase()).filter(Boolean));
  const key = parts[1].toUpperCase();
  return { modifiers, key, action, command };
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

function formatRecorderLine() {
  const order = ['SUPER', 'CTRL', 'ALT', 'SHIFT'];
  const mods = order.filter((m) => recordingMods.has(m));
  const prefix = mods.join(' ');
  return `bind = ${prefix}${prefix ? ', ' : ''}${recordingKey}, exec, cheese`;
}

function setRecording(action) {
  recordingAction = action;
  recordingMods = new Set();
  recordingKey = '';
  recorderState.textContent = `Recording ${action}`;
  recorderKeys.textContent = 'Press a key combo';
  recorder.focus();
}

async function persistRecording() {
  if (!recordingAction || !recordingKey) return;
  const line = formatRecorderLine();
  const updated = await UpdateBind(recordingAction, line);
  renderBinds(updated.binds || {});
  recorderState.textContent = `Saved ${recordingAction}`;
  recorderKeys.textContent = line;
  recordingAction = null;
}

async function loadState() {
  const loaded = await LoadAppState();
  renderBinds(loaded.binds || {});
  if (loaded.outputPath) outputInput.value = loaded.outputPath;
}

function runAction(action) {
  if (!action) return;
  if (action === 'capture') {
    startCapture(false);
  } else if (action === 'recapture') {
    startCapture(true);
  } else if (action === 'save') {
    document.getElementById('save').click();
  } else if (action === 'undo') {
    document.getElementById('undo').click();
  }
}

async function startCapture(forceRetake = false) {
  setCaptureMask(true, forceRetake ? 'Re-capturing…' : 'Capture the region…');
  captureBtn.disabled = true;
  try {
    window.runtime.WindowHide();
    const res = await CaptureRegion();
    if (!res || !res.data) throw new Error('No image returned');
    if (forceRetake) {
      annotations = [];
      current = null;
      dragStart = null;
      penPoints = [];
    }
    image = new Image();
    image.onload = () => {
      resizeCanvas();
      window.runtime.WindowShow();
    };
    image.src = `data:image/png;base64,${res.data}`;
    captureMode = false;
    captureBtn.textContent = 'Re-capture';
    document.getElementById('recapture').textContent = 'Re-capture';
    setCaptureMask(false);
  } catch (err) {
    console.error(err);
    setCaptureMask(true, 'Capture failed');
    window.runtime.WindowShow();
  } finally {
    captureBtn.disabled = false;
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
    tool = btn.dataset.tool;
    document.querySelectorAll('[data-tool]').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
  });
});

canvas.addEventListener('pointerdown', (evt) => {
  if (captureMode || tool === 'select') return;
  dragStart = pointerPos(evt);
  if (tool === 'pen') {
    penPoints = [dragStart];
    current = { kind: 'pen', points: penPoints };
  } else {
    current = {
      kind: tool,
      x: dragStart.x,
      y: dragStart.y,
      w: 0,
      h: 0,
      text: tool === 'text' ? prompt('Text label:') || '' : '',
    };
  }
  canvas.setPointerCapture(evt.pointerId);
});

canvas.addEventListener('pointermove', (evt) => {
  if (captureMode || !dragStart || !current || tool === 'text') return;
  const p = pointerPos(evt);
  if (tool === 'pen') penPoints.push(p);
  else {
    current.w = p.x - dragStart.x;
    current.h = p.y - dragStart.y;
  }
  redraw();
});

canvas.addEventListener('pointerup', () => {
  if (captureMode || !current) return;
  annotations.push(current);
  current = null;
  dragStart = null;
  redraw();
});

document.getElementById('capture').onclick = startCapture;
document.getElementById('recapture').onclick = () => startCapture(true);
document.getElementById('undo').onclick = () => {
  annotations.pop();
  redraw();
};
document.getElementById('save').onclick = async () => {
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
  const dataUrl = await new Promise((resolve) => {
    const reader = new FileReader();
    reader.onloadend = () => resolve(reader.result);
    reader.readAsDataURL(blob);
  });
  await SaveImage(dataUrl, outputInput.value);
  alert(`Saved to ${outputInput.value}`);
};

document.querySelectorAll('[data-record]').forEach((btn) => {
  btn.addEventListener('click', () => setRecording(btn.dataset.record));
});

document.getElementById('reset-binds').onclick = async () => {
  const defaults = await ResetBinds();
  renderBinds(defaults.binds || {});
};

recorder.addEventListener('keydown', async (evt) => {
  if (!recordingAction) return;
  evt.preventDefault();
  evt.stopPropagation();

  if (evt.key === 'Escape') {
    recordingAction = null;
    recorderState.textContent = 'Recording cancelled';
    recorderKeys.textContent = 'Waiting for input';
    return;
  }

  if (evt.key === 'Enter') {
    await persistRecording();
    return;
  }

  if (evt.key === 'Backspace') {
    recordingKey = '';
    recorderKeys.textContent = 'Press a key combo';
    return;
  }

  if (evt.key === 'Meta' || evt.key === 'Control' || evt.key === 'Alt' || evt.key === 'Shift') {
    if (evt.key === 'Meta') recordingMods.add('SUPER');
    if (evt.key === 'Control') recordingMods.add('CTRL');
    if (evt.key === 'Alt') recordingMods.add('ALT');
    if (evt.key === 'Shift') recordingMods.add('SHIFT');
    recorderKeys.textContent = formatRecorderLine();
    return;
  }

  recordingKey = evt.key.length === 1 ? evt.key.toUpperCase() : evt.key.toUpperCase();
  recorderKeys.textContent = formatRecorderLine();
});

window.addEventListener('keydown', (evt) => {
  if (recordingAction) return;
  if (Date.now() - lastHotkeyTs < 120) return;
  const binds = state.binds || {};
  const orderedActions = ['recapture', 'capture', 'save', 'undo'];
  for (const action of orderedActions) {
    const line = binds[action];
    if (matchBind(line, evt)) {
      evt.preventDefault();
      lastHotkeyTs = Date.now();
      runAction(action);
      return;
    }
  }
  if (evt.key === 'Escape') {
    tool = 'select';
    document.querySelectorAll('[data-tool]').forEach((b) => b.classList.remove('active'));
    document.querySelector('[data-tool="select"]').classList.add('active');
  }
  if (evt.metaKey || evt.ctrlKey) {
    if (evt.key.toLowerCase() === 's') {
      evt.preventDefault();
      document.getElementById('save').click();
    }
    if (evt.key.toLowerCase() === 'z') {
      evt.preventDefault();
      document.getElementById('undo').click();
    }
  }
});

async function init() {
  await loadState();
  setCaptureMask(true, 'Capture the region…');
  redraw();
  startCapture(false);
}

init();

import './style.css';
import { CaptureRegion, SaveImage } from '../wailsjs/go/main/App';

const app = document.querySelector('#app');

app.innerHTML = `
  <div class="shell">
    <div class="toolbar">
      <button id="capture" class="primary">Capture</button>
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
    <div class="workspace">
      <canvas id="canvas"></canvas>
    </div>
  </div>
`;

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d');
const mask = document.getElementById('mask');
const captureBtn = document.getElementById('capture');
const outputInput = document.getElementById('output');

let tool = 'select';
let captureMode = true;
let image = new Image();
let annotations = [];
let current = null;
let dragStart = null;
let penPoints = [];

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
}

async function startCapture() {
  setCaptureMask(true, 'Capture the region…');
  captureBtn.disabled = true;
  try {
    const res = await CaptureRegion();
    if (!res || !res.data) {
      throw new Error('No image returned');
    }
    image = new Image();
    image.onload = resizeCanvas;
    image.src = `data:image/png;base64,${res.data}`;
    captureMode = false;
    captureBtn.textContent = 'Re-capture';
    setCaptureMask(false);
  } catch (err) {
    console.error(err);
    setCaptureMask(true, 'Capture failed');
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
  if (tool === 'pen') {
    penPoints.push(p);
  } else {
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

window.addEventListener('keydown', (evt) => {
  if (evt.key === 'Escape') {
    tool = 'select';
    document.querySelectorAll('[data-tool]').forEach((b) => b.classList.remove('active'));
    document.querySelector('[data-tool="select"]').classList.add('active');
  }
});

setCaptureMask(true, 'Capture the region…');
redraw();
startCapture();

import { EDITOR_ICONS, type EditorIcon } from "./editor-icons";

// Kiekje capture showcase: one canvas, one rAF timeline, every frame = render(t).
// Heavy content is pre-rendered into sprites on load/resize; the loop only blits.

let W = 1920;
const H = 1200;
const MODE_T0 = 3.35;
const MODE_D = 1.8;
const ANNO_S = 5.95;
const ANNO_T0 = ANNO_S + MODE_D;
const ANNO_D = 4.3;
const LOOP = 12.1 + MODE_D + ANNO_D;
const TAKE = LOOP - 0.3;
const FINAL_STILL = 7.6 + MODE_D + ANNO_D;

const C = {
  bg: "#090908",
  deep: "#050504",
  surface: "#111110",
  raised: "#161615",
  raisedHi: "#1e1e1c",
  stroke: "#1b1b19",
  strokeHi: "#252523",
  fg: "#efeeea",
  muted: "#a3a099",
  faint: "#6b6962",
  accent: "#fe5101",
  onAccent: "#ffffff",
  ok: "#5cc48e",
};
function GEIST(s: number, w = 400) {
  return `${w} ${s}px Geist, sans-serif`;
}
function GMONO(s: number) {
  return `400 ${s}px "Geist Mono", monospace`;
}
const SYSMONO = `400 14px "DejaVu Sans Mono", "Ubuntu Mono", "Liberation Mono", ui-monospace, Menlo, monospace`;

// ---------- math ----------
type P = [number, number];
type Move = { t: number; d: number; from: P; to: P; ov: P; bend: number; seed: number };
function clamp(x: number, a = 0, b = 1) {
  return Math.min(b, Math.max(a, x));
}
function lerp(a: number, b: number, u: number) {
  return a + (b - a) * u;
}
function easeInOut(u: number) {
  return u < 0.5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2;
}
function easeOut(u: number) {
  return 1 - Math.pow(1 - u, 3);
}
function prog(t: number, t0: number, d: number) {
  return clamp((t - t0) / d);
}
function mulberry32(a: number) {
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let r = Math.imul(a ^ (a >>> 15), 1 | a);
    r = (r + Math.imul(r ^ (r >>> 7), 61 | r)) ^ r;
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}

// ---------- layout constants (logical px) ----------
const BAR = 34;
const GAP = 8;
const WIN_Y = BAR + GAP;
const WIN_H = H - WIN_Y - GAP;
type Box = { x: number; y: number; w: number; h: number };
let ED: Box,
  TE: Box,
  TRAY: P,
  POP: Box,
  ROW_TARGET: P,
  P0: P,
  NOTE: Box,
  MOVES: Move[] = [];
const ROW0_Y = 78;
const SEL0: P = [0, 0];
const SEL1: P = [0, 0];
const CODE_X = 392;
const CODE_TOP = 90;
const LH = 22;

// ---------- inline editor ----------
const T_ARROW_TOOL = 6.7 + MODE_D;
const T_DRAW0 = 7.4 + MODE_D;
const T_DRAW1 = 7.95 + MODE_D;
const T_STEP_TOOL = 8.55 + MODE_D;
const T_STEP1 = 9.05 + MODE_D;
const T_STEP2 = 9.5 + MODE_D;
const T_ACCEPT = 10.1 + MODE_D;
const ARROW_A: P = [0, 0];
const ARROW_B: P = [0, 0];
const STEP1: P = [0, 0];
const STEP2: P = [0, 0];
const CHIP = 30;
const CHIP_GAP = 2;
const BAR_PAD = 6;
const BAR_GAP = 10;
const TOOL_ICONS: EditorIcon[] = [
  "select",
  "rectangle",
  "ellipse",
  "arrow",
  "line",
  "pen",
  "highlight",
  "step",
  "text",
  "check",
  "cross",
  "emoji",
  "pixelate",
  "region",
];
const HISTORY_ICONS: EditorIcon[] = ["undo", "redo", "clear"];
const SWATCHES = ["#fe5101", "#e52b2b", "#ffd60a", "#2fbf71", "#2f80ed", "#ffffff", "#000000"];
const ACTIONS = [
  "Accept",
  "Cancel",
  "Save",
  "Save as",
  "Copy",
  "Record",
  "Record GIF",
  "Copy text",
  "Pin",
];
const TOOL_ROW_W = TOOL_ICONS.length * CHIP + (TOOL_ICONS.length - 1) * CHIP_GAP;
const HISTORY_W = HISTORY_ICONS.length * CHIP + (HISTORY_ICONS.length - 1) * CHIP_GAP;
const TOOLS_W = BAR_PAD * 2 + TOOL_ROW_W + 13 + HISTORY_W;
const TOOLS_H = BAR_PAD * 2 + CHIP * 2 + 13;
const MODES = [
  ["1", "Screenshot"],
  ["2", "Video"],
  ["3", "GIF"],
  ["4", "Extract text"],
];
const MODE_KEYS = [
  [MODE_T0 + 0.25, 1],
  [MODE_T0 + 0.65, 2],
  [MODE_T0 + 1.05, 3],
  [MODE_T0 + 1.5, 0],
];
const MODE_H = BAR_PAD * 2 + 30;
let MODE_X: number[] = [];
let MODE_W: number[] = [];
let MODE_CAPS: number[] = [];
let MODES_BOX: Box = { x: 0, y: 0, w: 0, h: MODE_H };
let SIZE_LABEL = "";
let TOOLS: Box = { x: 0, y: 0, w: TOOLS_W, h: TOOLS_H };
let ACTIONS_BOX: Box = { x: 0, y: SEL1[1] + BAR_GAP, w: 0, h: BAR_PAD * 2 + 28 };
let ACTION_X: number[] = [];
let ACTION_W: number[] = [];
let ACCEPT_P: P = [0, 0];
function SEL1_W() {
  return SEL1[0] - SEL0[0];
}
function SEL1_H() {
  return SEL1[1] - SEL0[1];
}
function toolCenter(i: number): P {
  return [TOOLS.x + BAR_PAD + i * (CHIP + CHIP_GAP) + CHIP / 2, TOOLS.y + BAR_PAD + CHIP / 2];
}
function placeRegion() {
  const m = document.createElement("canvas").getContext("2d")!;
  m.font = SYSMONO;
  const cw = m.measureText("M").width;
  const x0 = TE.x + 20;
  function row(i: number) {
    return Math.round(TE.y + 34 + (i + 8) * LH - 5);
  }
  SEL0.splice(0, 2, TE.x - 6, row(2) + 8);
  SEL1.splice(0, 2, Math.round(Math.min(x0 + cw * 55, TE.x + TE.w - 8)), row(9) + 18);
  ARROW_A.splice(0, 2, Math.round(Math.min(x0 + cw * 50, SEL1[0] - 24)), row(4) - 6);
  ARROW_B.splice(0, 2, Math.round(x0 + cw * 40), row(7) - 4);
  STEP1.splice(0, 2, TE.x + 18, row(3));
  STEP2.splice(0, 2, TE.x + 18, row(5));
  SIZE_LABEL = `${SEL1_W()} × ${SEL1_H()}`;
  TOOLS = {
    x: Math.round(clamp((SEL0[0] + SEL1[0]) / 2 - TOOLS_W / 2, 8, W - TOOLS_W - 8)),
    y: SEL0[1] - BAR_GAP - TOOLS_H,
    w: TOOLS_W,
    h: TOOLS_H,
  };
  m.font = GEIST(13, 500);
  let x = BAR_PAD + Math.ceil(m.measureText(SIZE_LABEL).width) + 16 + 4;
  ACTION_X = [];
  ACTION_W = [];
  for (const label of ACTIONS) {
    const w = Math.ceil(m.measureText(label).width) + 20;
    ACTION_X.push(x);
    ACTION_W.push(w);
    x += w + CHIP_GAP;
  }
  const w = x - CHIP_GAP + BAR_PAD;
  ACTIONS_BOX = {
    x: Math.round(clamp((SEL0[0] + SEL1[0]) / 2 - w / 2, 8, W - w - 8)),
    y: SEL1[1] + BAR_GAP,
    w,
    h: BAR_PAD * 2 + 28,
  };
  ACCEPT_P = [
    ACTIONS_BOX.x + (ACTION_X[0] ?? 0) + (ACTION_W[0] ?? 0) / 2,
    ACTIONS_BOX.y + ACTIONS_BOX.h / 2,
  ];
  MODE_X = [];
  MODE_W = [];
  MODE_CAPS = [];
  x = BAR_PAD;
  for (const [key, label] of MODES) {
    m.font = GEIST(11, 500);
    const cap = Math.max(20, Math.ceil(m.measureText(key ?? "").width) + 12);
    m.font = GEIST(14, 500);
    const bw = 10 + cap + 6 + Math.ceil(m.measureText(label ?? "").width) + 10;
    MODE_X.push(x);
    MODE_W.push(bw);
    MODE_CAPS.push(cap);
    x += bw + 2;
  }
  const mw = x - 2 + BAR_PAD;
  MODES_BOX = {
    x: Math.round(clamp((SEL0[0] + SEL1[0]) / 2 - mw / 2, 8, W - mw - 8)),
    y: SEL1[1] + 96,
    w: mw,
    h: MODE_H,
  };
}
function story(t: number) {
  if (t < MODE_T0) return t;
  if (t < MODE_T0 + MODE_D) return MODE_T0;
  const u = t - MODE_D;
  if (u < ANNO_S) return u;
  return u < ANNO_S + ANNO_D ? ANNO_S : u - ANNO_D;
}
function modeAt(t: number) {
  let from = 0,
    to = 0,
    at = -1;
  for (const [key, index] of MODE_KEYS) {
    if (t < (key ?? 0)) break;
    from = to;
    to = index ?? 0;
    at = key ?? 0;
  }
  return { from, to, at, u: at < 0 ? 1 : easeInOut(prog(t, at, 0.2)) };
}
function activeTool(t: number) {
  if (t < T_ARROW_TOOL) return TOOL_ICONS.indexOf("region");
  return t < T_STEP_TOOL ? TOOL_ICONS.indexOf("arrow") : TOOL_ICONS.indexOf("step");
}
function barsIn(t: number) {
  if (t < ANNO_T0 || t >= T_ACCEPT + 0.25) return 0;
  if (t < T_ACCEPT + 0.08) return easeOut(prog(t, ANNO_T0 + 0.05, 0.22));
  return 1 - easeOut(prog(t, T_ACCEPT + 0.08, 0.16));
}
function crosshair(t: number) {
  return (
    (t >= 2.95 && t < 5.4 + MODE_D) ||
    (t >= 7.12 + MODE_D && t < 8.08 + MODE_D) ||
    (t >= 8.82 + MODE_D && t < 9.72 + MODE_D)
  );
}

// ---------- timeline ----------
// The stage keeps a fixed 1200 px height; its width follows the viewport aspect
// so the whole desktop (both windows, bar edges) is visible instead of cropped.
function setStageWidth(w: number) {
  W = w;
  const usable = W - GAP * 3;
  const edW = Math.max(1106, Math.round(usable * 0.6));
  ED = { x: GAP, y: WIN_Y, w: edW, h: WIN_H };
  TE = { x: GAP * 2 + edW, y: WIN_Y, w: W - GAP * 3 - edW, h: WIN_H };
  TRAY = [W - 24, 17];
  POP = { x: W - 12 - 320, y: 40, w: 320, h: 344 };
  ROW_TARGET = [POP.x + 112, POP.y + ROW0_Y + 16];
  P0 = [TE.x + 26, 640];
  NOTE = { x: W - GAP - 360, y: BAR + 10, w: 360, h: 104 };
  placeRegion();
  const arrowTool = toolCenter(TOOL_ICONS.indexOf("arrow"));
  const stepTool = toolCenter(TOOL_ICONS.indexOf("step"));
  const idle: P = [ACCEPT_P[0] + 70, ACCEPT_P[1] + 46];
  MOVES = [
    { t: 0.6, d: 0.65, from: P0, to: TRAY, ov: [3, 2], bend: 0.14, seed: 1 },
    { t: 2.3, d: 0.26, from: TRAY, to: ROW_TARGET, ov: [-2, 3], bend: 0.1, seed: 2 },
    { t: 3.6 + MODE_D, d: 0.7, from: ROW_TARGET, to: SEL0, ov: [-3, -2], bend: 0.12, seed: 3 },
    { t: 4.5 + MODE_D, d: 0.9, from: SEL0, to: SEL1, ov: [0, 0], bend: 0.05, seed: 4 },
    { t: 6.15 + MODE_D, d: 0.5, from: SEL1, to: arrowTool, ov: [-2, -2], bend: 0.12, seed: 5 },
    { t: 6.85 + MODE_D, d: 0.45, from: arrowTool, to: ARROW_A, ov: [2, -2], bend: 0.1, seed: 6 },
    { t: T_DRAW0, d: 0.55, from: ARROW_A, to: ARROW_B, ov: [0, 0], bend: 0.04, seed: 7 },
    { t: 8.05 + MODE_D, d: 0.45, from: ARROW_B, to: stepTool, ov: [2, -2], bend: 0.12, seed: 8 },
    { t: 8.65 + MODE_D, d: 0.38, from: stepTool, to: STEP1, ov: [-2, 2], bend: 0.1, seed: 9 },
    { t: 9.15 + MODE_D, d: 0.3, from: STEP1, to: STEP2, ov: [1, 2], bend: 0.08, seed: 10 },
    { t: 9.6 + MODE_D, d: 0.45, from: STEP2, to: ACCEPT_P, ov: [-2, 2], bend: 0.12, seed: 11 },
    {
      t: 8.4 + MODE_D + ANNO_D,
      d: 0.45,
      from: ACCEPT_P,
      to: idle,
      ov: [-2, 1],
      bend: 0.18,
      seed: 12,
    },
  ];
}
setStageWidth(1920);
const SETTLE = 0.11;
const wobbleSeeds = Array.from({ length: 12 }, (_, i) => i + 1).map((seed) => {
  const r = mulberry32(seed * 9973);
  return [r() * 6.28, r() * 6.28, 9 + r() * 6, 11 + r() * 6];
});

function moveAt(m: Move, i: number, t: number): P {
  const end: P = [m.to[0] + m.ov[0], m.to[1] + m.ov[1]];
  if (t >= m.t + m.d) {
    const s = easeOut(prog(t, m.t + m.d, SETTLE));
    return [lerp(end[0], m.to[0], s), lerp(end[1], m.to[1], s)];
  }
  const u = easeInOut(prog(t, m.t, m.d));
  const dx = end[0] - m.from[0],
    dy = end[1] - m.from[1];
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len,
    ny = dx / len;
  const b = m.bend * len;
  const c1: P = [m.from[0] + dx * 0.3 + nx * b, m.from[1] + dy * 0.3 + ny * b];
  const c2: P = [m.from[0] + dx * 0.78 + nx * b * 0.5, m.from[1] + dy * 0.78 + ny * b * 0.5];
  const v = 1 - u;
  let x =
    v * v * v * m.from[0] + 3 * v * v * u * c1[0] + 3 * v * u * u * c2[0] + u * u * u * end[0];
  let y =
    v * v * v * m.from[1] + 3 * v * v * u * c1[1] + 3 * v * u * u * c2[1] + u * u * u * end[1];
  const [p1, p2, f1, f2] = wobbleSeeds[i] as [number, number, number, number];
  const env = Math.sin(Math.PI * u) * 1.3;
  x += env * Math.sin(u * f1 + p1);
  y += env * Math.sin(u * f2 + p2);
  return [x, y];
}
function cursorAt(t: number): P {
  let pos = P0;
  MOVES.forEach((m, i) => {
    if (t >= m.t) pos = moveAt(m, i, t);
  });
  return pos;
}
// ---------- camera ----------
const ZMAX = 1.22;
const CAM_STEP = 1 / 240;
const CAM_OMEGA = 5.2;
type Shot = { t: number; z: number; drift: number; x: () => number; y: () => number };
const SHOTS: Shot[] = [
  { t: 0, z: 1, drift: 0, x: () => W / 2, y: () => H / 2 },
  { t: 0.95, z: 1.18, drift: 0.008, x: () => POP.x + 40, y: () => 290 },
  {
    t: 3.05,
    z: 1.2,
    drift: 0.01,
    x: () => MODES_BOX.x + MODES_BOX.w / 2,
    y: () => MODES_BOX.y - 60,
  },
  {
    t: 3.2 + MODE_D,
    z: 1.07,
    drift: 0.01,
    x: () => (SEL0[0] + SEL1[0]) / 2 + 60,
    y: () => H / 2 - 20,
  },
  {
    t: 4.35 + MODE_D,
    z: 1.14,
    drift: 0.01,
    x: () => (SEL0[0] + SEL1[0]) / 2,
    y: () => (SEL0[1] + SEL1[1]) / 2 + 40,
  },
  {
    t: 5.75 + MODE_D,
    z: 1.12,
    drift: 0.008,
    x: () => (SEL0[0] + SEL1[0]) / 2 + 40,
    y: () => (TOOLS.y + SEL1[1] + BAR_GAP + 40) / 2,
  },
  {
    t: 6.25 + MODE_D + ANNO_D,
    z: 1.16,
    drift: 0.008,
    x: () => NOTE.x + NOTE.w / 2 - 180,
    y: () => 320,
  },
  { t: 9.3 + MODE_D + ANNO_D, z: 1, drift: 0, x: () => W / 2, y: () => H / 2 },
];
const CAM_FIELDS = 5;
let camTable = new Float32Array(0);
let camSamples = 0;
function clampCenter(z: number, x: number, y: number, vwL: number, vhL: number): P {
  const hw = vwL / z / 2,
    hh = vhL / z / 2;
  return [hw * 2 >= W ? W / 2 : clamp(x, hw, W - hw), hh * 2 >= H ? H / 2 : clamp(y, hh, H - hh)];
}
function shotTarget(t: number, vwL: number, vhL: number) {
  let shot = SHOTS[0]!;
  for (const next of SHOTS) if (t >= next.t) shot = next;
  const z = shot.z * (1 + shot.drift * (t - shot.t));
  const fx = shot.x(),
    fy = shot.y();
  const [x, y] = clampCenter(z, fx, fy, vwL, vhL);
  return [Math.log(z), x, y, fx, fy];
}
function buildCamera(vwL: number, vhL: number) {
  camSamples = Math.ceil(LOOP / CAM_STEP) + 1;
  camTable = new Float32Array(camSamples * CAM_FIELDS);
  const start = shotTarget(0, vwL, vhL);
  const springs = start.map((value) => ({ p1: value, v1: 0, p2: value, v2: 0 }));
  const w = CAM_OMEGA,
    dt = CAM_STEP;
  for (let pass = 0; pass < 2; pass++) {
    for (let i = 0; i < camSamples; i++) {
      const target = shotTarget(i * CAM_STEP, vwL, vhL);
      springs.forEach((spring, f) => {
        const goal = target[f] ?? spring.p1;
        spring.v1 += (w * w * (goal - spring.p1) - 2 * w * spring.v1) * dt;
        spring.p1 += spring.v1 * dt;
        spring.v2 += (w * w * (spring.p1 - spring.p2) - 2 * w * spring.v2) * dt;
        spring.p2 += spring.v2 * dt;
        if (pass === 1) camTable[i * CAM_FIELDS + f] = spring.p2;
      });
    }
  }
}
function cameraAt(t: number, vwL: number, vhL: number) {
  const pos = clamp(t / CAM_STEP, 0, camSamples - 1);
  const i = Math.min(Math.floor(pos), camSamples - 2),
    u = pos - i;
  function at(f: number) {
    return lerp(camTable[i * CAM_FIELDS + f]!, camTable[(i + 1) * CAM_FIELDS + f]!, u);
  }
  const z = Math.exp(at(0));
  const [cx, cy] = clampCenter(z, at(1), at(2), vwL, vhL);
  return { z, cx, cy, fx: at(3), fy: at(4), focus: clamp((z - 1) / 0.18) };
}
function pressed(t: number) {
  if ((t >= 1.55 && t < 1.64) || (t >= 2.75 && t < 2.84)) return true;
  if (t >= 4.5 + MODE_D && t < 5.4 + MODE_D) return true;
  if (t >= T_DRAW0 - 0.05 && t < T_DRAW1) return true;
  return [T_ARROW_TOOL, T_STEP_TOOL, T_STEP1, T_STEP2, T_ACCEPT].some(
    (at) => t >= at && t < at + 0.09,
  );
}
function frozen(t: number) {
  return t >= 2.75 && t < 6.3;
}
function blinkOn(t: number) {
  return Math.floor((frozen(t) ? 2.75 : t) / 0.53) % 2 === 0;
}

// ---------- canvas helpers ----------
type Sprite = { c: HTMLCanvasElement; lw: number; lh: number; pad: number };
function makeSprite(
  lw: number,
  lh: number,
  k: number,
  pad: number,
  draw: (g: CanvasRenderingContext2D) => void,
): Sprite {
  const c = document.createElement("canvas");
  c.width = Math.ceil((lw + pad * 2) * k);
  c.height = Math.ceil((lh + pad * 2) * k);
  const g = c.getContext("2d")!;
  g.scale(k, k);
  g.translate(pad, pad);
  draw(g);
  return { c, lw, lh, pad };
}
function blit(ctx: CanvasRenderingContext2D, s: Sprite, x: number, y: number) {
  ctx.drawImage(s.c, x - s.pad, y - s.pad, s.lw + s.pad * 2, s.lh + s.pad * 2);
}
function rr(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  g.beginPath();
  g.roundRect(x, y, w, h, r);
}
function liftShadow(
  g: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  g.save();
  g.shadowColor = "rgba(0,0,0,0.7)";
  g.shadowBlur = 32;
  g.shadowOffsetY = 18;
  g.fillStyle = "#000";
  rr(g, x + 8, y + 8, w - 16, h - 16, r);
  g.fill();
  g.restore();
}
function keycap(g: CanvasRenderingContext2D, x: number, cy: number, label: string): number {
  g.font = GEIST(11, 500);
  const w = Math.max(20, Math.ceil(g.measureText(label).width) + 12);
  rr(g, x + 0.5, cy - 9.5, w - 1, 19, 4);
  g.fillStyle = C.raised;
  g.fill();
  g.strokeStyle = C.stroke;
  g.lineWidth = 1;
  g.stroke();
  g.fillStyle = C.muted;
  g.textBaseline = "middle";
  g.textAlign = "center";
  g.fillText(label, x + w / 2, cy + 0.5);
  g.textAlign = "left";
  return w;
}

// ---------- desktop content ----------
const RUST = [
  "use crate::capture::{Monitor, Rect, Scale};",
  "use crate::geometry::{LogicalPoint, PhysicalRect};",
  "",
  "/// A selection as drawn on the overlay, in global logical pixels.",
  "#[derive(Debug, Clone, Copy, PartialEq)]",
  "pub struct Selection { pub origin: LogicalPoint, pub size: (f64, f64) }",
  "",
  "/// Maps a logical selection onto every monitor it touches.",
  "pub fn map_selection(",
  "    sel: Selection,",
  "    monitors: &[Monitor],",
  ") -> Vec<(u32, PhysicalRect)> {",
  "    let mut out = Vec::with_capacity(monitors.len());",
  "    for mon in monitors {",
  "        let Some(hit) = mon.logical.intersect(&sel.rect()) else {",
  "            continue;",
  "        };",
  "        // Fractional scale: round outward so no pixel row is lost.",
  "        let s: Scale = mon.scale;",
  "        let x0 = ((hit.x - mon.logical.x) * s.0).floor() as i32;",
  "        let y0 = ((hit.y - mon.logical.y) * s.0).floor() as i32;",
  "        let x1 = ((hit.right() - mon.logical.x) * s.0).ceil() as i32;",
  "        let y1 = ((hit.bottom() - mon.logical.y) * s.0).ceil() as i32;",
  "        out.push((mon.id, PhysicalRect::from_corners(x0, y0, x1, y1)));",
  "    }",
  "    out",
  "}",
  "",
  "impl Selection {",
  "    pub fn rect(&self) -> Rect {",
  "        Rect::new(self.origin.x, self.origin.y, self.size.0, self.size.1)",
  "    }",
  "}",
  "",
  "#[cfg(test)]",
  "mod tests {",
  "    use super::*;",
  "",
  "    #[test]",
  "    fn spans_two_monitors_at_1_25() {",
  "        let mons = [Monitor::fake(0, 0.0, 1.25), Monitor::fake(1, 2048.0, 1.0)];",
  "        let out = map_selection(sel_at(1900.0, 40.0, 300.0, 200.0), &mons);",
  "        assert_eq!(out.len(), 2);",
  "        assert_eq!(out[0].1.width(), 185);",
  "    }",
  "}",
];
const KW = new Set(
  "use pub fn let mut for in else continue struct impl as mod self super crate return if match where const".split(
    " ",
  ),
);
const PRIM = new Set("u32 i32 f64 usize bool".split(" "));
const SYN = {
  text: "#d6d2c8",
  kw: "#e0866a",
  type: "#e4c07a",
  fn: "#9fc2d8",
  num: "#c9a4e0",
  str: "#a8c98a",
  com: "#6f6b62",
  attr: "#8c8778",
  punct: "#9a958a",
  mac: "#7fc4b4",
};
function tokenize(line: string): [string, string][] {
  const out: [string, string][] = [];
  const re =
    /(\/\/.*$)|(#\[.*\])|("[^"]*")|(\b\d+(?:\.\d+)?\b)|([A-Za-z_]\w*!)|([A-Za-z_]\w*)|(\s+)|(.)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(line))) {
    const s = m[0];
    if (m[1]) out.push([s, SYN.com]);
    else if (m[2]) out.push([s, SYN.attr]);
    else if (m[3]) out.push([s, SYN.str]);
    else if (m[4]) out.push([s, SYN.num]);
    else if (m[5]) out.push([s, SYN.mac]);
    else if (m[6]) {
      const next = line[re.lastIndex];
      if (KW.has(s)) out.push([s, SYN.kw]);
      else if (PRIM.has(s) || /^[A-Z]/.test(s)) out.push([s, SYN.type]);
      else if (next === "(") out.push([s, SYN.fn]);
      else out.push([s, SYN.text]);
    } else if (m[7]) out.push([s, SYN.text]);
    else out.push([s, SYN.punct]);
  }
  return out;
}
const TERM_LINES: [string, string][] = [
  ["prompt", "cargo build --release"],
  ["out", "    Finished `release` profile in 41.2s"],
  ["", ""],
  ["prompt", "git log --oneline -3"],
  ["log", "a755278 perf: shrink the release binary"],
  ["log", "00ff716 chore: release 0.1.0-alpha.12"],
  ["log", "4e29a4d fix: make the overlay mode bar stand out"],
  ["", ""],
  ["prompt", "kiekje doctor"],
  ["head", "Session"],
  ["", ""],
  ["row", "desktop|KDE, wayland"],
  ["row", "overlay|layer-shell"],
  ["row", "window capture|kwin-scripting"],
  ["row", "clipboard|Klipper"],
  ["row", "text capture|Tesseract 5.5.3"],
  ["", ""],
  ["foot", "Screenshot v2 · ScreenCast v5 · GlobalShortcuts v2"],
  ["", ""],
  ["prompt", ""],
];
let termCursor: P = [0, 0];
let monoCharW = 8.4;

function drawWallpaper(g: CanvasRenderingContext2D) {
  const lg = g.createLinearGradient(0, 0, W, H);
  lg.addColorStop(0, "#3a2f27");
  lg.addColorStop(0.5, "#221d19");
  lg.addColorStop(1, "#15120f");
  g.fillStyle = lg;
  g.fillRect(0, 0, W, H);
  const rg = g.createRadialGradient(W * 0.72, H * 0.3, 0, W * 0.72, H * 0.3, W * 0.6);
  rg.addColorStop(0, "rgba(170,110,70,0.28)");
  rg.addColorStop(1, "rgba(120,86,60,0)");
  g.fillStyle = rg;
  g.fillRect(0, 0, W, H);
  // grain
  const n = document.createElement("canvas");
  n.width = n.height = 160;
  const ng = n.getContext("2d")!;
  const id = ng.createImageData(160, 160);
  const r = mulberry32(42);
  for (let i = 0; i < id.data.length; i += 4) {
    const v = r() * 255;
    id.data[i] = id.data[i + 1] = id.data[i + 2] = v;
    id.data[i + 3] = 14;
  }
  ng.putImageData(id, 0, 0);
  g.fillStyle = g.createPattern(n, "repeat")!;
  g.fillRect(0, 0, W, H);
}
function trayGlyph(g: CanvasRenderingContext2D, cx: number, cy: number, kind: number) {
  g.fillStyle = "#cfccc4";
  g.strokeStyle = "#cfccc4";
  g.lineWidth = 1.6;
  if (kind === 0) {
    // chat bubble
    rr(g, cx - 7, cy - 6, 14, 10, 3);
    g.fill();
    g.beginPath();
    g.moveTo(cx - 3, cy + 3);
    g.lineTo(cx - 5, cy + 7);
    g.lineTo(cx + 1, cy + 3);
    g.fill();
  } else {
    // cloud
    g.beginPath();
    g.arc(cx - 3, cy + 1, 4, 0, 7);
    g.arc(cx + 2, cy - 1, 5, 0, 7);
    g.arc(cx + 5, cy + 2, 3, 0, 7);
    g.fill();
    g.fillRect(cx - 3, cy + 1, 8, 4);
  }
}
function drawBar(g: CanvasRenderingContext2D) {
  g.fillStyle = "rgba(10,10,9,0.78)";
  g.fillRect(0, 0, W, BAR);
  g.fillStyle = "rgba(255,255,255,0.04)";
  g.fillRect(0, BAR - 1, W, 1);
  g.textBaseline = "middle";
  for (let i = 0; i < 5; i++) {
    const x = 10 + i * 30;
    if (i === 1) {
      rr(g, x, 6, 24, 22, 5);
      g.fillStyle = "rgba(239,238,234,0.12)";
      g.fill();
    }
    g.font = GEIST(12, 500);
    g.fillStyle = i === 1 ? C.fg : C.faint;
    g.textAlign = "center";
    g.fillText(String(i + 1), x + 12, 17.5);
  }
  g.font = GEIST(12.5, 500);
  g.fillStyle = "#c9c6be";
  g.textAlign = "center";
  g.fillText("nvim — src/capture/mapping.rs — kiekje", W / 2, 17.5);
  g.textAlign = "right";
  g.fillStyle = "#d8d5cd";
  g.font = GEIST(12.5, 500);
  g.fillText("Mon 27 Oct  14:32", W - 96, 17.5);
  // battery
  const bx = W - 280;
  g.strokeStyle = "#cfccc4";
  g.lineWidth = 1.2;
  rr(g, bx + 0.5, 11.5, 22, 11, 2.5);
  g.stroke();
  g.fillStyle = "#cfccc4";
  g.fillRect(bx + 23, 15, 2, 4);
  g.fillRect(bx + 2.5, 13.5, 14, 7);
  g.textAlign = "left";
  g.font = GEIST(12.5, 500);
  g.fillStyle = "#d8d5cd";
  g.fillText("78 %", bx + 30, 17.5);
  // volume
  const vx = W - 320;
  g.fillStyle = "#cfccc4";
  g.beginPath();
  g.moveTo(vx, 14);
  g.lineTo(vx + 4, 14);
  g.lineTo(vx + 9, 9);
  g.lineTo(vx + 9, 25);
  g.lineTo(vx + 4, 20);
  g.lineTo(vx, 20);
  g.fill();
  g.lineWidth = 1.5;
  g.strokeStyle = "#cfccc4";
  g.beginPath();
  g.arc(vx + 10, 17, 5, -0.9, 0.9);
  g.stroke();
  g.beginPath();
  g.arc(vx + 10, 17, 9, -0.9, 0.9);
  g.stroke();
  // network (wifi)
  const nx = W - 346;
  for (let i = 0; i < 3; i++) {
    g.beginPath();
    g.arc(nx, 24, 4 + i * 4.5, -Math.PI * 0.75, -Math.PI * 0.25);
    g.stroke();
  }
  g.beginPath();
  g.arc(nx, 23.5, 1.6, 0, 7);
  g.fill();
  trayGlyph(g, W - 74, 17, 0);
  trayGlyph(g, W - 49, 17, 1);
}
function aperture(g: CanvasRenderingContext2D, cx: number, cy: number, col: string) {
  g.save();
  g.translate(cx, cy);
  g.fillStyle = col;
  g.beginPath();
  g.arc(0, 0, 7.5, 0, Math.PI * 2);
  g.arc(0, 0, 2.6, 0, Math.PI * 2, true);
  g.fill("evenodd");
  g.strokeStyle = "rgba(10,10,9,0.95)";
  g.lineWidth = 1.3;
  for (let i = 0; i < 6; i++) {
    const a = (i * Math.PI) / 3;
    g.beginPath();
    g.moveTo(Math.cos(a) * 2.6, Math.sin(a) * 2.6);
    g.lineTo(Math.cos(a + 0.9) * 7.6, Math.sin(a + 0.9) * 7.6);
    g.stroke();
  }
  g.restore();
}
function windowFrame(
  g: CanvasRenderingContext2D,
  r: { x: number; y: number; w: number; h: number },
  bg: string,
  active: boolean,
) {
  rr(g, r.x, r.y, r.w, r.h, 10);
  g.fillStyle = bg;
  g.fill();
  rr(g, r.x + 0.5, r.y + 0.5, r.w - 1, r.h - 1, 10);
  g.strokeStyle = active ? "#a8693f" : "#3a3631";
  g.lineWidth = 1;
  g.stroke();
}
function drawEditor(g: CanvasRenderingContext2D) {
  windowFrame(g, ED, "#100f0e", true);
  g.save();
  rr(g, ED.x + 1, ED.y + 1, ED.w - 2, ED.h - 2, 9.5);
  g.clip();
  // sidebar
  g.fillStyle = "#0c0b0a";
  g.fillRect(ED.x, ED.y, 300, ED.h);
  g.fillStyle = "#1f1d1b";
  g.fillRect(ED.x + 300, ED.y, 1, ED.h);
  g.font = SYSMONO;
  g.textBaseline = "alphabetic";
  const tree: [number, string, number][] = [
    [0, "kiekje", 1],
    [1, ".github", 0],
    [1, "assets", 0],
    [1, "src", 1],
    [2, "capture", 1],
    [3, "mapping.rs", 2],
    [3, "monitor.rs", 0],
    [3, "portal.rs", 0],
    [3, "kwin.rs", 0],
    [3, "mod.rs", 0],
    [2, "overlay", 0],
    [2, "tray", 0],
    [2, "geometry.rs", 0],
    [2, "clipboard.rs", 0],
    [2, "main.rs", 0],
    [1, "tests", 0],
    [1, "Cargo.lock", 0],
    [1, "Cargo.toml", 0],
    [1, "README.md", 0],
  ];
  g.fillStyle = "#6f6b62";
  g.font = `600 11px "DejaVu Sans Mono", ui-monospace, monospace`;
  g.fillText("EXPLORER", ED.x + 18, ED.y + 26);
  g.font = SYSMONO;
  tree.forEach(([d, name, kind], i) => {
    const y = ED.y + 52 + i * 24;
    if (kind === 2) {
      g.fillStyle = "#211e1b";
      g.fillRect(ED.x, y - 16, 300, 24);
    }
    const isDir = !name.includes(".") || name === ".github";
    g.fillStyle = isDir ? "#8c8778" : kind === 2 ? "#efe9de" : "#b5b0a5";
    g.fillText((isDir ? (kind ? "▾ " : "▸ ") : "  ") + name, ED.x + 16 + d * 16, y);
  });
  // tab bar
  const cx = ED.x + 301;
  g.fillStyle = "#0c0b0a";
  g.fillRect(cx, ED.y, ED.w - 301, 36);
  g.fillStyle = "#100f0e";
  g.fillRect(cx, ED.y, 170, 36);
  g.fillStyle = C.accent;
  g.fillRect(cx, ED.y + 34, 170, 2);
  g.fillStyle = "#efe9de";
  g.fillText("mapping.rs", cx + 18, ED.y + 23);
  g.fillStyle = "#6f6b62";
  g.fillText("monitor.rs", cx + 188, ED.y + 23);
  g.fillText("portal.rs", cx + 318, ED.y + 23);
  // code
  monoCharW = g.measureText("M").width;
  RUST.forEach((line, i) => {
    const base = CODE_TOP + i * LH + 16;
    if (i === 8) {
      g.fillStyle = "#171513";
      g.fillRect(cx, CODE_TOP + i * LH, ED.w - 301, LH);
    }
    g.fillStyle = i === 8 ? "#b5b0a5" : "#4b4842";
    g.textAlign = "right";
    g.fillText(String(i + 1), CODE_X - 16, base);
    g.textAlign = "left";
    let x = CODE_X;
    for (const [s, col] of tokenize(line)) {
      g.fillStyle = col;
      g.fillText(s, x, base);
      x += s.length * monoCharW;
    }
  });
  // status line
  const sy = ED.y + ED.h - 26;
  g.fillStyle = "#0c0b0a";
  g.fillRect(ED.x, sy, ED.w, 26);
  g.fillStyle = "#8fae6f";
  g.fillRect(ED.x, sy, 82, 26);
  g.fillStyle = "#0c0b0a";
  g.font = `700 12px "DejaVu Sans Mono", ui-monospace, monospace`;
  g.fillText("NORMAL", ED.x + 16, sy + 17);
  g.font = `400 12px "DejaVu Sans Mono", ui-monospace, monospace`;
  g.fillStyle = "#b5b0a5";
  g.fillText(" main   src/capture/mapping.rs", ED.x + 96, sy + 17);
  g.textAlign = "right";
  g.fillText("rust   utf-8   rust-analyzer ✓   9:8   18%", ED.x + ED.w - 16, sy + 17);
  g.textAlign = "left";
  g.restore();
}
function drawTerminal(g: CanvasRenderingContext2D) {
  windowFrame(g, TE, "rgba(14,13,12,0.94)", false);
  g.font = SYSMONO;
  g.textBaseline = "alphabetic";
  const x0 = TE.x + 20;
  let y = TE.y + 34;
  const cw = g.measureText("M").width;
  for (const [kind, text] of TERM_LINES) {
    if (kind === "prompt") {
      let x = x0;
      const seg: [string, string][] = [
        ["~/src/kiekje", "#9fc2d8"],
        [" main", "#c9a4e0"],
        [" ❯ ", "#8fae6f"],
        [text, "#efe9de"],
      ];
      for (const [s, col] of seg) {
        g.fillStyle = col;
        g.fillText(s, x, y);
        x += g.measureText(s).width;
      }
      if (!text) termCursor = [x, y];
    } else if (kind === "head") {
      g.fillStyle = "#efe9de";
      g.fillText(text, x0, y);
    } else if (kind === "row" || kind === "rowdim") {
      const [a, b] = text.split("|");
      g.fillStyle = kind === "row" ? "#8fae6f" : "#e4c07a";
      g.fillText(kind === "row" ? "✓" : "!", x0 + cw * 2, y);
      g.fillStyle = "#b5b0a5";
      g.fillText(a ?? "", x0 + cw * 4, y);
      g.fillStyle = kind === "row" ? "#efe9de" : "#e4c07a";
      g.fillText(b ?? "", x0 + cw * 24, y);
    } else if (kind === "log") {
      const hash = text.slice(0, 7);
      g.fillStyle = "#e4c07a";
      g.fillText(hash, x0, y);
      g.fillStyle = "#b5b0a5";
      g.fillText(text.slice(7), x0 + g.measureText(hash).width, y);
    } else if (kind === "out") {
      g.fillStyle = "#8fae6f";
      g.fillText(text, x0, y);
    } else if (kind === "foot") {
      g.fillStyle = "#8c8778";
      g.fillText(text, x0, y);
    }
    y += LH;
  }
}
function drawDesktop(g: CanvasRenderingContext2D) {
  drawWallpaper(g);
  drawBar(g);
  drawEditor(g);
  drawTerminal(g);
}

// ---------- Kiekje UI ----------
function rowIcon(g: CanvasRenderingContext2D, kind: number, x: number, cy: number, col: string) {
  g.fillStyle = col;
  g.strokeStyle = col;
  g.lineWidth = 1.5;
  function L(a: number, b: number, w: number, h: number) {
    return g.fillRect(x + a, cy + b, w, h);
  }
  switch (kind) {
    case 0:
      L(0, -7, 5, 2);
      L(0, -7, 2, 5);
      L(9, -7, 5, 2);
      L(12, -7, 2, 5);
      L(0, 5, 5, 2);
      L(0, 2, 2, 5);
      L(9, 5, 5, 2);
      L(12, 2, 2, 5);
      L(5, -2, 4, 4);
      break;
    case 1:
      g.beginPath();
      g.moveTo(x + 1, cy + 7);
      g.lineTo(x + 2, cy + 3);
      g.lineTo(x + 10, cy - 5);
      g.lineTo(x + 13, cy - 2);
      g.lineTo(x + 5, cy + 6);
      g.closePath();
      g.fill();
      break;
    case 2:
      rr(g, x, cy - 6, 8, 10, 2);
      g.fill();
      rr(g, x + 9, cy - 4, 6, 8, 2);
      g.fill();
      L(2, 5, 10, 2);
      break;
    case 3:
      rr(g, x, cy - 6, 14, 12, 2.5);
      g.fill();
      g.fillStyle = C.bg;
      g.fillRect(x + 2, cy - 2, 10, 6);
      break;
    case 4:
      g.beginPath();
      g.arc(x + 7, cy, 5.5, -2.6, 2.2);
      g.stroke();
      g.beginPath();
      g.moveTo(x, cy - 6);
      g.lineTo(x + 5, cy - 6);
      g.lineTo(x + 1, cy - 1);
      g.fill();
      break;
    case 5:
      g.beginPath();
      g.arc(x + 7, cy, 6, 0, 7);
      g.fill();
      g.fillStyle = C.bg;
      g.beginPath();
      g.arc(x + 7, cy, 2.5, 0, 7);
      g.fill();
      break;
  }
}
function drawPopover(g: CanvasRenderingContext2D) {
  const { w, h } = POP;
  liftShadow(g, 0, 0, w, h, 10);
  rr(g, 0, 0, w, h, 10);
  g.fillStyle = C.bg;
  g.fill();
  rr(g, 0.5, 0.5, w - 1, h - 1, 10);
  g.strokeStyle = C.stroke;
  g.lineWidth = 1;
  g.stroke();
  g.textBaseline = "middle";
  rr(g, 6, 6, w - 12, 36, 6);
  g.fillStyle = C.surface;
  g.fill();
  g.strokeStyle = C.faint;
  g.lineWidth = 1.6;
  g.beginPath();
  g.arc(22, 23, 4.5, 0, 7);
  g.stroke();
  g.beginPath();
  g.moveTo(25.5, 26.5);
  g.lineTo(29, 30);
  g.stroke();
  g.font = GEIST(13);
  g.fillStyle = C.faint;
  g.fillText("Search actions", 38, 24.5);
  function cap(y: number, s: string) {
    g.font = GEIST(12);
    g.fillStyle = C.faint;
    g.fillText(s, 14, y);
  }
  function row(y: number, icon: number, label: string, keys: string[], sel = false) {
    if (sel) {
      rr(g, 6, y, w - 12, 32, 6);
      g.fillStyle = "rgba(254,81,1,0.14)";
      g.fill();
    }
    rowIcon(g, icon, 16, y + 16, sel ? C.fg : C.muted);
    g.font = GEIST(13, sel ? 500 : 400);
    g.fillStyle = sel ? C.fg : C.muted;
    g.fillText(label, 42, y + 16.5);
    let kx = w - 14;
    for (let i = keys.length - 1; i >= 0; i--) {
      g.font = GEIST(11, 500);
      const kw = Math.max(20, Math.ceil(g.measureText(keys[i]!).width) + 12);
      kx -= kw;
      keycap(g, kx, y + 16, keys[i]!);
      kx -= 4;
    }
  }
  cap(66, "Capture");
  row(ROW0_Y, 0, "Capture region", ["Print"], true);
  row(110, 1, "Annotate region", []);
  row(142, 2, "Capture every screen", ["Shift", "Print"]);
  row(174, 3, "Capture window", []);
  row(206, 4, "Repeat last region", []);
  cap(250, "Record");
  row(262, 5, "Record screen", []);
  g.fillStyle = C.stroke;
  g.fillRect(0, 300, w, 1);
  g.font = GEIST(13);
  g.fillStyle = C.muted;
  ["Gallery", "Settings"].forEach((s, i) => g.fillText(s, 18 + i * 82, 323));
  g.textAlign = "right";
  g.fillText("Quit", w - 18, 323);
  g.textAlign = "left";
}
type HintItem = [string[], string];
const HINT_A: HintItem[] = [
  [["Drag"], "select"],
  [["Arrows"], "move cursor"],
  [["Space"], "set corner"],
  [["F"], "screen"],
  [["Enter"], "capture"],
  [["D"], "draw"],
  [["Z"], "loupe"],
  [["C"], "colour"],
  [["?"], "hide"],
  [["Esc"], "cancel"],
];
const HINT_B: HintItem[] = [
  [["Enter"], "capture"],
  [["Arrows"], "move"],
  [["Ctrl", "Arrows"], "resize"],
  [["D"], "draw"],
  [["Esc"], "cancel"],
];
type Hint = { sprite: Sprite; x: number; enter: [number, number] };
function buildHint(items: HintItem[], k: number): Hint {
  const m = document.createElement("canvas").getContext("2d")!;
  let w = 14;
  const enter: [number, number] = [0, 0];
  for (const [keys, label] of items) {
    for (const key of keys) {
      m.font = GEIST(11, 500);
      w += Math.max(20, Math.ceil(m.measureText(key).width) + 12) + 4;
    }
    m.font = GEIST(12);
    w += 2 + m.measureText(label).width + 14;
  }
  w = Math.ceil(w);
  const h = 36;
  const sprite = makeSprite(w, h, k, 2, (g) => {
    rr(g, 0, 0, w, h, 8);
    g.fillStyle = C.deep;
    g.fill();
    rr(g, 0.5, 0.5, w - 1, h - 1, 8);
    g.strokeStyle = C.stroke;
    g.lineWidth = 1;
    g.stroke();
    let x = 14;
    for (const [keys, label] of items) {
      for (const key of keys) {
        const kw = keycap(g, x, 18, key);
        if (key === "Enter") enter.splice(0, 2, x, kw);
        x += kw + 4;
      }
      g.font = GEIST(12);
      g.fillStyle = C.muted;
      g.textBaseline = "middle";
      g.fillText(label, x + 2, 18.5);
      x += 2 + g.measureText(label).width + 14;
    }
  });
  return { sprite, x: Math.round((W - w) / 2), enter };
}
function drawNotification(g: CanvasRenderingContext2D, base: HTMLCanvasElement, bk: number) {
  const { w, h } = NOTE;
  liftShadow(g, 0, 0, w, h, 8);
  rr(g, 0, 0, w, h, 8);
  g.fillStyle = "#151413";
  g.fill();
  rr(g, 0.5, 0.5, w - 1, h - 1, 8);
  g.strokeStyle = "#2c2a27";
  g.lineWidth = 1;
  g.stroke();
  // thumbnail: real crop of the frozen frame, cover-fit into 64x64
  const sw = SEL1[0] - SEL0[0],
    sh = SEL1[1] - SEL0[1];
  const side = Math.min(sw, sh);
  const sx = SEL0[0] + (sw - side) / 2;
  g.save();
  rr(g, 14, 14, 64, 64, 5);
  g.clip();
  g.drawImage(base, sx * bk, SEL0[1] * bk, side * bk, side * bk, 14, 14, 64, 64);
  g.translate(14, 14);
  g.scale(64 / side, 64 / side);
  g.translate(-sx, -SEL0[1]);
  drawAnnotations(g, ARROW_B, 1, 1);
  g.restore();
  rr(g, 14.5, 14.5, 63, 63, 5);
  g.strokeStyle = "rgba(255,255,255,0.08)";
  g.stroke();
  g.textBaseline = "alphabetic";
  g.font = GEIST(13, 600);
  g.fillStyle = "#eeece7";
  g.fillText("Kiekje", 92, 31);
  g.font = GEIST(12);
  g.fillStyle = "#77746d";
  g.textAlign = "right";
  g.fillText("now", w - 14, 31);
  g.textAlign = "left";
  g.fillStyle = "#b9b6ae";
  g.font = GEIST(12.5);
  g.fillText("Copied and saved to ~/Pictures/Kiekje/", 92, 50);
  g.fillText("2026-10-27_14-32-18.png", 92, 66);
  g.font = GEIST(12, 500);
  [
    ["Open", 92],
    ["Annotate", 148],
  ].forEach(([s, x]) => {
    const tw = g.measureText(s as string).width + 20;
    rr(g, x as number, 76, tw, 22, 5);
    g.fillStyle = "#22201e";
    g.fill();
    g.fillStyle = "#dcd9d2";
    g.textBaseline = "middle";
    g.fillText(s as string, (x as number) + 10, 87.5);
    g.textBaseline = "alphabetic";
  });
}
function drawArrow(g: CanvasRenderingContext2D) {
  const path = new Path2D(
    "M0 0 L0 17.5 Q0 19 1.2 18 L5.2 14.2 L8.3 20.6 Q8.9 21.7 10 21.2 L11.6 20.4 Q12.6 19.9 12.1 18.8 L9.1 12.6 L14.6 12.3 Q16.1 12.2 15 11 Z",
  );
  g.save();
  g.shadowColor = "rgba(0,0,0,0.45)";
  g.shadowBlur = 2;
  g.shadowOffsetY = 1;
  g.fillStyle = "#f6f5f1";
  g.fill(path);
  g.restore();
  g.lineJoin = "round";
  g.strokeStyle = "#111";
  g.lineWidth = 1;
  g.stroke(path);
}
function drawCross(g: CanvasRenderingContext2D) {
  g.save();
  g.shadowColor = "rgba(0,0,0,0.6)";
  g.shadowBlur = 2;
  g.fillStyle = "#111";
  g.fillRect(-1.5, -11.5, 3, 9);
  g.fillRect(-1.5, 2.5, 3, 9);
  g.fillRect(-11.5, -1.5, 9, 3);
  g.fillRect(2.5, -1.5, 9, 3);
  g.restore();
  g.fillStyle = "#f6f5f1";
  g.fillRect(-0.5, -11, 1, 8);
  g.fillRect(-0.5, 3, 1, 8);
  g.fillRect(-11, -0.5, 8, 1);
  g.fillRect(3, -0.5, 8, 1);
}

const iconCache = new Map<EditorIcon, Path2D[]>();
function drawIcon(
  g: CanvasRenderingContext2D,
  name: EditorIcon,
  cx: number,
  cy: number,
  col: string,
) {
  let paths = iconCache.get(name);
  if (!paths) {
    paths = EDITOR_ICONS[name].map((d) => new Path2D(d));
    iconCache.set(name, paths);
  }
  g.save();
  g.translate(cx - 8, cy - 8);
  g.fillStyle = col;
  for (const path of paths) g.fill(path);
  g.restore();
}
function barPanel(g: CanvasRenderingContext2D, w: number, h: number) {
  liftShadow(g, 0, 0, w, h, 12);
  rr(g, 0, 0, w, h, 12);
  g.fillStyle = C.surface;
  g.fill();
  rr(g, 0.5, 0.5, w - 1, h - 1, 12);
  g.strokeStyle = C.strokeHi;
  g.lineWidth = 1;
  g.stroke();
}
function drawToolBar(g: CanvasRenderingContext2D) {
  const { w, h } = TOOLS;
  barPanel(g, w, h);
  const top = BAR_PAD + CHIP / 2;
  TOOL_ICONS.forEach((name, i) => {
    drawIcon(g, name, BAR_PAD + i * (CHIP + CHIP_GAP) + CHIP / 2, top, C.muted);
  });
  const divX = BAR_PAD + TOOL_ROW_W + 6;
  g.fillStyle = C.strokeHi;
  g.fillRect(divX, BAR_PAD + 6, 1, CHIP - 12);
  HISTORY_ICONS.forEach((name, i) => {
    drawIcon(
      g,
      name,
      divX + 7 + i * (CHIP + CHIP_GAP) + CHIP / 2,
      top,
      i === 1 ? C.faint : C.muted,
    );
  });
  const lineY = BAR_PAD + CHIP + 6;
  g.fillRect(BAR_PAD + 2, lineY, w - BAR_PAD * 2 - 4, 1);
  const rowY = lineY + 7;
  const mid = rowY + CHIP / 2;
  SWATCHES.forEach((col, i) => {
    const x = BAR_PAD + i * 28;
    if (i === 0) {
      rr(g, x, rowY + 1, 28, 28, 7);
      g.fillStyle = "rgba(254,81,1,0.14)";
      g.fill();
    }
    rr(g, x + 7, mid - 7, 14, 14, 4);
    g.fillStyle = col;
    g.fill();
    if (col === "#000000") {
      rr(g, x + 7.5, mid - 6.5, 13, 13, 4);
      g.strokeStyle = C.strokeHi;
      g.stroke();
    }
  });
  let x = BAR_PAD + SWATCHES.length * 28 + 6;
  g.fillStyle = C.strokeHi;
  g.fillRect(x, rowY + 6, 1, CHIP - 12);
  x += 14;
  g.textBaseline = "middle";
  for (const [label, value, fill] of [
    ["Size", "4 px", 0.3],
    ["Opacity", "100%", 1],
  ] as [string, string, number][]) {
    g.font = GEIST(13, 500);
    g.fillStyle = C.muted;
    g.fillText(label, x, mid + 0.5);
    x += g.measureText(label).width + 10;
    rr(g, x, mid - 2, 84, 4, 2);
    g.fillStyle = C.strokeHi;
    g.fill();
    rr(g, x, mid - 2, 84 * fill, 4, 2);
    g.fillStyle = C.accent;
    g.fill();
    g.beginPath();
    g.arc(x + 84 * fill, mid, 6, 0, Math.PI * 2);
    g.fillStyle = C.fg;
    g.fill();
    x += 84 + 10;
    g.fillStyle = C.fg;
    g.fillText(value, x, mid + 0.5);
    x += g.measureText(value).width + 18;
  }
}
function drawModeBar(g: CanvasRenderingContext2D, t: number, alpha: number, lift: number) {
  const { x: bx, h } = MODES_BOX;
  const by = MODES_BOX.y - lift;
  g.save();
  g.globalAlpha = alpha;
  g.translate(bx, by);
  const mode = modeAt(t);
  const px = lerp(MODE_X[mode.from] ?? 0, MODE_X[mode.to] ?? 0, mode.u);
  const pw = lerp(MODE_W[mode.from] ?? 0, MODE_W[mode.to] ?? 0, mode.u);
  rr(g, px, BAR_PAD, pw, 30, 7);
  g.fillStyle = "rgba(254,81,1,0.24)";
  g.fill();
  const flash = mode.at >= 0 && t < mode.at + 0.14;
  MODES.forEach(([key, label], i) => {
    const x = (MODE_X[i] ?? 0) + 10;
    const cap = MODE_CAPS[i] ?? 20;
    const on = i === mode.to;
    rr(g, x + 0.5, h / 2 - 9.5, cap - 1, 19, 4);
    g.fillStyle = on ? (flash ? "#5a2510" : "#3a1d10") : C.raised;
    g.fill();
    g.strokeStyle = C.strokeHi;
    g.lineWidth = 1;
    g.stroke();
    g.font = GEIST(11, 500);
    g.fillStyle = on ? C.fg : C.muted;
    g.textBaseline = "middle";
    g.textAlign = "center";
    g.fillText(key ?? "", x + cap / 2, h / 2 + 0.5);
    g.textAlign = "left";
    g.font = GEIST(14, 500);
    g.fillText(label ?? "", x + cap + 6, h / 2 + 0.5);
  });
  g.restore();
}
function drawActionBar(g: CanvasRenderingContext2D) {
  const { w, h } = ACTIONS_BOX;
  barPanel(g, w, h);
  const mid = h / 2;
  g.font = GEIST(13, 500);
  g.textBaseline = "middle";
  g.fillStyle = C.fg;
  g.fillText(SIZE_LABEL, BAR_PAD + 8, mid + 0.5);
  ACTIONS.forEach((label, i) => {
    const x = ACTION_X[i] ?? 0,
      bw = ACTION_W[i] ?? 0;
    if (i === 0) {
      rr(g, x, mid - 14, bw, 28, 6);
      g.fillStyle = C.accent;
      g.fill();
    }
    g.fillStyle = i === 0 ? C.onAccent : C.muted;
    g.fillText(label, x + 10, mid + 0.5);
  });
}
function drawAnnotations(g: CanvasRenderingContext2D, end: P | null, step1: number, step2: number) {
  if (end) {
    const dx = end[0] - ARROW_A[0],
      dy = end[1] - ARROW_A[1];
    const len = Math.hypot(dx, dy);
    if (len > 3) {
      const ux = dx / len,
        uy = dy / len;
      const head = Math.min(22, len * 0.6);
      g.save();
      g.lineCap = "round";
      g.lineJoin = "round";
      g.strokeStyle = C.accent;
      g.fillStyle = C.accent;
      g.lineWidth = 4;
      g.beginPath();
      g.moveTo(ARROW_A[0], ARROW_A[1]);
      g.lineTo(end[0] - ux * head * 0.6, end[1] - uy * head * 0.6);
      g.stroke();
      g.beginPath();
      g.moveTo(end[0], end[1]);
      g.lineTo(end[0] - ux * head - uy * head * 0.5, end[1] - uy * head + ux * head * 0.5);
      g.lineTo(end[0] - ux * head + uy * head * 0.5, end[1] - uy * head - ux * head * 0.5);
      g.closePath();
      g.fill();
      g.stroke();
      g.restore();
    }
  }
  for (const [p, u, n] of [
    [STEP1, step1, "1"],
    [STEP2, step2, "2"],
  ] as [P, number, string][]) {
    if (u <= 0) continue;
    const r = 13 * lerp(0.55, 1, u) * (1 + 0.08 * Math.sin(Math.PI * u));
    g.save();
    g.globalAlpha *= clamp(u * 2);
    g.beginPath();
    g.arc(p[0], p[1], r, 0, Math.PI * 2);
    g.fillStyle = C.accent;
    g.fill();
    g.font = GEIST(14 * (r / 13), 600);
    g.fillStyle = C.onAccent;
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(n, p[0], p[1] + 1);
    g.restore();
  }
}

// ---------- engine ----------
export function startShowcase(
  canvas: HTMLCanvasElement,
  ui: {
    stage: HTMLElement;
    scrub: HTMLElement;
    bar: HTMLElement;
    time: HTMLElement;
    play: HTMLElement;
  },
) {
  const ctx = canvas.getContext("2d", { alpha: false })!;
  let k = 1,
    rk = 1,
    ox = 0,
    oy = 0,
    dpr = 1,
    vwL = W,
    vhL = H;
  let base!: HTMLCanvasElement, snap!: HTMLCanvasElement;
  let toolBar!: Sprite, actionBar!: Sprite, modePanel!: Sprite;
  let pop!: Sprite, note!: Sprite, arrow!: Sprite, cross!: Sprite, hintA!: Hint, hintB!: Hint;
  let sample!: Uint8ClampedArray;
  const loupeHex = new Map<number, string>();

  // pixel sampling source: frozen frame at 1:1 logical scale (rebuilt per layout)
  function buildSample() {
    const c = document.createElement("canvas");
    c.width = W;
    c.height = H;
    const g = c.getContext("2d", { willReadFrequently: true })!;
    drawDesktop(g);
    sample = g.getImageData(0, 0, W, H).data;
  }
  function px(x: number, y: number) {
    const i = (clamp(Math.round(y), 0, H - 1) * W + clamp(Math.round(x), 0, W - 1)) * 4;
    return ((sample[i] ?? 0) << 16) | ((sample[i + 1] ?? 0) << 8) | (sample[i + 2] ?? 0);
  }

  function layout() {
    dpr = Math.min(2, window.devicePixelRatio || 1);
    const vw = Math.max(1, ui.stage.clientWidth),
      vh = Math.max(1, ui.stage.clientHeight);
    setStageWidth(Math.round(clamp((vw / vh) * H, 1580, 2800)));
    buildSample();
    canvas.width = Math.round(vw * dpr);
    canvas.height = Math.round(vh * dpr);
    const s = Math.max(vw / W, vh / H);
    k = s * dpr;
    ox = ((vw - W * s) / 2) * dpr;
    oy = ((vh - H * s) / 2) * dpr;
    vwL = vw / s;
    vhL = vh / s;
    buildCamera(vwL, vhL);
    // render textures with headroom for the camera push-ins, capped for memory
    rk = Math.min(k * ZMAX, 6000 / W);
    base = document.createElement("canvas");
    base.width = Math.round(W * rk);
    base.height = Math.round(H * rk);
    const bg = base.getContext("2d")!;
    bg.scale(rk, rk);
    drawDesktop(bg);
    pop = makeSprite(POP.w, POP.h, rk, 40, drawPopover);
    note = makeSprite(NOTE.w, NOTE.h, rk, 40, (g) => drawNotification(g, base, rk));
    arrow = makeSprite(18, 24, rk, 4, drawArrow);
    toolBar = makeSprite(TOOLS.w, TOOLS.h, rk, 40, drawToolBar);
    actionBar = makeSprite(ACTIONS_BOX.w, ACTIONS_BOX.h, rk, 40, drawActionBar);
    modePanel = makeSprite(MODES_BOX.w, MODES_BOX.h, rk, 40, (g) =>
      barPanel(g, MODES_BOX.w, MODES_BOX.h),
    );
    cross = makeSprite(1, 1, rk, 14, drawCross);
    hintA = buildHint(HINT_A, rk);
    hintB = buildHint(HINT_B, rk);
    snap = document.createElement("canvas");
    snap.width = canvas.width;
    snap.height = canvas.height;
    render(snap.getContext("2d", { alpha: false })!, 0);
  }

  function render(g: CanvasRenderingContext2D, t: number) {
    const s = story(t);
    const cam = cameraAt(t, vwL, vhL);
    // camera: scale z about the stage, offset so (cx, cy) sits at the viewport centre
    const kz = k * cam.z;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.fillStyle = "#0b0a09";
    g.fillRect(0, 0, g.canvas.width, g.canvas.height);
    const vcx = ox + (W / 2) * k,
      vcy = oy + (H / 2) * k;
    g.setTransform(kz, 0, 0, kz, vcx - cam.cx * kz, vcy - cam.cy * kz);
    g.imageSmoothingQuality = "high";
    g.drawImage(base, 0, 0, W, H);
    // terminal block cursor (live desktop)
    if (blinkOn(s)) {
      g.fillStyle = "#e9e5dc";
      g.fillRect(termCursor[0], termCursor[1] - 14, monoCharW, 18);
    }
    // tray icon
    let hover = 0;
    if (s >= 1.35 && s < 2.75) hover = easeOut(prog(s, 1.35, 0.12));
    else if (s >= 2.75) hover = 1 - prog(s, 2.75, 0.11);
    if (hover > 0) {
      g.globalAlpha = hover;
      rr(g, TRAY[0] - 12, 5, 24, 24, 6);
      g.fillStyle = "rgba(239,238,234,0.05)";
      g.fill();
      g.globalAlpha = 1;
    }
    if (s >= 1.55 && s < 1.64) {
      rr(g, TRAY[0] - 12, 5, 24, 24, 6);
      g.fillStyle = "rgba(0,0,0,0.28)";
      g.fill();
    }
    aperture(g, TRAY[0], TRAY[1], s >= 6.05 && s < 6.21 ? C.accent : C.fg);
    // popover
    let pa = 0,
      ps = 1;
    if (s >= 1.55 && s < 2.75) {
      const u = easeOut(prog(s, 1.55, 0.14));
      pa = u;
      ps = lerp(0.96, 1, u);
    } else if (s >= 2.75 && s < 2.86) {
      const u = easeOut(prog(s, 2.75, 0.11));
      pa = 1 - u;
      ps = lerp(1, 0.97, u);
    }
    if (pa > 0) {
      g.save();
      g.globalAlpha = pa;
      g.translate(POP.x + POP.w, POP.y);
      g.scale(ps, ps);
      g.translate(-(POP.x + POP.w), -POP.y);
      blit(g, pop, POP.x, POP.y);
      if (s >= 2.75 && s < 2.84) {
        rr(g, POP.x + 6, POP.y + ROW0_Y, POP.w - 12, 32, 6);
        g.fillStyle = "rgba(0,0,0,0.25)";
        g.fill();
      }
      g.restore();
    }
    // selection overlay
    const cur = cursorAt(t);
    const ovIn =
      s >= 2.95 && s < 6.0
        ? easeOut(prog(s, 2.95, 0.16))
        : s >= 6.0 && s < 6.18
          ? 1 - prog(s, 6.0, 0.18)
          : 0;
    let sel: [number, number, number, number] | null = null;
    if (s >= 4.5 && s < 6.18) {
      const e = s < 5.4 ? cur : SEL1;
      sel = [
        Math.min(SEL0[0], e[0]),
        Math.min(SEL0[1], e[1]),
        Math.abs(e[0] - SEL0[0]),
        Math.abs(e[1] - SEL0[1]),
      ];
    }
    if (ovIn > 0) {
      g.fillStyle = `rgba(0,0,0,${0.55 * ovIn})`;
      g.beginPath();
      g.rect(0, 0, W, H);
      if (sel) g.rect(sel[0] + sel[2], sel[1], -sel[2], sel[3]);
      g.fill("nonzero");
    }
    const overlayUp = s >= 2.95 && s < 6.0;
    const editing = t >= ANNO_T0 ? clamp((t - ANNO_T0) / 0.15) : 0;
    if (overlayUp && sel) {
      const [sx, sy, sw, sh] = sel;
      g.strokeStyle = C.accent;
      g.lineWidth = 1;
      g.strokeRect(sx + 0.5, sy + 0.5, sw - 1, sh - 1);
      g.fillStyle = C.accent;
      for (const [hx, hy] of [
        [0, 0],
        [0.5, 0],
        [1, 0],
        [0, 0.5],
        [1, 0.5],
        [0, 1],
        [0.5, 1],
        [1, 1],
      ] as [number, number][])
        g.fillRect(sx + sw * hx - 3.5, sy + sh * hy - 3.5, 7, 7);
      g.globalAlpha = 1 - editing;
      const label = `${Math.round(sw)} × ${Math.round(sh)}`;
      g.font = GMONO(12);
      const lw = monoW12 * label.length + 16;
      const lx = sx + sw - lw,
        ly = sy + sh + 8;
      rr(g, lx, ly, lw, 22, 11);
      g.fillStyle = C.deep;
      g.fill();
      g.fillStyle = C.fg;
      g.textBaseline = "middle";
      g.fillText(label, lx + 8, ly + 11.5);
      g.globalAlpha = 1;
    }
    if (overlayUp && s >= 3.1) {
      const h = s < 5.4 ? hintA : hintB;
      g.globalAlpha = easeOut(prog(s, 3.1, 0.12)) * (1 - editing);
      blit(g, h.sprite, h.x, H - 32 - 36);
      g.globalAlpha = 1;
    }
    if (overlayUp && s >= 3.1 && s < 4.62) {
      const shown = easeOut(prog(s, 3.1, 0.16)) * (1 - easeOut(prog(s, 4.5, 0.12)));
      g.globalAlpha = shown;
      blit(g, modePanel, MODES_BOX.x, MODES_BOX.y - (1 - shown) * 6);
      g.globalAlpha = 1;
      drawModeBar(g, t, shown, (1 - shown) * 6);
    }
    // capture outline flash
    if (s >= 6.18 && s < 6.4) {
      g.globalAlpha = 1 - prog(s, 6.18, 0.22);
      g.strokeStyle = C.accent;
      g.lineWidth = 1;
      g.strokeRect(SEL0[0] + 0.5, SEL0[1] + 0.5, SEL1[0] - SEL0[0] - 1, SEL1[1] - SEL0[1] - 1);
      g.globalAlpha = 1;
    }
    // loupe
    if (overlayUp && s >= 3.3 && editing < 1) {
      g.globalAlpha = easeOut(prog(s, 3.3, 0.08)) * (1 - editing);
      const LW = 120,
        LH2 = 146;
      let lx = cur[0] + 24,
        ly = cur[1] + 24;
      if (lx + LW > W - 8) lx = cur[0] - 24 - LW;
      if (ly + LH2 > H - 80) ly = cur[1] - 24 - LH2;
      rr(g, lx, ly, LW, LH2, 10);
      g.fillStyle = C.surface;
      g.fill();
      rr(g, lx + 0.5, ly + 0.5, LW - 1, LH2 - 1, 10);
      g.strokeStyle = C.stroke;
      g.stroke();
      const cs = 12,
        gx = lx + 6,
        gy = ly + 6;
      g.save();
      rr(g, gx, gy, 108, 108, 6);
      g.clip();
      const cxp = Math.round(cur[0]),
        cyp = Math.round(cur[1]);
      for (let j = -4; j <= 4; j++)
        for (let i = -4; i <= 4; i++) {
          const v = px(cxp + i, cyp + j);
          g.fillStyle = `rgb(${v >> 16},${(v >> 8) & 255},${v & 255})`;
          g.fillRect(gx + (i + 4) * cs, gy + (j + 4) * cs, cs, cs);
        }
      g.restore();
      g.strokeStyle = C.accent;
      g.lineWidth = 1.5;
      g.strokeRect(gx + 4 * cs + 0.75, gy + 4 * cs + 0.75, cs - 1.5, cs - 1.5);
      const v = px(cxp, cyp);
      let hex = loupeHex.get(v);
      if (!hex) {
        hex = "#" + v.toString(16).padStart(6, "0");
        loupeHex.set(v, hex);
      }
      g.fillStyle = `rgb(${v >> 16},${(v >> 8) & 255},${v & 255})`;
      rr(g, lx + 10, ly + 122, 12, 12, 3);
      g.fill();
      g.font = GMONO(12);
      g.fillStyle = C.muted;
      g.textBaseline = "middle";
      g.fillText(hex, lx + 30, ly + 128.5);
      g.globalAlpha = 1;
    }
    const annotated = t >= T_DRAW0 && s < 6.18 ? (s < 6.0 ? 1 : 1 - prog(s, 6.0, 0.18)) : 0;
    if (annotated > 0) {
      g.globalAlpha = annotated;
      const end = t < T_DRAW1 ? cur : ARROW_B;
      drawAnnotations(g, end, easeOut(prog(t, T_STEP1, 0.18)), easeOut(prog(t, T_STEP2, 0.18)));
      g.globalAlpha = 1;
    }
    const bars = barsIn(t);
    if (bars > 0) {
      g.globalAlpha = bars;
      const lift = (1 - bars) * 8;
      blit(g, toolBar, TOOLS.x, TOOLS.y - lift);
      const [ax, ay] = toolCenter(activeTool(t));
      rr(g, ax - CHIP / 2, ay - CHIP / 2 - lift, CHIP, CHIP, 8);
      const clicked = [T_ARROW_TOOL, T_STEP_TOOL].some((at) => t >= at && t < at + 0.12);
      g.fillStyle = clicked ? "rgba(254,81,1,0.3)" : "rgba(254,81,1,0.14)";
      g.fill();
      drawIcon(g, TOOL_ICONS[activeTool(t)] ?? "region", ax, ay - lift, C.fg);
      blit(g, actionBar, ACTIONS_BOX.x, ACTIONS_BOX.y + lift);
      if (t >= T_ACCEPT && t < T_ACCEPT + 0.12) {
        rr(
          g,
          ACTIONS_BOX.x + (ACTION_X[0] ?? 0),
          ACTIONS_BOX.y + lift + ACTIONS_BOX.h / 2 - 14,
          ACTION_W[0] ?? 0,
          28,
          6,
        );
        g.fillStyle = "rgba(0,0,0,0.22)";
        g.fill();
      }
      g.globalAlpha = 1;
    }
    // notification
    let na = 0,
      nx = 0;
    if (s >= 6.5 && s < 10.5) {
      const u = easeOut(prog(s, 6.5, 0.22));
      na = u;
      nx = 24 * (1 - u);
    } else if (s >= 10.5 && s < 10.7) {
      const u = easeInOut(prog(s, 10.5, 0.2));
      na = 1 - u;
      nx = 24 * u;
    }
    if (na > 0) {
      g.globalAlpha = na;
      blit(g, note, NOTE.x + nx, NOTE.y);
      g.globalAlpha = 1;
    }
    // cursor, always last
    const dip = pressed(t) ? 1 : 0;
    if (crosshair(t)) blit(g, cross, cur[0], cur[1] + dip);
    else blit(g, arrow, cur[0], cur[1] + dip);
    // film light: soft falloff away from the action, plus a gentle vignette
    g.setTransform(1, 0, 0, 1, 0, 0);
    const cw = g.canvas.width,
      ch = g.canvas.height;
    const fx = vcx + (cam.fx - cam.cx) * kz,
      fy = vcy + (cam.fy - cam.cy) * kz;
    const rad = Math.max(cw, ch) * 0.75;
    const vg = g.createRadialGradient(fx, fy, rad * 0.25, fx, fy, rad);
    vg.addColorStop(0, "rgba(0,0,0,0)");
    vg.addColorStop(1, `rgba(0,0,0,${0.35 + 0.25 * cam.focus})`);
    g.fillStyle = vg;
    g.fillRect(0, 0, cw, ch);
    // loop crossfade back to s = 0
    if (t > TAKE) {
      g.setTransform(1, 0, 0, 1, 0, 0);
      g.globalAlpha = easeInOut(prog(t, TAKE, 0.3));
      g.drawImage(snap, 0, 0);
      g.globalAlpha = 1;
    }
  }
  let monoW12 = 7.2;
  const mctx = document.createElement("canvas").getContext("2d")!;
  mctx.font = GMONO(12);
  monoW12 = mctx.measureText("0").width;

  layout();
  let lastW = ui.stage.clientWidth,
    lastH = ui.stage.clientHeight;
  const resizer = new ResizeObserver(() => {
    if (ui.stage.clientWidth === lastW && ui.stage.clientHeight === lastH) return;
    lastW = ui.stage.clientWidth;
    lastH = ui.stage.clientHeight;
    layout();
    draw();
  });
  resizer.observe(ui.stage);

  // ---------- playback ----------
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let t = reduced ? FINAL_STILL : 0;
  const dev = new URLSearchParams(window.location.search).has("dev");
  let playing = !reduced,
    once = false,
    last = 0,
    raf = 0,
    showDev = false,
    visible = true;
  const watcher = new IntersectionObserver(([entry]) => {
    visible = entry?.isIntersecting ?? true;
    last = 0;
  });
  watcher.observe(ui.stage);
  let frames = 0,
    sum = 0,
    longFrames = 0,
    avg = 0,
    lastTimeLabel = -1;
  const ring: number[] = [];
  ui.play.style.display = reduced ? "flex" : "none";

  function draw() {
    render(ctx, t);
    if (showDev) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = "rgba(5,5,4,0.85)";
      ctx.fillRect(10, 10, 210, 48);
      ctx.font = GMONO(12);
      ctx.fillStyle = C.fg;
      ctx.textBaseline = "middle";
      ctx.fillText(`avg ${avg.toFixed(2)} ms`, 20, 26);
      ctx.fillStyle = longFrames ? C.accent : C.ok;
      ctx.fillText(`long frames ${longFrames}`, 20, 44);
    }
    ui.bar.style.transform = `scaleX(${t / LOOP})`;
    const tl = Math.floor(t * 10);
    if (tl !== lastTimeLabel) {
      lastTimeLabel = tl;
      ui.time.textContent = `${t.toFixed(1)} / ${LOOP.toFixed(1)} s`;
    }
  }
  function frame(now: number) {
    raf = requestAnimationFrame(frame);
    const dt = last ? now - last : 16.67;
    last = now;
    if (playing && visible) {
      ring.push(dt);
      sum += dt;
      if (ring.length > 60) sum -= ring.shift()!;
      avg = sum / ring.length;
      frames++;
      if (frames > 30 && dt > 17) longFrames++;
      t += Math.min(dt, 100) / 1000;
      if (once && t >= TAKE) {
        once = false;
        playing = false;
        t = FINAL_STILL;
        ui.play.style.display = "flex";
      }
      if (t >= LOOP) {
        t -= LOOP;
        longFrames = 0;
        frames = 0;
      }
      draw();
    }
  }
  draw();
  raf = requestAnimationFrame(frame);

  function onKey(e: KeyboardEvent) {
    if (!visible || e.ctrlKey || e.metaKey || e.altKey) return;
    const onPage = e.target === document.body || e.target === ui.stage;
    if (!onPage) return;
    if (e.code === "Space") {
      e.preventDefault();
      playing = !playing;
      last = 0;
    } else if (e.key === "r" || e.key === "R") {
      t = 0;
      longFrames = 0;
      frames = 0;
      playing = true;
      ui.play.style.display = "none";
      draw();
    } else if (dev && (e.key === "f" || e.key === "F")) {
      showDev = !showDev;
      draw();
    }
  }
  function onClick(e: MouseEvent) {
    if (!dev || playing || e.target !== canvas) return;
    t = (t + 1 / 60) % LOOP;
    draw();
  }
  let scrubbing = false;
  function scrubTo(x: number) {
    const box = ui.scrub.getBoundingClientRect();
    t = clamp((x - box.left) / Math.max(1, box.width)) * LOOP * 0.99999;
    draw();
  }
  function onDown(e: PointerEvent) {
    scrubbing = true;
    ui.scrub.setPointerCapture(e.pointerId);
    scrubTo(e.clientX);
  }
  function onMove(e: PointerEvent) {
    if (scrubbing) scrubTo(e.clientX);
  }
  function onUp() {
    scrubbing = false;
    last = 0;
  }
  function onPlay() {
    t = 0;
    once = true;
    playing = true;
    last = 0;
    ui.play.style.display = "none";
  }
  window.addEventListener("keydown", onKey);
  ui.stage.addEventListener("click", onClick);
  ui.scrub.addEventListener("pointerdown", onDown);
  ui.scrub.addEventListener("pointermove", onMove);
  ui.scrub.addEventListener("pointerup", onUp);
  ui.play.addEventListener("click", onPlay);

  return () => {
    cancelAnimationFrame(raf);
    resizer.disconnect();
    watcher.disconnect();
    window.removeEventListener("keydown", onKey);
    ui.stage.removeEventListener("click", onClick);
    ui.scrub.removeEventListener("pointerdown", onDown);
    ui.scrub.removeEventListener("pointermove", onMove);
    ui.scrub.removeEventListener("pointerup", onUp);
    ui.play.removeEventListener("click", onPlay);
  };
}

export const __timeline = { cursorAt, frozen, blinkOn, LOOP, SEL0, SEL1 };

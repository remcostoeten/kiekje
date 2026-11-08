export const STROKES =
  ":is(path, line, rect, circle, ellipse):not(.kk-art-dash, .kk-art-late *, .kk-ink, defs *)";

const CLOSED_PATH = /^\s*[Mm][^Mm]*[Zz]\s*$/;
const REACH = 52;
const SPREAD = 0.22;
const HOLD = 420;
const DRAW_IN = 280;
const SETTLE = 0.42;
const SETTLE_TIME = 720;
const LEAVE_TIME = 700;
const STAGGER = 1.1;
const STAGGER_MAX = 240;
const ACCENT_TIME = 280;
const ACCENTS = 3;
const HOT_MAX_LEN = 200;
const SHOWN = 0.002;

type Tone = "ink" | "fine" | "lit" | "mark";
type Sample = { x: number; y: number; at: number };
type Client = { x: number; y: number };
type Item = {
  clone: SVGGeometryElement;
  len: number;
  closed: boolean;
  points: Sample[];
  drawn: number;
  lit: number;
  hot: number;
  fade: number;
  anchor: number;
  near: number;
  release: number;
  visited: boolean;
  distance: number;
  shown: boolean;
  litText: string;
  hotText: string;
};
type Surface = {
  svg: SVGSVGElement;
  items: Item[];
  hovered: boolean;
  entered: number;
  client: Client | null;
};

function stillMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * @name arm
 * @description Measures every solid stroke inside the `.kk-art` drawings under a node
 * and stores its length and draw-in delay as custom properties, then marks the node
 * ready so the CSS draw-in can start when it gets `data-draw="on"`.
 *
 * @example
 * arm(grid);
 * grid.dataset["draw"] = "on";
 */
export function arm(node: HTMLElement, step = 14, cap = 420) {
  node.querySelectorAll<SVGSVGElement>(".kk-art").forEach((svg) => {
    svg.querySelectorAll<SVGGeometryElement>(STROKES).forEach((stroke, index) => {
      stroke.style.setProperty("--kk-len", `${Math.ceil(stroke.getTotalLength()) + 1}`);
      stroke.style.setProperty("--kk-step", `${Math.min(index * step, cap)}ms`);
    });
  });
  node.dataset["draw"] = "ready";
}

/**
 * @name float
 * @description Gives the `.kk-art-layer` groups of a drawing parallax depth: they drift
 * on slow sines while idle and follow the mouse over the surface, with a light tilt of
 * the whole drawing while hovered. Returns a cleanup function.
 *
 * @example
 * useEffect(() => float(svg.current, card.current), []);
 */
export function float(svg: SVGSVGElement, surface: HTMLElement) {
  if (stillMotion()) return undefined;
  const layers = [...svg.querySelectorAll<SVGGElement>(".kk-art-layer")].map(
    (layer) => [layer, Number(layer.dataset["depth"] ?? 1)] as const,
  );
  const begin = performance.now();
  let targetX = 0;
  let targetY = 0;
  let x = 0;
  let y = 0;
  let tilt = 0;
  let frame = 0;
  let hover = false;
  let visible = false;

  function step(now: number) {
    if (!hover) {
      const time = (now - begin) / 1000;
      targetX = Math.sin(time * 0.55) * 0.3;
      targetY = Math.cos(time * 0.4) * 0.22;
    }
    x += (targetX - x) * 0.07;
    y += (targetY - y) * 0.07;
    for (const [layer, depth] of layers) {
      layer.style.transform = `translate(${(x * depth * 6).toFixed(2)}px, ${(y * depth * 4.5).toFixed(2)}px)`;
    }
    tilt += ((hover ? 1 : 0) - tilt) * 0.08;
    svg.style.transform =
      tilt < 0.01
        ? ""
        : `perspective(900px) rotateX(${(-y * 5 * tilt).toFixed(2)}deg) rotateY(${(x * 7 * tilt).toFixed(2)}deg)`;
    frame = visible ? requestAnimationFrame(step) : 0;
  }

  function move(event: PointerEvent) {
    if (event.pointerType !== "mouse") return;
    const box = surface.getBoundingClientRect();
    targetX = ((event.clientX - box.left) / box.width) * 2 - 1;
    targetY = ((event.clientY - box.top) / box.height) * 2 - 1;
    hover = true;
  }

  function leave() {
    hover = false;
  }

  const observer = new IntersectionObserver(([entry]) => {
    visible = Boolean(entry?.isIntersecting);
    if (visible && !frame) frame = requestAnimationFrame(step);
  });
  observer.observe(svg);
  surface.addEventListener("pointermove", move);
  surface.addEventListener("pointerleave", leave);
  return () => {
    observer.disconnect();
    cancelAnimationFrame(frame);
    surface.removeEventListener("pointermove", move);
    surface.removeEventListener("pointerleave", leave);
  };
}

function userSpace(stroke: SVGGeometryElement, svg: SVGSVGElement) {
  const own = stroke.getScreenCTM();
  const root = svg.getScreenCTM();
  if (!own || !root) return new DOMMatrix();
  return root.inverse().multiply(own);
}

function sample(stroke: SVGGeometryElement, len: number, svg: SVGSVGElement) {
  const matrix = userSpace(stroke, svg);
  const count = Math.max(2, Math.min(24, Math.ceil(len / 12)));
  const points: Sample[] = [];
  for (let index = 0; index <= count; index += 1) {
    const at = (len * index) / count;
    const own = stroke.getPointAtLength(at);
    const point = new DOMPoint(own.x, own.y).matrixTransform(matrix);
    points.push({ x: point.x, y: point.y, at });
  }
  return points;
}

function toneOf(stroke: SVGGeometryElement): Tone {
  if (stroke.classList.contains("kk-art-fine")) return "fine";
  if (stroke.classList.contains("kk-art-lit")) return "lit";
  if (stroke.classList.contains("kk-art-accent")) return "mark";
  return "ink";
}

function isClosed(stroke: SVGGeometryElement) {
  if (stroke.localName === "line") return false;
  if (stroke.localName !== "path") return true;
  return CLOSED_PATH.test(stroke.getAttribute("d") ?? "");
}

function inkItem(stroke: SVGGeometryElement, svg: SVGSVGElement): Item | null {
  const clone = stroke.cloneNode(false);
  if (!(clone instanceof SVGGeometryElement)) return null;
  clone.removeAttribute("class");
  clone.removeAttribute("style");
  clone.removeAttribute("id");
  clone.classList.add("kk-ink");
  clone.setAttribute("aria-hidden", "true");
  clone.dataset["tone"] = toneOf(stroke);
  stroke.after(clone);
  const len = stroke.getTotalLength();
  return {
    clone,
    len,
    closed: isClosed(stroke),
    points: sample(stroke, len, svg),
    drawn: 0,
    lit: 0,
    hot: 0,
    fade: 0,
    anchor: 0,
    near: -Infinity,
    release: 0,
    visited: false,
    distance: Infinity,
    shown: false,
    litText: "",
    hotText: "",
  };
}

function nearest(item: Item, x: number, y: number) {
  let best = Infinity;
  let at = 0;
  for (const point of item.points) {
    const distance = Math.hypot(point.x - x, point.y - y);
    if (distance < best) {
      best = distance;
      at = point.at;
    }
  }
  return { best, at };
}

function ease(value: number) {
  return value * value * (3 - 2 * value);
}

function percent(value: number) {
  return `${(ease(value) * 100).toFixed(1)}%`;
}

function dash(item: Item, eased: number) {
  const length = eased * item.len;
  if (item.closed) {
    const from = (((item.anchor - length / 2) % item.len) + item.len) % item.len;
    return { length, gap: Math.max(0, item.len - length), from };
  }
  return { length, gap: item.len * 2 + 2, from: item.anchor * (1 - eased) };
}

function paint(item: Item) {
  const visible = item.drawn > SHOWN;
  if (visible !== item.shown) {
    item.clone.style.visibility = visible ? "visible" : "hidden";
    item.shown = visible;
  }
  if (!visible) return;
  const { length, gap, from } = dash(item, ease(item.drawn));
  item.clone.style.strokeDasharray = `${length.toFixed(2)} ${gap.toFixed(2)}`;
  item.clone.style.strokeDashoffset = `${(-from).toFixed(2)}`;
  const litText = percent(item.lit);
  if (litText !== item.litText) {
    item.clone.style.setProperty("--kk-lit", litText);
    item.litText = litText;
  }
  const hotText = percent(item.hot);
  if (hotText !== item.hotText) {
    item.clone.style.setProperty("--kk-hot", hotText);
    item.hotText = hotText;
  }
}

function local(state: Surface, client: Client | null) {
  const screen = state.svg.getScreenCTM();
  if (!screen || !client) return null;
  return new DOMPoint(client.x, client.y).matrixTransform(screen.inverse());
}

function toward(value: number, target: number, duration: number, dt: number) {
  const delta = dt / duration;
  return value < target ? Math.min(target, value + delta) : Math.max(target, value - delta);
}

function aim(state: Surface, now: number) {
  const pointer = local(state, state.client);
  if (!pointer) return;
  const reach = Math.min(REACH, 8 + (now - state.entered) * SPREAD);
  state.items.forEach((item) => {
    const { best, at } = nearest(item, pointer.x, pointer.y);
    item.distance = best;
    if (best >= reach) return;
    if (item.drawn <= SHOWN) {
      item.anchor = at;
      item.lit = 1;
    }
    item.visited = true;
    item.near = now;
  });
}

function release(state: Surface, client: Client, now: number) {
  const pointer = local(state, client);
  const distances = state.items.map((item) =>
    pointer ? nearest(item, pointer.x, pointer.y).best : 0,
  );
  const closest = Math.min(
    ...distances.filter((_, index) => (state.items[index]?.drawn ?? 0) > SHOWN),
  );
  state.items.forEach((item, index) => {
    const lag = Number.isFinite(closest) ? ((distances[index] ?? 0) - closest) * STAGGER : 0;
    item.visited = false;
    item.distance = Infinity;
    item.release = now + Math.min(STAGGER_MAX, Math.max(0, lag));
    item.fade = item.lit / (Math.max(item.drawn, SHOWN) * LEAVE_TIME);
  });
}

function explore(item: Item, now: number, dt: number) {
  const lit = now - item.near <= HOLD ? 1 : SETTLE;
  item.drawn = toward(item.drawn, 1, DRAW_IN, dt);
  item.lit =
    item.lit < lit
      ? toward(item.lit, lit, DRAW_IN, dt)
      : toward(item.lit, lit, SETTLE_TIME / (1 - SETTLE), dt);
}

function recede(item: Item, now: number, dt: number) {
  if (now < item.release) return;
  item.drawn = Math.max(0, item.drawn - dt / LEAVE_TIME);
  item.lit = item.drawn > 0 ? Math.max(0, item.lit - dt * item.fade) : 0;
}

function pickHot(items: Item[], count: number) {
  const picked: Item[] = [];
  for (const item of items) {
    if (item.drawn <= 0.3 || item.distance >= REACH || item.len > HOT_MAX_LEN) continue;
    const slot = picked.findIndex((other) => item.distance < other.distance);
    picked.splice(slot === -1 ? picked.length : slot, 0, item);
    if (picked.length > count) picked.pop();
  }
  return picked;
}

function accent(state: Surface, dt: number) {
  const hot = new Set(state.hovered ? pickHot(state.items, ACCENTS) : []);
  state.items.forEach((item) => {
    item.hot = toward(item.hot, hot.has(item) ? 1 : 0, ACCENT_TIME, dt);
  });
}

function tickSurface(state: Surface, now: number, dt: number) {
  if (state.hovered) aim(state, now);
  state.items.forEach((item) => {
    if (state.hovered && item.visited) explore(item, now, dt);
    else recede(item, now, dt);
  });
  accent(state, dt);
  state.items.forEach((item) => paint(item));
  return state.hovered || state.items.some((item) => item.drawn > 0 || item.hot > 0);
}

/**
 * @name ink
 * @description Lets a mouse pointer ink a `.kk-art` drawing: strokes near the pointer
 * draw in from the point nearest to it, the closest few turn accent, strokes the
 * pointer leaves settle to a half tone, and everything recedes when the pointer leaves
 * the surface. Returns a cleanup function.
 *
 * @example
 * useEffect(() => ink(svg.current, card.current), []);
 */
export function ink(svg: SVGSVGElement, surface: HTMLElement) {
  if (stillMotion()) return undefined;
  const items = [...svg.querySelectorAll<SVGGeometryElement>(STROKES)]
    .map((stroke) => inkItem(stroke, svg))
    .filter((item) => item !== null);
  const state: Surface = { svg, items, hovered: false, entered: 0, client: null };
  let frame = 0;
  let last = 0;

  function tick(now: number) {
    const dt = Math.max(1, Math.min(64, now - last));
    last = now;
    frame = tickSurface(state, now, dt) ? requestAnimationFrame(tick) : 0;
  }

  function wake() {
    if (frame) return;
    last = performance.now();
    frame = requestAnimationFrame(tick);
  }

  function enter(event: PointerEvent) {
    state.hovered = true;
    state.entered = performance.now();
    state.client = { x: event.clientX, y: event.clientY };
    wake();
  }

  function move(event: PointerEvent) {
    if (event.pointerType !== "mouse") return;
    if (!state.hovered) {
      enter(event);
      return;
    }
    state.client = { x: event.clientX, y: event.clientY };
  }

  function start(event: PointerEvent) {
    if (event.pointerType === "mouse") enter(event);
  }

  function leave(event: PointerEvent) {
    if (event.pointerType !== "mouse") return;
    state.hovered = false;
    release(state, { x: event.clientX, y: event.clientY }, performance.now());
    wake();
  }

  surface.addEventListener("pointerenter", start);
  surface.addEventListener("pointermove", move);
  surface.addEventListener("pointerleave", leave);
  return () => {
    cancelAnimationFrame(frame);
    surface.removeEventListener("pointerenter", start);
    surface.removeEventListener("pointermove", move);
    surface.removeEventListener("pointerleave", leave);
    items.forEach((item) => item.clone.remove());
  };
}

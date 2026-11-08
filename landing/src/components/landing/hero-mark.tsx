import { useEffect, useRef } from "react";

const CORNERS = ["tl", "tr", "bl", "br"];
const dragEase = cubicBezier(0.77, 0, 0.175, 1);

function cubicBezier(x1: number, y1: number, x2: number, y2: number) {
  const ax = 1 - 3 * x2 + 3 * x1;
  const bx = 3 * x2 - 6 * x1;
  const cx = 3 * x1;
  const ay = 1 - 3 * y2 + 3 * y1;
  const by = 3 * y2 - 6 * y1;
  const cy = 3 * y1;
  return (x: number) => {
    let t = x;
    for (let step = 0; step < 8; step++) {
      const error = ((ax * t + bx) * t + cx) * t - x;
      const slope = (3 * ax * t + 2 * bx) * t + cx;
      if (Math.abs(error) < 1e-6 || slope === 0) break;
      t -= error / slope;
    }
    const clamped = Math.min(1, Math.max(0, t));
    return ((ay * clamped + by) * clamped + cy) * clamped;
  };
}

function followDrag(node: HTMLElement, text: HTMLElement, drag: Animation | undefined) {
  let width = 0;
  let height = 0;
  let request = 0;

  function paint() {
    if (!width) return;
    const progress = dragEase(drag?.effect?.getComputedTiming().progress ?? 1);
    text.textContent = `${Math.round(width * progress)} × ${Math.round(height * progress)}`;
  }

  function tick() {
    paint();
    if (drag?.playState === "running") request = requestAnimationFrame(tick);
  }

  const observer = new ResizeObserver(() => {
    const box = node.getBoundingClientRect();
    const scale = window.devicePixelRatio || 1;
    width = Math.round(box.width * scale);
    height = Math.round(box.height * scale);
    paint();
  });
  observer.observe(node);
  tick();
  return () => {
    observer.disconnect();
    cancelAnimationFrame(request);
  };
}

export function HeroMark() {
  const frame = useRef<HTMLSpanElement>(null);
  const label = useRef<HTMLSpanElement>(null);
  const fill = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const node = frame.current;
    const text = label.current;
    if (!node || !text) return;
    return followDrag(node, text, fill.current?.getAnimations()[0]);
  }, []);

  return (
    <h1 id="hero-title" className="kk-mark">
      <span ref={frame} className="kk-mark-frame">
        <span className="kk-mark-word">Kiekje</span>
        <span className="kk-mark-box" aria-hidden="true">
          <span ref={label} className="kk-mark-size" />
          {CORNERS.map((corner) => (
            <i key={corner} className="kk-mark-handle" data-corner={corner} />
          ))}
          <span className="kk-mark-drag">
            <span className="kk-drag-lit">
              <span className="kk-drag-word">Kiekje</span>
            </span>
            <span ref={fill} className="kk-drag-fill" />
            <i className="kk-drag-edge" data-edge="t" />
            <i className="kk-drag-edge" data-edge="l" />
            <span className="kk-drag-corner">
              <i className="kk-drag-edge" data-edge="r" />
              <i className="kk-drag-edge" data-edge="b" />
              <i className="kk-drag-cursor" />
            </span>
          </span>
        </span>
      </span>
      <span className="kk-sr">, a screenshot tool for Linux</span>
    </h1>
  );
}

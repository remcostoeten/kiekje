import { useEffect, useRef, useState, type AnimationEvent, type CSSProperties } from "react";

import { STEPS, type StepId } from "@/kiekje/content";
import { arm } from "@/shared/helpers/line-art";

import { StepArt, type Flow, type Source } from "./step-art";

type StepState = "idle" | "active" | "done";

const SOURCES: Source[] = ["tray", "key", "cli"];
const DURATION: Record<StepId, number> = {
  trigger: 3.4,
  freeze: 2.8,
  select: 5.6,
  keep: 4.2,
};

function finePointer() {
  return window.matchMedia("(hover: hover) and (pointer: fine)").matches;
}

function stateOf(index: number, active: number): StepState {
  if (index === active) return "active";
  return index < active ? "done" : "idle";
}

function nextSource(current: Source) {
  return SOURCES[(SOURCES.indexOf(current) + 1) % SOURCES.length] ?? "tray";
}

export function Steps() {
  const rootRef = useRef<HTMLDivElement>(null);
  const pinned = useRef(false);
  const finished = useRef(false);
  const [enabled, setEnabled] = useState(false);
  const [visible, setVisible] = useState(false);
  const [active, setActive] = useState(0);
  const [source, setSource] = useState<Source>("tray");
  const [flow, setFlow] = useState<Flow>("next");

  useEffect(() => {
    const node = rootRef.current;
    if (!node || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    arm(node, 6, 900);
    setEnabled(true);
    const observer = new IntersectionObserver(
      ([entry]) => {
        const seen = Boolean(entry?.isIntersecting);
        setVisible(seen);
        if (seen) node.dataset["draw"] = "on";
      },
      { threshold: 0.25 },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  function go(index: number, kind: Flow) {
    finished.current = false;
    if (index === 0) setSource(nextSource);
    setFlow(kind);
    setActive(index);
  }

  function onEnd(event: AnimationEvent<HTMLLIElement>, index: number) {
    if (event.animationName !== "kk-bar" || index !== active) return;
    finished.current = true;
    if (!pinned.current) go((index + 1) % STEPS.length, "next");
  }

  function pin(index: number) {
    if (!enabled || !finePointer()) return;
    pinned.current = true;
    if (index !== active) go(index, index === (active + 1) % STEPS.length ? "next" : "jump");
  }

  function release() {
    if (!pinned.current) return;
    pinned.current = false;
    if (finished.current) go((active + 1) % STEPS.length, "next");
  }

  const current = STEPS[active] ?? STEPS[0];
  const play = { "--kk-play": `${DURATION[current?.id ?? "trigger"]}s` } as CSSProperties;
  let motion: string | undefined;
  if (enabled) motion = visible ? "run" : "hold";

  return (
    <section id="how" className="kk-section" aria-labelledby="how-title">
      <div className="kk-container">
        <header className="kk-section-head">
          <h2 id="how-title">Capture first, select second</h2>
          <p>
            Kiekje grabs every monitor the moment you press the key and keeps that frame still while
            you choose. What you select is what was on screen then, not what is there when you let
            go.
          </p>
        </header>
        <div
          ref={rootRef}
          className="kk-how kk-drawn"
          data-motion={motion}
          data-live={enabled ? String(visible) : undefined}
          style={play}
        >
          <ol className="kk-how-list" onPointerLeave={release}>
            {STEPS.map((step, index) => (
              <li
                key={step.id}
                className="kk-how-step"
                data-state={enabled ? stateOf(index, active) : undefined}
                onPointerEnter={() => pin(index)}
                onAnimationEnd={(event) => onEnd(event, index)}
              >
                <span className="kk-how-index">
                  {String(index + 1).padStart(2, "0")}
                  <span className="kk-how-bar" aria-hidden="true">
                    <i />
                  </span>
                </span>
                <h3>{step.title}</h3>
                <p>{step.body}</p>
              </li>
            ))}
          </ol>
          <StepArt step={enabled ? current?.id : undefined} source={source} flow={flow} />
        </div>
      </div>
    </section>
  );
}

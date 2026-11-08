import { useEffect, useRef } from "react";

import { LINKS, PRIVACY, type Privacy as Fact, type PrivacyArt } from "@/kiekje/content";
import { arm } from "@/shared/helpers/line-art";

import { Icon } from "./icons";
import { PrivacyArt as Art } from "./privacy-art";

type Beat = { loop: string; period: number; start: number };

const BEATS: Record<PrivacyArt, Beat> = {
  telemetry: { loop: "kk-art-packet", period: 4800, start: 760 },
  update: { loop: "kk-art-request", period: 10000, start: 0 },
  upload: { loop: "kk-art-plug", period: 4200, start: 1150 },
  text: { loop: "kk-art-scan", period: 6000, start: 1440 },
};

function loopPhase(card: HTMLElement, beat: Beat) {
  const loop = card
    .querySelector(`.${beat.loop}`)
    ?.getAnimations()
    .find(
      (animation) => animation instanceof CSSAnimation && animation.animationName === beat.loop,
    );
  const time = loop?.currentTime;
  if (typeof time !== "number") return null;
  const local = time - Number(loop?.effect?.getTiming().delay ?? 0);
  return ((local % beat.period) + beat.period) % beat.period;
}

function keepTime(card: HTMLElement) {
  const beat = BEATS[card.dataset["art"] as PrivacyArt];
  const phase = beat ? loopPhase(card, beat) : null;
  if (!beat || phase === null) return;
  const delay = phase <= beat.start ? -phase : beat.period - phase;
  card.style.setProperty("--kk-beat-delay", `${delay.toFixed(1)}ms`);
  card.dataset["lit"] = "";
}

function Body({ fact }: { fact: Fact }) {
  const at = fact.body.indexOf(fact.key);
  if (at === -1) return <p>{fact.body}</p>;
  return (
    <p>
      {fact.body.slice(0, at)}
      <span className="kk-fact-key">{fact.key}</span>
      {fact.body.slice(at + fact.key.length)}
    </p>
  );
}

export function Privacy() {
  const grid = useRef<HTMLUListElement>(null);

  useEffect(() => {
    const node = grid.current;
    if (!node) return;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still) return;
    arm(node);
    node.dataset["live"] = "false";
    const observer = new IntersectionObserver(
      ([entry]) => {
        const seen = Boolean(entry?.isIntersecting);
        node.dataset["live"] = String(seen);
        if (seen) node.dataset["draw"] = "on";
      },
      { threshold: 0.15 },
    );
    function handOff(event: AnimationEvent) {
      if (event.animationName !== "kk-fact-hand" || !(event.target instanceof Element)) return;
      const card = event.target.closest<HTMLElement>(".kk-fact");
      if (card) keepTime(card);
    }
    observer.observe(node);
    node.addEventListener("animationend", handOff);
    return () => {
      observer.disconnect();
      node.removeEventListener("animationend", handOff);
    };
  }, []);

  return (
    <section id="privacy" className="kk-section" aria-labelledby="privacy-title">
      <div className="kk-container kk-privacy">
        <header className="kk-section-head">
          <h2 id="privacy-title">Your captures stay on your machine</h2>
          <p>
            Captures go to ~/Pictures/Kiekje, recordings to ~/Videos/Kiekje, and the gallery index
            to ~/.local/share/kiekje. Nothing leaves unless you switch it on.
          </p>
          <a className="kk-link" href={LINKS.privacy}>
            Privacy details
            <Icon name="arrow" size={14} />
          </a>
        </header>
        <ul ref={grid} className="kk-facts kk-drawn">
          {PRIVACY.map((item) => (
            <li key={item.title} className="kk-fact" data-art={item.art}>
              <Art name={item.art} />
              <h3>{item.title}</h3>
              <Body fact={item} />
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

import { useEffect, useRef, useState } from "react";

import { INSTALL_METHODS, LINKS, type Release } from "@/kiekje/content";
import { track } from "@/shared/helpers/track";

import { CopyButton } from "./copy-button";
import { HeroMark } from "./hero-mark";
import { Icon } from "./icons";

type Props = {
  release: Release;
};

const QUICK_INSTALL = INSTALL_METHODS[0];

export function Hero({ release }: Props) {
  const stage = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const scrub = useRef<HTMLDivElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const time = useRef<HTMLSpanElement>(null);
  const play = useRef<HTMLButtonElement>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    async function start() {
      await Promise.all([
        document.fonts.load("400 13px Geist"),
        document.fonts.load("500 13px Geist"),
        document.fonts.load("600 13px Geist"),
        document.fonts.load('400 12px "Geist Mono"'),
      ]).catch(() => undefined);
      await document.fonts.ready;
      const { startShowcase } = await import("@/kiekje/showcase");
      if (
        controller.signal.aborted ||
        !stage.current ||
        !canvas.current ||
        !scrub.current ||
        !bar.current
      )
        return null;
      if (!time.current || !play.current) return null;
      const stop = startShowcase(canvas.current, {
        stage: stage.current,
        scrub: scrub.current,
        bar: bar.current,
        time: time.current,
        play: play.current,
      });
      setReady(true);
      return stop;
    }
    const running = start();
    return () => {
      controller.abort();
      void running.then((stop) => stop?.());
    };
  }, []);

  return (
    <section className="kk-hero-section" aria-labelledby="hero-title">
      <div ref={stage} className="kk-stage" data-ready={ready}>
        <canvas ref={canvas} className="kk-canvas" aria-hidden="true" />
        <p className="kk-sr">
          Demo of a capture on a Linux desktop: the tray menu opens, the screen freezes, the mode
          bar cycles through screenshot, video, GIF and text extraction, a region is dragged over
          terminal output with a size label and a pixel loupe, an arrow and two numbered steps are
          drawn on it with the inline editor, and a notification says the image was copied and saved
          to Pictures.
        </p>
        <div className="kk-scrim" aria-hidden="true" />
        <div ref={scrub} className="kk-scrub" aria-hidden="true">
          <div ref={bar} className="kk-scrub-bar" />
          <span ref={time} className="kk-scrub-time" />
        </div>
        <button
          ref={play}
          type="button"
          className="kk-play"
          style={{ display: "none" }}
          aria-label="Play the demo"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
            <path d="M3 1.5v11l9.5-5.5z" fill="currentColor" />
          </svg>
          Play
        </button>
      </div>

      <div className="kk-hero">
        <a
          className="kk-release"
          href={release.url}
          onClick={() => track("release_open", { from: "hero" })}
        >
          <span className="kk-release-dot" aria-hidden="true" />
          <span>{release.version}</span>
          <span className="kk-release-label">Pre-release for Linux</span>
          <Icon name="arrow" size={14} />
        </a>
        <HeroMark />
        <p className="kk-tagline">
          Freeze the screen first, keep only the part you want.{" "}
          <span>A screenshot tray app for KDE Plasma and Hyprland.</span>
        </p>
        <div className="kk-actions">
          <a className="kk-btn kk-btn-primary" href="#install">
            Install
            <Icon name="arrow" size={16} />
          </a>
          <a
            className="kk-btn kk-btn-ghost"
            href={LINKS.repo}
            onClick={() => track("repo_open", { from: "hero" })}
          >
            <Icon name="github" size={18} />
            Source on GitHub
          </a>
        </div>
        {QUICK_INSTALL && (
          <div className="kk-quick">
            <span className="kk-quick-label">Arch</span>
            <code>{QUICK_INSTALL.commands}</code>
            <CopyButton text={QUICK_INSTALL.commands} method="arch-hero" />
          </div>
        )}
      </div>
    </section>
  );
}

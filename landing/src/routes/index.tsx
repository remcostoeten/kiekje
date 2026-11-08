import { createFileRoute } from "@tanstack/react-router";

import { Desktops } from "@/components/landing/desktops";
import { Features } from "@/components/landing/features";
import { Footer } from "@/components/landing/footer";
import { Hero } from "@/components/landing/hero";
import { Install } from "@/components/landing/install";
import { Keyboard } from "@/components/landing/keyboard";
import { Privacy } from "@/components/landing/privacy";
import { Steps } from "@/components/landing/steps";
import { Topbar } from "@/components/landing/topbar";
import { useLatestRelease } from "@/kiekje/release";

const TITLE = "Kiekje, a screenshot tool for Linux";
const DESCRIPTION =
  "A screenshot tray app for KDE Plasma and Hyprland on Wayland. It freezes the screen when you press the shortcut, then you select, annotate, copy and save.";

export const Route = createFileRoute("/")({
  head: () => ({
    meta: [
      { title: TITLE },
      { name: "description", content: DESCRIPTION },
      { property: "og:title", content: TITLE },
      { property: "og:description", content: DESCRIPTION },
      { name: "twitter:title", content: TITLE },
      { name: "twitter:description", content: DESCRIPTION },
    ],
  }),
  component: Index,
});

function Index() {
  const release = useLatestRelease();

  return (
    <>
      <a className="kk-skip" href="#how">
        Skip to content
      </a>
      <Topbar />
      <main>
        <Hero release={release} />
        <Steps />
        <Features />
        <Keyboard />
        <Desktops />
        <Privacy />
        <Install release={release} />
      </main>
      <Footer />
    </>
  );
}

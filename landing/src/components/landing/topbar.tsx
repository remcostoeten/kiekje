import { LINKS } from "@/kiekje/content";
import { track } from "@/shared/helpers/track";

import { Icon } from "./icons";

export function Topbar() {
  return (
    <header className="kk-topbar">
      <a className="kk-brand" href="/" aria-label="Kiekje home">
        <img src="/icon.png" width="28" height="28" alt="" />
        <span>Kiekje</span>
      </a>
      <nav className="kk-nav" aria-label="Main">
        <a href="#how">How it works</a>
        <a href="#features">Features</a>
        <a href="#desktops">Desktops</a>
        <a href="#install">Install</a>
        <a href={LINKS.docs}>Docs</a>
        <a
          className="kk-nav-icon"
          href={LINKS.repo}
          aria-label="Kiekje on GitHub"
          onClick={() => track("repo_open", { from: "topbar" })}
        >
          <Icon name="github" size={18} />
        </a>
      </nav>
    </header>
  );
}

import { EDITOR_KEYS, LINKS, OVERLAY_KEYS, type Key } from "@/kiekje/content";

import { Icon } from "./icons";

type Props = {
  title: string;
  keys: Key[];
};

function KeyList({ title, keys }: Props) {
  return (
    <div className="kk-keys">
      <h3>{title}</h3>
      <dl>
        {keys.map((item) => (
          <div key={item.action} className="kk-key-row">
            <dt>
              {item.keys.map((key) => (
                <kbd key={key} className="kk-kbd">
                  {key}
                </kbd>
              ))}
            </dt>
            <dd>{item.action}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export function Keyboard() {
  return (
    <section id="keyboard" className="kk-section" aria-labelledby="keyboard-title">
      <div className="kk-container">
        <header className="kk-section-head">
          <h2 id="keyboard-title">Every step works from the keyboard</h2>
          <p>
            A hint bar on the overlay lists the keys and hides with <kbd className="kk-kbd">?</kbd>.
            Shortcuts and tool letters can be changed in Settings.
          </p>
        </header>
        <div className="kk-keys-grid">
          <KeyList title="On the overlay" keys={OVERLAY_KEYS} />
          <KeyList title="In the editor" keys={EDITOR_KEYS} />
        </div>
        <a className="kk-link" href={LINKS.keys}>
          Every key
          <Icon name="arrow" size={14} />
        </a>
      </div>
    </section>
  );
}

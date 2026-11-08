import { COMMANDS, DESKTOPS, DOCTOR_OUTPUT, LINKS, type Support } from "@/kiekje/content";

import { Icon } from "./icons";

const SUPPORT_LABEL: Record<Support, string> = {
  lab: "Primary",
  partial: "Partial",
  unverified: "Unverified",
};

export function Desktops() {
  return (
    <section id="desktops" className="kk-section" aria-labelledby="desktops-title">
      <div className="kk-container">
        <header className="kk-section-head">
          <h2 id="desktops-title">Built for Plasma and Hyprland</h2>
          <p>
            Kiekje picks its capture backend at runtime from what your session offers and talks to
            KWin, the Wayland protocols or the desktop portal directly. Anything a desktop can't do
            is disabled in the app with the reason.
          </p>
        </header>

        <div className="kk-table-wrap">
          <table className="kk-table">
            <thead>
              <tr>
                <th scope="col">Desktop</th>
                <th scope="col">Capture</th>
                <th scope="col">Global shortcuts</th>
                <th scope="col">Status</th>
              </tr>
            </thead>
            <tbody>
              {DESKTOPS.map((desktop) => (
                <tr key={desktop.name}>
                  <th scope="row">
                    <span>{desktop.name}</span>
                    <span className="kk-badge" data-support={desktop.support}>
                      {SUPPORT_LABEL[desktop.support]}
                    </span>
                  </th>
                  <td data-label="Capture">{desktop.capture}</td>
                  <td data-label="Global shortcuts">{desktop.shortcuts}</td>
                  <td data-label="Status">{desktop.status}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="kk-footnote">
          Other X11 sessions have no capture backend yet. Kiekje is a pre-release: lab results come
          from virtual machines, and real-hardware runs on Plasma, Omarchy and GNOME are still open.{" "}
          <a className="kk-inline-link" href={LINKS.desktops}>
            Per-desktop setup
          </a>
        </p>

        <div className="kk-split">
          <div className="kk-terminal" aria-label="Output of kiekje doctor on Plasma 6">
            <pre>
              <code>{DOCTOR_OUTPUT}</code>
            </pre>
          </div>
          <div className="kk-cli">
            <h3>Scriptable from the terminal</h3>
            <p>
              <code>kiekje doctor</code> reports what your session supports. Every capture mode is
              also a command, and the running instance picks it up over D-Bus.
            </p>
            <dl>
              {COMMANDS.map((item) => (
                <div key={item.command} className="kk-cli-row">
                  <dt>
                    <code>{item.command}</code>
                  </dt>
                  <dd>{item.description}</dd>
                </div>
              ))}
            </dl>
            <a className="kk-link" href={LINKS.cli}>
              CLI reference
              <Icon name="arrow" size={14} />
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

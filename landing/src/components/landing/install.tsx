import { useRef, useState, type KeyboardEvent } from "react";

import { DOCTOR_SAMPLE, INSTALL_METHODS, LINKS, REPO, type Release } from "@/kiekje/content";
import { track } from "@/shared/helpers/track";

import { CopyButton } from "./copy-button";
import { Icon } from "./icons";
import { InstallMark } from "./install-mark";

type Props = {
  release: Release;
};

function fill(template: string, release: Release) {
  return template.replaceAll("{PACKAGE}", release.version.replaceAll("-", "."));
}

function nextTab(key: string, index: number, last: number) {
  switch (key) {
    case "ArrowRight":
    case "ArrowDown":
      return index === last ? 0 : index + 1;
    case "ArrowLeft":
    case "ArrowUp":
      return index === 0 ? last : index - 1;
    case "Home":
      return 0;
    case "End":
      return last;
    default:
      return -1;
  }
}

function breakAfterSlashes(text: string) {
  return text
    .split(/(?<=\/)/)
    .flatMap((part, index) => (index === 0 ? [part] : [<wbr key={index} />, part]));
}

export function Install({ release }: Props) {
  const [active, setActive] = useState(INSTALL_METHODS[0]?.id ?? "");
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);
  const method = INSTALL_METHODS.find((item) => item.id === active) ?? INSTALL_METHODS[0];
  const commands = method ? fill(method.commands, release) : "";
  const file = method?.file ? fill(method.file, release) : null;

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    const index = INSTALL_METHODS.findIndex((item) => item.id === active);
    const next = nextTab(event.key, index, INSTALL_METHODS.length - 1);
    const target = INSTALL_METHODS[next];
    if (!target) return;
    event.preventDefault();
    setActive(target.id);
    tabs.current[next]?.focus();
  }

  return (
    <section id="install" className="kk-section kk-get" aria-labelledby="install-title">
      <div className="kk-container kk-get-inner">
        <img className="kk-get-icon" src="/icon.png" width="76" height="76" alt="" />
        <h2 id="install-title" className="kk-get-title">
          Your desktop next.
        </h2>
        <p className="kk-get-sub">
          Version {release.version} is a <span className="kk-nowrap">pre-release</span>. It needs
          GTK 4.14 or newer.
        </p>

        <div className="kk-get-marks" role="tablist" aria-label="Install method">
          {INSTALL_METHODS.map((item, index) => (
            <button
              key={item.id}
              ref={(node) => {
                tabs.current[index] = node;
              }}
              type="button"
              role="tab"
              id={`install-tab-${item.id}`}
              aria-selected={item.id === active}
              aria-controls="install-panel"
              aria-label={item.label}
              title={item.label}
              tabIndex={item.id === active ? 0 : -1}
              className="kk-get-mark"
              onClick={() => setActive(item.id)}
              onKeyDown={onKeyDown}
            >
              <InstallMark mark={item.mark} />
            </button>
          ))}
        </div>
        <p className="kk-get-name" aria-hidden="true">
          {method?.label}
        </p>

        {method && (
          <div
            key={method.id}
            className="kk-get-panel"
            role="tabpanel"
            id="install-panel"
            aria-labelledby={`install-tab-${method.id}`}
          >
            <div className="kk-get-chip">
              <pre>
                <code>{breakAfterSlashes(commands)}</code>
              </pre>
              <CopyButton text={commands} method={method.id} />
            </div>
            <p className="kk-get-note">{method.note}</p>
            {file && (
              <a
                className="kk-get-file"
                href={`${REPO}/releases/download/v${release.version}/${file}`}
                onClick={() => track("release_open", { from: "install", file: method.id })}
              >
                <Icon name="download" size={16} />
                <span>Download {file}</span>
              </a>
            )}
          </div>
        )}

        <figure className="kk-doctor" aria-label="Trimmed output of kiekje doctor on a KDE session">
          <div className="kk-doctor-head">
            <span className="kk-doctor-cmd">
              <Icon name="terminal" size={16} />
              <code>kiekje doctor</code>
            </span>
            <span className="kk-doctor-where">{DOCTOR_SAMPLE.where}</span>
          </div>
          <div className="kk-doctor-body">
            <div>
              <p className="kk-doctor-group" id="doctor-session">
                Session
              </p>
              <dl className="kk-doctor-kv" aria-labelledby="doctor-session">
                {DOCTOR_SAMPLE.session.map(([key, value]) => (
                  <div key={key}>
                    <dt>{key}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
            </div>
            <div>
              <p className="kk-doctor-group" id="doctor-backends">
                Capture backends
              </p>
              <ul className="kk-doctor-backends" aria-labelledby="doctor-backends">
                {DOCTOR_SAMPLE.backends.map((backend) => (
                  <li key={backend.name} data-first={backend.first || undefined}>
                    <span className="kk-doctor-backend">
                      <i aria-hidden="true" />
                      <code>{backend.name}</code>
                    </span>
                    <span className="kk-doctor-role">{backend.role}</span>
                  </li>
                ))}
              </ul>
              <p className="kk-doctor-fallback">{DOCTOR_SAMPLE.fallback}</p>
            </div>
          </div>
          <div className="kk-doctor-result">
            <span className="kk-doctor-tick">
              <Icon name="check" size={18} />
            </span>
            {DOCTOR_SAMPLE.result}
          </div>
        </figure>

        <p className="kk-get-lead">
          Start it with <code>kiekje daemon</code>, then run <code>kiekje doctor</code> on your own
          session. Plasma needs <code>kiekje install-desktop-file</code> once.{" "}
          <a className="kk-inline-link" href={LINKS.start}>
            Getting started
          </a>
        </p>

        <nav className="kk-get-links" aria-label="Release links">
          <a href={release.url} onClick={() => track("release_open", { from: "install" })}>
            All files for {release.version}
            <Icon name="external" size={15} />
          </a>
          <a href={LINKS.install}>
            Install guide
            <Icon name="external" size={15} />
          </a>
          <a href={LINKS.notes}>
            Release notes
            <Icon name="external" size={15} />
          </a>
        </nav>
      </div>
    </section>
  );
}

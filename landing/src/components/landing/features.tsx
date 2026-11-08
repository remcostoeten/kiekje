import { FEATURES, MORE_FEATURES, SHOWCASES } from "@/kiekje/content";

import { Icon } from "./icons";

export function Features() {
  return (
    <section id="features" className="kk-section" aria-labelledby="features-title">
      <div className="kk-container">
        <header className="kk-section-head">
          <h2 id="features-title">One tray app from capture to share</h2>
          <p>
            After the selection the same app annotates, saves, lists and shares the capture, from
            the overlay or from the gallery.
          </p>
        </header>

        <div className="kk-rows">
          {SHOWCASES.map((item, index) => (
            <article
              key={item.id}
              className="kk-row"
              data-flip={index % 2 === 1}
              aria-labelledby={`${item.id}-title`}
            >
              <div className="kk-row-text">
                <h3 id={`${item.id}-title`}>{item.title}</h3>
                <p>{item.body}</p>
                <ul className="kk-points">
                  {item.points.map((point) => (
                    <li key={point}>{point}</li>
                  ))}
                </ul>
                <a className="kk-link" href={item.link.href}>
                  {item.link.label}
                  <Icon name="arrow" size={14} />
                </a>
              </div>
              <figure className="kk-shot">
                <img
                  src={item.image}
                  alt={item.alt}
                  width={item.width}
                  height={item.height}
                  loading="lazy"
                  decoding="async"
                />
              </figure>
            </article>
          ))}
        </div>

        <ul className="kk-grid" aria-label="More features">
          {FEATURES.map((feature) => (
            <li key={feature.title} className="kk-card">
              <span className="kk-card-icon">
                <Icon name={feature.icon} />
              </span>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </li>
          ))}
        </ul>

        <div className="kk-more">
          <h3>Also in Kiekje</h3>
          <div className="kk-more-grid">
            {MORE_FEATURES.map((group) => (
              <div key={group.title}>
                <h4>{group.title}</h4>
                <ul>
                  {group.items.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

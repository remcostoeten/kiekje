import { FOOTER, LINKS } from "@/kiekje/content";

export function Footer() {
  return (
    <footer className="kk-footer">
      <div className="kk-container kk-footer-inner">
        <div className="kk-footer-brand">
          <img src="/icon.png" width="32" height="32" alt="" />
          <div>
            <p className="kk-footer-name">Kiekje</p>
            <p className="kk-footer-meta">
              By Remco Stoeten. <a href={LINKS.license}>MIT license</a>, Geist and Geist Pixel under
              the OFL.
            </p>
          </div>
        </div>
        <nav className="kk-footer-cols" aria-label="Footer">
          {FOOTER.map((column) => (
            <div key={column.title}>
              <h2 className="kk-footer-title">{column.title}</h2>
              <ul>
                {column.links.map((link) => (
                  <li key={link.label}>
                    <a href={link.href}>{link.label}</a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>
      </div>
    </footer>
  );
}

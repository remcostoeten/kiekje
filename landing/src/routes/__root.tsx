import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRoute,
  useRouter,
  type ErrorComponentProps,
} from "@tanstack/react-router";
import { useEffect, type ReactNode } from "react";

import appCss from "../styles.css?url";
import { reportLovableError } from "../lib/lovable-error-reporting";

const OG_IMAGE =
  "https://raw.githubusercontent.com/remcostoeten/kiekje/master/docs/branding/social-preview/social-preview.png";

function NotFoundComponent() {
  return (
    <main className="kk-error">
      <img src="/icon.png" width="56" height="56" alt="" />
      <h1>Page not found</h1>
      <p>This page doesn't exist or has moved.</p>
      <a className="kk-btn kk-btn-primary" href="/">
        Go to Kiekje
      </a>
    </main>
  );
}

function ErrorComponent({ error, reset }: ErrorComponentProps) {
  const router = useRouter();

  useEffect(() => {
    console.error(error);
    reportLovableError(error, { boundary: "tanstack_root_error_component" });
  }, [error]);

  function retry() {
    void router.invalidate();
    reset();
  }

  return (
    <main className="kk-error">
      <img src="/icon.png" width="56" height="56" alt="" />
      <h1>This page didn't load</h1>
      <p>Something went wrong while rendering it. Try again, or go back to the start.</p>
      <div className="kk-error-actions">
        <button type="button" className="kk-btn kk-btn-primary" onClick={retry}>
          Try again
        </button>
        <a className="kk-btn kk-btn-ghost" href="/">
          Go to Kiekje
        </a>
      </div>
    </main>
  );
}

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { name: "theme-color", content: "#090908" },
      { name: "color-scheme", content: "dark" },
      { title: "Kiekje" },
      { name: "author", content: "Remco Stoeten" },
      { property: "og:site_name", content: "Kiekje" },
      { property: "og:type", content: "website" },
      { property: "og:image", content: OG_IMAGE },
      { property: "og:image:width", content: "2560" },
      { property: "og:image:height", content: "1280" },
      {
        property: "og:image:alt",
        content: "Kiekje: freeze the screen first, keep only the part you want.",
      },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:image", content: OG_IMAGE },
    ],
    links: [
      { rel: "stylesheet", href: appCss },
      { rel: "icon", href: "/favicon.png", type: "image/png" },
      { rel: "apple-touch-icon", href: "/icon.png" },
      {
        rel: "preload",
        href: "/fonts/geist-pixel-square.woff2",
        as: "font",
        type: "font/woff2",
        crossOrigin: "anonymous",
      },
      { rel: "preconnect", href: "https://fonts.googleapis.com" },
      { rel: "preconnect", href: "https://fonts.gstatic.com", crossOrigin: "anonymous" },
      {
        rel: "stylesheet",
        href: "https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500&display=swap",
      },
    ],
  }),
  shellComponent: RootShell,
  component: Outlet,
  notFoundComponent: NotFoundComponent,
  errorComponent: ErrorComponent,
});

function RootShell({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}

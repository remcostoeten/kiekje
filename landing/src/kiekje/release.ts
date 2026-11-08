import { useEffect, useState } from "react";

import { FALLBACK_RELEASE, REPO, type Release } from "./content";

type GithubRelease = {
  tag_name?: string;
  html_url?: string;
};

const TAG = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)$/;
const API = "https://api.github.com/repos/remcostoeten/kiekje/releases?per_page=1";

async function fetchLatestRelease(signal: AbortSignal): Promise<Release | null> {
  const response = await fetch(API, { signal, headers: { accept: "application/vnd.github+json" } });
  if (!response.ok) return null;
  const releases: GithubRelease[] = await response.json();
  const newest = releases[0];
  const match = TAG.exec(newest?.tag_name ?? "");
  if (!match?.[1]) return null;
  return {
    version: match[1],
    url: newest?.html_url?.startsWith(REPO) ? newest.html_url : FALLBACK_RELEASE.url,
  };
}

/**
 * @name useLatestRelease
 * @description Returns the newest Kiekje release, pre-releases included. Renders the
 * bundled fallback first and swaps in the GitHub answer once it arrives, so the page
 * never shows a stale version for longer than one request.
 *
 * @example
 * const release = useLatestRelease();
 * return <a href={release.url}>{release.version}</a>;
 */
export function useLatestRelease(): Release {
  const [release, setRelease] = useState(FALLBACK_RELEASE);

  useEffect(() => {
    const controller = new AbortController();
    fetchLatestRelease(controller.signal)
      .then((latest) => {
        if (latest) setRelease(latest);
      })
      .catch(() => {
        // The bundled fallback stays when GitHub is unreachable or rate limited.
      });
    return () => controller.abort();
  }, []);

  return release;
}

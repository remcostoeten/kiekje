import { createRouter, rootRouteId } from "@tanstack/react-router";
import { describe, expect, it } from "vitest";

import { INSTALL_METHODS, FALLBACK_RELEASE } from "@/kiekje/content";
import { routeTree } from "@/routeTree.gen";

describe("App routing", () => {
  it("matches a page for / instead of falling back to not found", () => {
    const router = createRouter({ routeTree });

    const matches = router.matchRoutes("/");

    expect(matches.at(-1)?.routeId).not.toBe(rootRouteId);
  });
});

describe("Install content", () => {
  it("gives every install method a unique id", () => {
    const ids = INSTALL_METHODS.map((method) => method.id);

    expect(new Set(ids).size).toBe(ids.length);
  });

  it("keeps the fallback release on a semver tag", () => {
    expect(FALLBACK_RELEASE.url).toContain(`/tag/v${FALLBACK_RELEASE.version}`);
  });
});

type Event = "install_copy" | "release_open" | "repo_open";

/**
 * @name track
 * @description Records a landing event through Lovable's analytics when the host
 * injected it, and does nothing otherwise. Feeds the weekly downloads north star.
 *
 * @example
 * track("install_copy", { method: "arch" });
 */
export function track(event: Event, properties: Record<string, string> = {}) {
  window.__lovableEvents?.track?.(event, properties);
}

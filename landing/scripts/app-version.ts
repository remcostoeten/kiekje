import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const CARGO = `${ROOT}Cargo.toml`;
// The first top-level `version = "..."` line, which is the [package] version.
const VERSION_LINE = /^version\s*=\s*"([^"]+)"/m;

function fromCargo() {
  if (!existsSync(CARGO)) return null;
  return VERSION_LINE.exec(readFileSync(CARGO, "utf8"))?.[1] ?? null;
}

function fromGitTag() {
  try {
    const tag = execFileSync("git", ["describe", "--tags", "--abbrev=0", "--match", "v*"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    return tag.trim().replace(/^v/, "");
  } catch (error) {
    throw new Error("No app version: ../Cargo.toml is missing and git has no v* tag", {
      cause: error,
    });
  }
}

/**
 * @name appVersion
 * @description Reads the Kiekje version at build time from the app's Cargo.toml, falling
 * back to the newest `v*` git tag, so the landing page's bundled release never needs a
 * manual bump.
 *
 * @example
 * define: { KIEKJE_APP_VERSION: JSON.stringify(appVersion()) }
 */
export function appVersion() {
  return fromCargo() ?? fromGitTag();
}

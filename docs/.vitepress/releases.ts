import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const releasesDir = fileURLToPath(new URL("../releases", import.meta.url));
const cargoToml = fileURLToPath(new URL("../../Cargo.toml", import.meta.url));

type VersionParts = {
  numbers: number[];
  prerelease: string[];
};

function parseVersion(version: string): VersionParts {
  const [release, prerelease = ""] = version.split("-", 2);
  return {
    numbers: release.split(".").map(Number),
    prerelease: prerelease === "" ? [] : prerelease.split("."),
  };
}

function comparePart(left: string, right: string): number {
  const leftNumber = Number(left);
  const rightNumber = Number(right);
  if (!Number.isNaN(leftNumber) && !Number.isNaN(rightNumber)) {
    return leftNumber - rightNumber;
  }
  return left.localeCompare(right);
}

/**
 * @name compareVersions
 * @description Orders two semver strings ascending, with a prerelease sorting before its release.
 *
 * @example
 * ['0.1.0', '0.1.0-alpha.2', '0.1.0-alpha.10'].sort(compareVersions)
 */
export function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  for (let index = 0; index < Math.max(a.numbers.length, b.numbers.length); index += 1) {
    const difference = (a.numbers[index] ?? 0) - (b.numbers[index] ?? 0);
    if (difference !== 0) {
      return difference;
    }
  }
  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    return b.prerelease.length - a.prerelease.length;
  }
  for (let index = 0; index < Math.max(a.prerelease.length, b.prerelease.length); index += 1) {
    const difference = comparePart(a.prerelease[index] ?? "", b.prerelease[index] ?? "");
    if (difference !== 0) {
      return difference;
    }
  }
  return 0;
}

/**
 * @name releaseVersions
 * @description Lists the versions that have a release notes file, newest first.
 *
 * @example
 * releaseVersions().map((version) => ({ text: version, link: `/releases/${version}` }))
 */
export function releaseVersions(): string[] {
  return readdirSync(releasesDir)
    .filter((name) => name.endsWith(".md") && name !== "index.md")
    .map((name) => name.slice(0, -".md".length))
    .sort(compareVersions)
    .reverse();
}

/**
 * @name crateVersion
 * @description Reads the version field from the workspace Cargo.toml.
 *
 * @example
 * const label = `v${crateVersion()}`
 */
export function crateVersion(): string {
  // Matches the first `version = "..."` line, which is the package version in Cargo.toml.
  const match = /^version = "([^"]+)"/m.exec(readFileSync(cargoToml, "utf8"));
  return match?.[1] ?? "unknown";
}

---
title: Releasing
description: How a tagged release is built, what it publishes, and what the repository needs for the AUR step.
---

# Releasing

Releases are pre-release alphas from GitHub Releases. The release workflow runs on a `v*` tag and can be re-run by hand to replace the assets of an existing tag.

## Cutting a release

1. Bump `version` in `Cargo.toml` and `Cargo.lock`, and the PKGBUILDs under `packaging/`.
2. Write `docs/releases/<version>.md`. The file starts with a frontmatter block and a heading, which the workflow strips before the text becomes the GitHub release body:

   ```md
   ---
   title: 0.1.0-alpha.7
   date: 2026-10-20
   ---

   # 0.1.0-alpha.7

   Seventh alpha. One sentence on what it brings.

   ## Section

   - Change.

   ## Tested

   What ran where, and what is unverified.
   ```

   Without the file, the tag's annotation becomes the body.
3. Merge, then tag and push:

   ```bash
   git tag -a v0.1.0-alpha.7 -m "kiekje 0.1.0-alpha.7"
   git push origin v0.1.0-alpha.7
   ```

## What the workflow does

| Job | Output |
| --- | --- |
| `version` | Checks that the tag matches `Cargo.toml` |
| `portable` | Builds x86_64 and aarch64 binaries with gtk4-layer-shell linked statically, then a tarball, a `.deb` and an `.rpm` for each, and installs and starts them on Arch, Fedora, openSUSE Tumbleweed, Debian 13, Ubuntu 25.10 and Ubuntu 24.04 |
| `release` | Writes `SHA256SUMS`, appends the install block to the notes, creates the GitHub release and attaches the two PKGBUILDs |
| `aur` | Pushes `kiekje` and `kiekje-bin` to the AUR |
| `flatpak` | Builds `kiekje.flatpak` from `dev.remcostoeten.Kiekje.json` and attaches it |

A tag with a `-` in it is marked as a pre-release.

## AUR publishing

Every Release run publishes both packages after the GitHub release completes: a pushed `v*` tag, or running the Release workflow by hand. Running it with `republish` on an existing tag rebuilds the assets and publishes that version again; the AUR push is skipped when the PKGBUILDs did not change.

The `aur` job needs an `AUR_SSH_PRIVATE_KEY` Actions secret holding the private key whose public half is registered on the AUR account that owns `kiekje` and `kiekje-bin`. The key is used only by that job to clone and push the two AUR repositories. `packaging/aur/publish.sh` does the push and `packaging/aur/bump.sh` rewrites the PKGBUILDs with the new version and checksums.

## Documentation

This site deploys from the `docs` workflow on every push to `master` that touches `docs/`. It builds with pnpm and publishes `docs/.vitepress/dist` to GitHub Pages. The version shown in the navigation comes from `Cargo.toml` at build time, and the release notes in the sidebar come from the files in `docs/releases/`.

---
title: Contributing
description: Bug reports, desktop test results, pull requests and the checks CI runs.
---

# Contributing

Bug reports, desktop test results and pull requests are welcome. The most useful contribution right now is a run on real hardware: Omarchy, Plasma or GNOME on a physical machine, with the `kiekje doctor` output and what worked. That is [issue #41](https://github.com/remcostoeten/kiekje/issues/41).

## Reporting a bug

Open an issue with the bug report form. Include the output of `kiekje doctor`, your desktop and its version, and the steps that trigger the bug. Run `kiekje daemon` from a terminal to capture its log.

## Development

Needs Rust 1.85 or newer (edition 2024), GTK 4.14 and gtk4-layer-shell 1.1.

```bash
cargo build                                                                     # debug build
cargo run -- doctor                                                             # what this session supports
cargo test                                                                      # unit tests
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings     # what CI checks
```

CI runs fmt, clippy and test. All three must pass before a change is done.

`lab/kiekje-lab` drives the same checks on VMs and headless sessions and writes each run to `lab-results/<os>_<desktop>-<version>_<virt>/<timestamp>/`. The [lab results](/project/lab-results#lab-cli) page lists its commands.

## Pull requests

1. Read `AGENTS.md` in the repository for the stack, layout, architecture rules and code style, and the [rewrite proposal](/project/proposal) before changing capture, overlay, shortcuts or tray behavior. The [architecture page](/project/architecture) is the short version.
2. Keep each pull request to one change, with a Conventional Commit title such as `fix: keep the selection inside the monitor`.
3. Run what CI runs.
4. In the description, state what changed, why, and which desktops you tested on (KDE, Hyprland, GNOME, Sway). Label anything you could not test as unverified. Attach screenshots for UI changes.

## Code style in short

- No comments except a one-line workaround reason, a one-line explanation of an opaque expression or regex, or a `TODO`. Public functions in shared modules get a rustdoc block with a one-line summary and an `# Examples` section.
- `thiserror` enums per module for errors callers branch on, `anyhow` only at the app edge. Never detect an error by its message text.
- No `unwrap` or `expect` outside tests and startup invariants. No empty error handling.
- Finite states are enums, never strings.
- Unit tests sit inline under `#[cfg(test)]` and are named by behavior, such as `maps_selection_across_fractional_scale`.

## This site

The documentation is a VitePress site in `docs/`:

```bash
cd docs
pnpm install
pnpm dev        # http://localhost:5173/kiekje/
pnpm build      # docs/.vitepress/dist
```

Pages are Markdown under `docs/guide`, `docs/reference`, `docs/project` and `docs/releases`. The sidebar is in `docs/.vitepress/config.ts`. A push to `master` that touches `docs/` deploys the site to GitHub Pages.

Contributions are released under the [MIT license](https://github.com/remcostoeten/kiekje/blob/master/LICENSE). See `SECURITY.md` in the repository to report a vulnerability.

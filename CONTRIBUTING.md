# Contributing

Bug reports, desktop test results and pull requests are welcome.

## Reporting a bug

Open an issue with the bug report form. Include the output of `kiekje doctor`, your desktop and its version, and the steps that trigger the bug. Run `kiekje daemon` from a terminal to capture its log.

## Pull requests

1. Read [AGENTS.md](AGENTS.md) for the stack, layout, architecture rules and code style. Read [docs/proposal.md](docs/proposal.md) before changing capture, overlay, shortcuts or tray behavior.
2. Keep each pull request to one change, with a Conventional Commit title such as `fix: keep the selection inside the monitor`.
3. Run what CI runs:

   ```bash
   cargo fmt --all
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```

4. In the description, state what changed, why, and which desktops you tested on (KDE, Hyprland, GNOME, Sway). Label anything you could not test as unverified. Attach screenshots for UI changes.

Contributions are released under the [MIT license](LICENSE).

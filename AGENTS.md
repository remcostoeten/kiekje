# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the main Rust application (`capture-app`). Key areas are `src/capture` for capture modes, `src/platform/linux` for Wayland and Hyprland integrations, `src/editor` for annotation tools, `src/settings` for persisted config, and `src/app` for the GTK/libadwaita shell. The Go Bubble Tea helper lives in [`cmd/screeny-tui`](./cmd/screeny-tui), with the compiled binary typically written to `bin/`. Shell utilities are in `scripts/`, reference docs in `docs/`, and CI/PR templates in `.github/`.

## Build, Test, and Development Commands
Use Cargo for the Rust app and Go tooling for the TUI helper:

- `cargo build` builds the default debug binary.
- `cargo build --release` produces `./target/release/capture-app`.
- `cargo run -- --doctor` runs the dependency diagnostics flow locally.
- `cargo test` runs Rust unit tests embedded in module files.
- `cargo fmt --all` formats Rust code.
- `cargo clippy --all-targets --all-features -- -D warnings` enforces Rust lint cleanliness.
- `go build -o ./bin/screeny-tui ./cmd/screeny-tui` builds the TUI helper.
- `(cd cmd/screeny-tui && go test ./...)` runs Go tests.
- `(cd cmd/screeny-tui && go vet ./...)` performs Go static checks.

## Coding Style & Naming Conventions
Follow `.editorconfig`: UTF-8, LF endings, trailing whitespace trimmed, and final newlines enabled. Use 4 spaces in Rust and Go files. Keep Rust modules small and domain-focused; prefer `snake_case` for files, modules, functions, and variables. Use `CamelCase` for Rust structs/enums and Go exported types. Run `cargo fmt` and `gofmt` before opening a PR.

## Testing Guidelines
Rust tests are inline under `#[cfg(test)]` blocks near the code they validate, for example in `src/settings/config.rs` and `src/platform/linux/hyprctl.rs`. Add focused unit tests for parser behavior, save-path logic, and error recovery when changing those flows. Name tests descriptively by behavior, such as `parses_active_window_geometry`. Run both `cargo test` and `(cd cmd/screeny-tui && go test ./...)` before submitting.

## Commit & Pull Request Guidelines
Recent history favors short, imperative messages with Conventional Commit prefixes, for example `feat: add recovery workflows to TUI and bash menu` and `fix: handle editor startup failures without panic`. Keep commits focused. PRs should include a short summary, the reason for the change, test results, and doc updates when behavior changes. Follow the PR template and attach screenshots or terminal output when UI, editor, or recovery flows are affected.

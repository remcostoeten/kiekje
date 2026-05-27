# Releasing Kiekje

Kiekje uses [git-cliff](https://git-cliff.org) for changelog generation and [Semantic Versioning](https://semver.org).

## Prerequisites

Install git-cliff locally (once):

```bash
cargo install git-cliff --locked
# or on Arch: sudo pacman -S git-cliff
```

## Commit message format

Use [Conventional Commits](https://www.conventionalcommits.org) so version bumps are automatic:

| Prefix | Release impact |
| --- | --- |
| `feat:` | minor (`0.1.0` -> `0.2.0`) |
| `fix:` | patch (`0.1.0` -> `0.1.1`) |
| `feat!:` or `BREAKING CHANGE:` | major (`0.1.0` -> `1.0.0`) |
| `chore:`, `docs:`, `refactor:` | patch unless configured otherwise |

Legacy commits without a prefix are grouped under **Changes** and still appear in the changelog.

## Local release flow

Preview the next version and notes:

```bash
cd cheese-wails
./scripts/release-prepare.sh bump
./scripts/release-prepare.sh notes
```

Create the release commit, updated changelog, and annotated tag:

```bash
./scripts/release-prepare.sh prepare --bump auto
# or force a bump level:
./scripts/release-prepare.sh prepare --bump minor

# dry run:
./scripts/release-prepare.sh prepare --bump auto --dry-run
```

Push the tag to trigger the GitHub release workflow:

```bash
git push origin HEAD v0.1.0
```

Shortcut via install script menu:

```bash
./kiekje.sh release
```

## GitHub Actions

Two workflows live in `.github/workflows/`:

1. **Prepare Release** (`release-prepare.yml`) — manual dispatch to bump version, update `CHANGELOG.md`, commit, and tag. Optionally pushes to origin.
2. **Publish Release** (`release-publish.yml`) — runs on every `v*` tag push, builds the Linux tarball with `./kiekje.sh ship`, and publishes a GitHub Release with git-cliff notes (no emojis).

## Files updated on release

- `cheese-wails/VERSION`
- `cheese-wails/CHANGELOG.md`
- `cheese-wails/wails.json` (`info.productVersion`)
- `cheese-wails/frontend/package.json` (`version`)

## Distribution artifact

`./kiekje.sh ship` writes:

```text
dist/kiekje-linux-<arch>.tar.gz
```

Contents: `kiekje`, `kiekje-tray`, icon, `install.sh`, and `README.md`.

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(git -C "$ROOT_DIR" rev-parse --show-toplevel)"
CONFIG="$ROOT_DIR/cliff.toml"
VERSION_FILE="$ROOT_DIR/VERSION"
INCLUDE_PATH="cheese-wails/"

require_git_cliff() {
  if command -v git-cliff >/dev/null 2>&1; then
    return 0
  fi
  if command -v cargo >/dev/null 2>&1; then
    echo "Installing git-cliff via cargo..."
    cargo install git-cliff --locked
    return 0
  fi
  echo "git-cliff is required. Install it with: cargo install git-cliff" >&2
  exit 1
}

usage() {
  cat <<EOF
Usage: ./scripts/release-prepare.sh [command] [options]

Commands:
  notes         Print release notes for unreleased commits
  bump          Print the next semantic version (auto-detected)
  prepare       Bump version files, update CHANGELOG, commit, and tag
  changelog     Regenerate the full CHANGELOG.md

Options for prepare:
  --bump auto|patch|minor|major   Version bump strategy (default: auto)
  --dry-run                       Print actions without writing or committing
  --push                          Push commit and tag to origin after prepare

Examples:
  ./scripts/release-prepare.sh bump
  ./scripts/release-prepare.sh prepare --bump auto --push
  ./scripts/release-prepare.sh prepare --bump minor --dry-run
EOF
}

read_version() {
  tr -d '[:space:]' < "$VERSION_FILE"
}

write_version() {
  printf '%s\n' "$1" > "$VERSION_FILE"
}

bump_manual() {
  local current="$1"
  local kind="$2"
  local major minor patch
  IFS='.' read -r major minor patch <<< "${current#v}"

  case "$kind" in
    major)
      major=$((major + 1))
      minor=0
      patch=0
      ;;
    minor)
      minor=$((minor + 1))
      patch=0
      ;;
    patch)
      patch=$((patch + 1))
      ;;
    *)
      echo "Unknown bump kind: $kind" >&2
      exit 1
      ;;
  esac

  printf '%s.%s.%s' "$major" "$minor" "$patch"
}

next_version() {
  local strategy="${1:-auto}"
  local current
  current="$(read_version)"

  if [[ "$strategy" == "auto" ]]; then
    git-cliff \
      --config "$CONFIG" \
      --include-path "$INCLUDE_PATH" \
      --unreleased \
      --bumped-version
    return
  fi

  bump_manual "$current" "$strategy"
}

sync_version_files() {
  local version="$1"
  local tag="v${version}"

  write_version "$version"

  if command -v python3 >/dev/null 2>&1; then
    python3 - "$ROOT_DIR/wails.json" "$version" <<'PY'
import json
import sys

path, version = sys.argv[1], sys.argv[2]
with open(path, "r", encoding="utf-8") as handle:
    data = json.load(handle)
data["info"] = {
    "companyName": "Remco Stoeten",
    "productName": "Kiekje",
    "productVersion": version,
    "copyright": f"Copyright (c) Remco Stoeten",
}
with open(path, "w", encoding="utf-8") as handle:
    json.dump(data, handle, indent=2)
    handle.write("\n")
PY

    python3 - "$ROOT_DIR/frontend/package.json" "$version" <<'PY'
import json
import sys

path, version = sys.argv[1], sys.argv[2]
with open(path, "r", encoding="utf-8") as handle:
    data = json.load(handle)
data["version"] = version
with open(path, "w", encoding="utf-8") as handle:
    json.dump(data, handle, indent=2)
    handle.write("\n")
PY
  fi

  echo "$tag"
}

generate_changelog() {
  git-cliff \
    --config "$CONFIG" \
    --include-path "$INCLUDE_PATH" \
    --output "$ROOT_DIR/CHANGELOG.md"
}

generate_changelog_with_tag() {
  local tag="$1"
  git-cliff \
    --config "$CONFIG" \
    --include-path "$INCLUDE_PATH" \
    --tag "$tag" \
    --output "$ROOT_DIR/CHANGELOG.md"
}

latest_tag() {
  git -C "$REPO_ROOT" describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true
}

generate_release_notes() {
  local version="${1:-}"
  local tag="${2:-}"

  if [[ -z "$tag" && -n "$version" ]]; then
    tag="v${version#v}"
  fi

  if [[ -n "$(latest_tag)" ]]; then
    git-cliff \
      --config "$CONFIG" \
      --include-path "$INCLUDE_PATH" \
      --unreleased \
      --strip header
    return
  fi

  git-cliff \
    --config "$CONFIG" \
    --include-path "$INCLUDE_PATH" \
    --tag "$tag" \
    --strip header
}

prepare_release() {
  local strategy="auto"
  local dry_run=0
  local push=0

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --bump)
        strategy="${2:-auto}"
        shift 2
        ;;
      --dry-run)
        dry_run=1
        shift
        ;;
      --push)
        push=1
        shift
        ;;
      *)
        echo "Unknown option: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
  done

  require_git_cliff

  local version tag notes
  version="$(next_version "$strategy")"
  version="${version#v}"
  tag="v${version}"

  if git -C "$REPO_ROOT" rev-parse "$tag" >/dev/null 2>&1; then
    echo "Tag already exists: $tag" >&2
    exit 1
  fi

  echo "Next version: $version"
  if [[ "$dry_run" -eq 1 ]]; then
    echo "Dry run only. Release notes preview:"
    generate_release_notes "$version" "$tag"
    exit 0
  fi

  sync_version_files "$version"
  generate_changelog_with_tag "$tag"

  notes="$(mktemp)"
  generate_release_notes "$version" "$tag" > "$notes"

  git -C "$REPO_ROOT" add \
    "$VERSION_FILE" \
    "$ROOT_DIR/CHANGELOG.md" \
    "$ROOT_DIR/wails.json" \
    "$ROOT_DIR/frontend/package.json"

  git -C "$REPO_ROOT" commit -m "chore(release): prepare for ${tag}"

  git -C "$REPO_ROOT" tag -a "$tag" -F "$notes"
  rm -f "$notes"

  echo "Created commit and annotated tag ${tag}"
  echo "Push with: git push origin HEAD ${tag}"

  if [[ "$push" -eq 1 ]]; then
    git -C "$REPO_ROOT" push origin HEAD "$tag"
  fi
}

main() {
  local command="${1:-}"
  shift || true

  case "$command" in
    notes)
      require_git_cliff
      version="$(read_version)"
      generate_release_notes "$version" "v${version}"
      ;;
    bump)
      require_git_cliff
      next_version "${1:-auto}" | sed 's/^v//'
      ;;
    changelog)
      require_git_cliff
      generate_changelog
      echo "Updated $ROOT_DIR/CHANGELOG.md"
      ;;
    prepare)
      prepare_release "$@"
      ;;
    -h|--help|help|"")
      usage
      ;;
    *)
      echo "Unknown command: $command" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"

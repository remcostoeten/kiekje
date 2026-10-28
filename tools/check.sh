#!/usr/bin/env bash
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REPO

function usage() {
    printf 'usage: tools/check.sh [--full | --install-hook]\n'
    printf '  (none)          Cargo.lock is current, flatpak sources match it, rustfmt is clean\n'
    printf '  --full          the above plus clippy and the tests, two jobs at a time\n'
    printf '  --install-hook  run the quick checks before every git push\n'
}

function quick() {
    cd "$REPO"
    cargo metadata --locked --format-version 1 >/dev/null
    python3 -I packaging/flatpak/check-sources.py
    cargo fmt --all -- --check
}

function full() {
    quick
    cargo clippy -j 2 --locked --all-targets --all-features -- -D warnings
    cargo test -j 2 --locked -- --test-threads=2
}

function install_hook() {
    local hook="$REPO/.git/hooks/pre-push"
    [[ ! -e "$hook" ]] || { printf 'tools/check.sh: %s already exists, not overwriting\n' "$hook" >&2; exit 1; }
    printf '#!/usr/bin/env bash\nexec "%s/tools/check.sh"\n' "$REPO" > "$hook"
    chmod +x "$hook"
    printf 'installed %s\n' "$hook"
}

case "${1:-}" in
    "") quick ;;
    --full) full ;;
    --install-hook) install_hook ;;
    -h | --help) usage ;;
    *) usage >&2; exit 2 ;;
esac

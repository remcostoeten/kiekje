#!/usr/bin/env bash
# Installs a release build into DESTDIR/PREFIX. Usage: packaging/install.sh [binary]
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
binary="${1:-$repo/target/release/kiekje}"
prefix="${PREFIX:-/usr}"
root="${DESTDIR:-}$prefix"

[[ -x "$binary" ]] || { echo "install.sh: no executable at $binary; run cargo build --release" >&2; exit 1; }

install -Dm755 "$binary" "$root/bin/kiekje"
install -Dm644 "$repo/packaging/dev.remcostoeten.Kiekje.desktop" "$root/share/applications/dev.remcostoeten.Kiekje.desktop"
for icon in "$repo"/resources/icons/hicolor/*/apps/*; do
    install -Dm644 "$icon" "$root/share/icons/hicolor/${icon#"$repo/resources/icons/hicolor/"}"
done
install -Dm644 "$repo/LICENSE" "$root/share/licenses/kiekje/LICENSE"
install -Dm644 "$repo/resources/fonts/OFL.txt" "$root/share/licenses/kiekje/OFL-fonts.txt"

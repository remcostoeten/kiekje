#!/usr/bin/env bash
# Writes both PKGBUILDs for a release into OUT. Usage: packaging/aur/bump.sh VERSION OUT SOURCE_TARBALL X86_64_TARBALL [AARCH64_TARBALL]
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
version="$1"
out="$2"
source_tarball="$3"
x86_64_tarball="$4"
aarch64_tarball="${5:-}"

pkgver="$version"
if [[ "$version" == *-* ]]; then
    pre="${version#*-}"
    pkgver="${version%%-*}${pre//./}"
fi
sum() { sha256sum "$1" | cut -d' ' -f1; }

mkdir -p "$out/kiekje" "$out/kiekje-bin"

sed -e "s/^pkgver=.*/pkgver=$pkgver/" \
    -e "s/^_tag=.*/_tag=$version/" \
    -e "s/^sha256sums=.*/sha256sums=('$(sum "$source_tarball")')/" \
    "$repo/packaging/aur/PKGBUILD" > "$out/kiekje/PKGBUILD"

aarch64_sum="SKIP"
[[ -n "$aarch64_tarball" ]] && aarch64_sum="$(sum "$aarch64_tarball")"
sed -e "s/^pkgver=.*/pkgver=$pkgver/" \
    -e "s/^_tag=.*/_tag=$version/" \
    -e "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('$(sum "$x86_64_tarball")')/" \
    -e "s/^sha256sums_aarch64=.*/sha256sums_aarch64=('$aarch64_sum')/" \
    "$repo/packaging/aur-bin/PKGBUILD" > "$out/kiekje-bin/PKGBUILD"

#!/usr/bin/env bash
# Installs the latest Kiekje release. Usage: curl -fsSL https://raw.githubusercontent.com/remcostoeten/kiekje/master/get.sh | bash
# KIEKJE_VERSION=v0.1.0-alpha.4 pins a tag. KIEKJE_FORMAT=tarball skips the native package.
set -euo pipefail

repo="remcostoeten/kiekje"
api="https://api.github.com/repos/$repo/releases"

fail() { echo "kiekje: $*" >&2; exit 1; }

for tool in curl sha256sum tar; do
    command -v "$tool" >/dev/null || fail "needs $tool"
done

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 deb_arch=amd64 ;;
    aarch64 | arm64) arch=aarch64 deb_arch=arm64 ;;
    *) fail "no build for $(uname -m)" ;;
esac

if [[ -n "${KIEKJE_VERSION:-}" ]]; then
    release="$(curl -fsSL "$api/tags/$KIEKJE_VERSION")" || fail "no release $KIEKJE_VERSION"
else
    # The list includes pre-releases, which /releases/latest skips.
    release="$(curl -fsSL "$api?per_page=1")" || fail "cannot reach GitHub releases"
fi
urls="$(grep -o '"browser_download_url": *"[^"]*"' <<<"$release" | sed 's/.*"\(https[^"]*\)"/\1/')"
[[ -n "$urls" ]] || fail "release has no assets"

asset() { grep -E "/$1\$" <<<"$urls" | head -1; }

format="${KIEKJE_FORMAT:-}"
if [[ -z "$format" ]]; then
    if command -v apt-get >/dev/null && command -v dpkg >/dev/null; then
        format=deb
    elif command -v dnf >/dev/null || command -v zypper >/dev/null; then
        format=rpm
    else
        format=tarball
    fi
fi

case "$format" in
    deb) url="$(asset "kiekje_[^/]*_${deb_arch}\.deb")" ;;
    rpm) url="$(asset "kiekje-[^/]*\.${arch}\.rpm")" ;;
    tarball) url="$(asset "kiekje-[^/]*-${arch}-linux\.tar\.gz")" ;;
    *) fail "unknown format $format" ;;
esac
[[ -n "$url" ]] || fail "release has no $format for $arch"
sums="$(asset SHA256SUMS)"
[[ -n "$sums" ]] || fail "release has no SHA256SUMS"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
file="${url##*/}"

echo "kiekje: downloading $file"
curl -fsSL -o "$work/$file" "$url"
curl -fsSL -o "$work/SHA256SUMS" "$sums"
(cd "$work" && grep " $file\$" SHA256SUMS | sha256sum -c --quiet -) || fail "checksum mismatch for $file"

sudo=""
[[ "$(id -u)" -eq 0 ]] || sudo="sudo"

case "$format" in
    deb) $sudo apt-get install -y "$work/$file" ;;
    rpm)
        if command -v dnf >/dev/null; then
            $sudo dnf install -y "$work/$file"
        else
            $sudo zypper --non-interactive install --allow-unsigned-rpm "$work/$file"
        fi
        ;;
    tarball)
        tar -xzf "$work/$file" -C "$work"
        dir="$work/${file%.tar.gz}"
        $sudo "$dir/packaging/install.sh" "$dir/kiekje"
        ;;
esac

echo "kiekje: installed. Run kiekje doctor to check your desktop."

#!/usr/bin/env bash
set -euo pipefail

version="${1:?usage: publish.sh VERSION SOURCE_TARBALL X86_64_TARBALL AARCH64_TARBALL}"
source_tarball="${2:?usage: publish.sh VERSION SOURCE_TARBALL X86_64_TARBALL AARCH64_TARBALL}"
x86_64_tarball="${3:?usage: publish.sh VERSION SOURCE_TARBALL X86_64_TARBALL AARCH64_TARBALL}"
aarch64_tarball="${4:?usage: publish.sh VERSION SOURCE_TARBALL X86_64_TARBALL AARCH64_TARBALL}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

"$repo_root/packaging/aur/bump.sh" "$version" "$work/generated" "$source_tarball" "$x86_64_tarball" "$aarch64_tarball"

for package in kiekje kiekje-bin; do
    package_dir="$work/$package"
    git clone "ssh://aur@aur.archlinux.org/$package.git" "$package_dir"
    git -C "$package_dir" pull --ff-only origin master
    install -Dm644 "$work/generated/$package/PKGBUILD" "$package_dir/PKGBUILD"
    (
        cd "$package_dir"
        makepkg --verifysource --noconfirm
        makepkg --printsrcinfo > .SRCINFO
        if git diff --quiet -- PKGBUILD .SRCINFO; then
            exit 0
        fi
        git add PKGBUILD .SRCINFO
        git commit -m "Update to $version"
        git push origin HEAD:master
    )
done


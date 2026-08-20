#!/usr/bin/env bash
# Sync the AUR package with the released version: copy the PKGBUILD, pin
# the real tarball checksum, regenerate .SRCINFO and commit in the AUR
# clone. Run after the release tag is pushed (the checksum needs the
# tarball to exist). Pushing to the AUR remains the operator's move.
set -euo pipefail

REPO=$(git rev-parse --show-toplevel)
AUR_DIR=${AUR_DIR:-$REPO/../aur-riso}
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO/Cargo.toml")

if [ ! -d "$AUR_DIR/.git" ]; then
  git clone "ssh://aur@aur.archlinux.org/riso.git" "$AUR_DIR"
fi

TARBALL=$(mktemp)
trap 'rm -f "$TARBALL"' EXIT
curl -fsSL -o "$TARBALL" "https://github.com/eldios/riso/archive/v$VERSION.tar.gz"
SHA=$(sha256sum "$TARBALL" | cut -d' ' -f1)

cp "$REPO/packaging/PKGBUILD" "$AUR_DIR/PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('$SHA')/" "$AUR_DIR/PKGBUILD"

cd "$AUR_DIR"
if command -v makepkg > /dev/null; then
  makepkg --printsrcinfo > .SRCINFO
else
  # NixOS has no /etc/makepkg.conf; the pacman package ships one.
  nix-shell -p pacman --run \
    'MAKEPKG_CONF="$(dirname "$(dirname "$(command -v makepkg)")")/etc/makepkg.conf" makepkg --printsrcinfo' \
    > .SRCINFO
fi

git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
  echo "aur-publish: already at $VERSION, nothing to commit"
else
  git commit -m "riso $VERSION"
  echo "committed; publish with: git -C $AUR_DIR push origin master"
fi

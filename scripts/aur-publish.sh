#!/usr/bin/env bash
# Sync the AUR package with the released version: copy the PKGBUILD,
# regenerate .SRCINFO and commit in the AUR clone. Run after the signed
# release tag is pushed, since makepkg verifies that tag. Pushing to the
# AUR remains the operator's move.
set -euo pipefail

REPO=$(git rev-parse --show-toplevel)
AUR_DIR=${AUR_DIR:-$REPO/../aur-riso}
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO/Cargo.toml")

if [ ! -d "$AUR_DIR/.git" ]; then
  git clone "ssh://aur@aur.archlinux.org/riso.git" "$AUR_DIR"
fi

cp "$REPO/packaging/PKGBUILD" "$AUR_DIR/PKGBUILD"

cd "$AUR_DIR"
if command -v makepkg > /dev/null; then
  makepkg --printsrcinfo > .SRCINFO
else
  # NixOS has no /etc/makepkg.conf; the pacman package ships one.
  # shellcheck disable=SC2016
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

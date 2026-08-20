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

BIN_DIR=${AUR_BIN_DIR:-$REPO/../aur-riso-bin}
if [ ! -d "$BIN_DIR/.git" ]; then
  git clone "ssh://aur@aur.archlinux.org/riso-bin.git" "$BIN_DIR"
fi
cp "$REPO/packaging/PKGBUILD-bin" "$BIN_DIR/PKGBUILD"
"$REPO/scripts/pin-bin-sums.sh" "$BIN_DIR/PKGBUILD"

publish() {
  cd "$1"
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
    echo "aur-publish: $1 already at $VERSION, nothing to commit"
  else
    git commit -m "$(basename "$1" | sed s/^aur-//) $VERSION"
    echo "committed; publish with: git -C $1 push origin master"
  fi
}

publish "$AUR_DIR"
publish "$BIN_DIR"

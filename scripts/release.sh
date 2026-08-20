#!/usr/bin/env bash
# Move every version literal in the repo to a new release in one command:
#   scripts/release.sh 0.4.0 "first changelog line" "second line"
# Cargo.toml is the source of truth; the flake and package.nix read it at
# eval time, cargo-deb and cargo-generate-rpm read it at build time. What
# remains are the files that must carry a literal for other ecosystems:
# the man page header, the AUR PKGBUILD and the RPM spec (plus its
# changelog, fed from the arguments). Gates and commit/tag stay outside:
# `just release` runs the gates, the operator signs the commit and tag.
set -euo pipefail

VERSION=${1:?usage: release.sh X.Y.Z "changelog line" [more lines...]}
shift
[ "$#" -ge 1 ] || { echo "release.sh: at least one changelog line" >&2; exit 1; }
case "$VERSION" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "release.sh: '$VERSION' does not look like X.Y.Z" >&2; exit 1 ;;
esac

cd "$(git rev-parse --show-toplevel)"

MANDATE=$(date +%Y-%m-%d)
RPMDATE=$(LC_ALL=C date '+%a %b %d %Y')

sed -i "s/^version = \".*\"$/version = \"$VERSION\"/" Cargo.toml
sed -i "s/^\.TH RISO 1 \"[^\"]*\" \"riso [^\"]*\"/.TH RISO 1 \"$MANDATE\" \"riso $VERSION\"/" docs/riso.1
sed -i "s/^pkgver=.*/pkgver=$VERSION/" packaging/PKGBUILD packaging/PKGBUILD-bin
sed -i "s/^Version:.*/Version:        $VERSION/" packaging/riso.spec

ENTRY="* $RPMDATE Emanuele Calo <emanuele.lele.calo@gmail.com> - $VERSION-1"
for line in "$@"; do
  ENTRY="$ENTRY
- $line"
done
awk -v entry="$ENTRY" '{ print; if ($0 == "%changelog") print entry "\n" }' \
  packaging/riso.spec > packaging/riso.spec.new
mv packaging/riso.spec.new packaging/riso.spec

# Rewrites the workspace members' versions in the lock.
cargo update --workspace --quiet

echo "version literals now at $VERSION:"
grep -Hn "$VERSION" Cargo.toml docs/riso.1 packaging/PKGBUILD packaging/riso.spec | head -6
cat <<DONE

next, after the gates pass:
  but commit -b chore/release-$VERSION -m "chore(release): $VERSION"
  git tag -s v$VERSION -m "riso $VERSION" <commit>
DONE

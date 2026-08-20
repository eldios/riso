#!/usr/bin/env bash
# Rewrite a PKGBUILD's sha256sums by downloading every source it names.
# Run for the -bin package once the release assets exist; the sums then
# describe exactly what users will fetch.
set -euo pipefail

PKG=${1:?usage: pin-bin-sums.sh path/to/PKGBUILD}

mapfile -t urls < <(bash -c 'source "$1" > /dev/null 2>&1
  for s in "${source[@]}"; do printf "%s\n" "${s##*::}"; done' _ "$PKG")

sums=()
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
for u in "${urls[@]}"; do
  curl -fsSL --retry 3 -o "$tmp" "$u"
  sums+=("$(sha256sum "$tmp" | cut -d' ' -f1)")
done

SUMS_LINES=$(printf "'%s'\n" "${sums[@]}") python3 - "$PKG" <<'PY'
import os, re, sys
path = sys.argv[1]
sums = os.environ["SUMS_LINES"].split()
body = "sha256sums=(" + "\n            ".join(sums) + ")"
text = open(path).read()
new = re.sub(r"sha256sums=\([^)]*\)", body, text, count=1)
open(path, "w").write(new)
PY
grep -A5 '^sha256sums' "$PKG"

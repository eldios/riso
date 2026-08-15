#!/usr/bin/env bash
#
# Render every theme Omarchy ships, both with riso and with Omarchy's own shell
# pipeline, and diff the two. A passing run means riso is a drop-in replacement
# for `omarchy-theme-set-templates` at the given ref.
#
# Usage: scripts/check-against-omarchy.sh [ref]
#
# Requires network access on the first run: the upstream repo is cloned into
# target/omarchy-<ref> and reused afterwards.

set -euo pipefail

REF="${1:-v4.0.0}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OMARCHY="$ROOT/target/omarchy-$REF"
WORK="$ROOT/target/conformance-$REF"
RISO="$ROOT/target/debug/riso"

if [[ ! -d $OMARCHY ]]; then
  echo "cloning omarchy $REF"
  git clone --depth 1 --branch "$REF" --quiet \
    https://github.com/basecamp/omarchy.git "$OMARCHY"
fi

echo "building riso"
cargo build --quiet

# Upstream scripts hard-code /bin/bash, which does not exist on NixOS.
PATCHED="$WORK/bin"
rm -rf "$WORK"
mkdir -p "$PATCHED"
for script in omarchy-theme-color omarchy-theme-set-templates; do
  sed '1s|^#!/bin/bash|#!/usr/bin/env bash|' "$OMARCHY/bin/$script" >"$PATCHED/$script"
  chmod +x "$PATCHED/$script"
done

identical=0
differing=0

for theme_dir in "$OMARCHY"/themes/*/; do
  theme="$(basename "$theme_dir")"

  # Reference: Omarchy's own pipeline, with the theme staged the way
  # omarchy-theme-set stages it before rendering.
  home="$WORK/$theme/home"
  staged="$home/.local/state/omarchy/current/next-theme"
  mkdir -p "$staged" "$home/.config/omarchy/themed"
  cp -R "$theme_dir". "$staged/"
  env HOME="$home" OMARCHY_PATH="$OMARCHY" PATH="$PATCHED:$PATH" \
    bash "$PATCHED/omarchy-theme-set-templates" 2>/dev/null

  # Subject: riso.
  actual="$WORK/$theme/riso"
  "$RISO" render --theme "$theme_dir" --out "$actual" \
    --templates "$OMARCHY/default/themed" >/dev/null 2>&1

  if diff -r -q "$staged" "$actual" >"$WORK/$theme.diff" 2>&1; then
    identical=$((identical + 1))
  else
    differing=$((differing + 1))
    echo "MISMATCH $theme"
    head -20 "$WORK/$theme.diff"
  fi
done

echo "----------------------------------------"
echo "identical: $identical   differing: $differing"
[[ $differing -eq 0 ]]

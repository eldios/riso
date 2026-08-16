#!/usr/bin/env bash
#
# One line per background of the theme in use: label<TAB>preview<TAB>path.
# The image previews itself, and the path is what a selection applies.

set -euo pipefail

state="${XDG_STATE_HOME:-$HOME/.local/state}/riso"
dir="$state/current/theme/backgrounds"
[ -d "$dir" ] || exit 0

find -L "$dir" -maxdepth 1 -type f \
  \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \) \
  2>/dev/null | sort | while IFS= read -r image; do
  name=$(basename "$image")
  printf '%s\t%s\t%s\n' "${name%.*}" "$image" "$image"
done

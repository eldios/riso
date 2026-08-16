#!/usr/bin/env bash
#
# One line per theme riso can see: name<TAB>preview image path.
# The preview is the file the theme names preview.*, else its first
# background; a theme with neither still gets a line, and the carousel
# draws a swatch card for it.

set -euo pipefail

find_preview() {
  local dir="$1" candidate
  for candidate in preview.png preview.jpg preview.jpeg preview.webp; do
    [ -f "$dir/$candidate" ] && { printf '%s' "$dir/$candidate"; return; }
  done
  if [ -d "$dir/backgrounds" ]; then
    find -L "$dir/backgrounds" -maxdepth 1 -type f \
      \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \) \
      2>/dev/null | sort | head -n1 | tr -d '\n'
  fi
}

riso theme list | while IFS=$'\t' read -r name path _; do
  printf '%s\t%s\t%s\n' "$name" "$(find_preview "$path")" "$name"
done

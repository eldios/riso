#!/usr/bin/env bash
#
# Point the current-background link at the chosen image, the same swap riso
# itself performs: built beside the link and renamed over it, so a reader
# never finds it missing. Desktops that need telling are the launcher
# wrapper's business, not this file's.

set -euo pipefail

target="$1"
current="${XDG_STATE_HOME:-$HOME/.local/state}/riso/current"
staging="$current/.background.riso-tmp"

mkdir -p "$current"
rm -f "$staging"
ln -s "$target" "$staging"
mv -T "$staging" "$current/background"

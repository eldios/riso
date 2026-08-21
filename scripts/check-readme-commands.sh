#!/usr/bin/env bash
# Every riso command the README's table names must parse in the real CLI,
# so the table cannot drift from the code unnoticed.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
RISO=${RISO:-target/debug/riso}
[ -x "$RISO" ] || { echo "check-readme-commands: build first: $RISO missing" >&2; exit 1; }

fail=0
# shellcheck disable=SC2016
while IFS= read -r cmd; do
  # Keep the fixed words: strip <...> placeholders, |alternatives beyond
  # the first, [...] optionals and flags.
  args=$(printf '%s' "$cmd" \
    | sed -E 's/`//g; s/\\//g; s/riso //; s/<[^>]*>//g; s/\[[^]]*\]//g; s/\|[^ ]*//g; s/--[a-z-]+//g; s/  +/ /g; s/ +$//')
  # shellcheck disable=SC2086
  if ! "$RISO" $args --help > /dev/null 2>&1; then
    echo "FAIL: riso $args (from: $cmd)"
    fail=1
  fi
done < <(grep -oE '`riso [^`]+`' README.md | sort -u)

[ "$fail" -eq 0 ] && echo "readme commands: all parse"
exit "$fail"

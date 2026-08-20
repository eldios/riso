# Run `just` with no arguments to see this list.
default:
    @just --list

# Every gate, in the order CI runs them.
ci: fmt-check lint test

build:
    nice -n 19 cargo build

fmt:
    nice -n 19 cargo fmt

fmt-check:
    nice -n 19 cargo fmt --check

# Test code is linted too: a warning there fails CI just the same.
lint:
    nice -n 19 cargo clippy --all-targets --all-features -- -D warnings

test:
    nice -n 19 cargo test

# Render a theme, for a quick look. Templates live upstream for now, so point
# at a checkout: `just render <theme-dir> <out-dir> target/omarchy-v4.0.0/default/themed`
render theme out templates:
    nice -n 19 cargo run -q -p riso-cli -- render --theme {{theme}} --out {{out}} --templates {{templates}}

# Compare riso against the real Omarchy pipeline on every shipped theme.
# Needs git and a network fetch of the upstream repo; see the script header.
conformance omarchy_ref="v4.0.0":
    ./scripts/check-against-omarchy.sh {{omarchy_ref}}

# Regenerate the test fixtures from a checkout of upstream Omarchy.
fixtures omarchy_ref="v4.0.0":
    ./scripts/update-fixtures.sh {{omarchy_ref}}

# Move every version literal to a new release, then run the same gates as CI.
# Usage: just release 0.4.0 "changelog line" ["another line"...]
release version +notes: && ci
    ./scripts/release.sh {{version}} {{notes}}

# Sync the AUR package to the released version; run after the tag is pushed.
aur:
    ./scripts/aur-publish.sh

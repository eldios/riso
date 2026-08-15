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

# Render a theme with the in-tree templates, for a quick look.
render theme out:
    nice -n 19 cargo run -q -p riso-cli -- render --theme {{theme}} --out {{out}} --templates templates

# Compare riso against the real Omarchy pipeline on every shipped theme.
# Needs git and a network fetch of the upstream repo; see the script header.
conformance omarchy_ref="v4.0.0":
    ./scripts/check-against-omarchy.sh {{omarchy_ref}}

# Regenerate the test fixtures from a checkout of upstream Omarchy.
fixtures omarchy_ref="v4.0.0":
    ./scripts/update-fixtures.sh {{omarchy_ref}}

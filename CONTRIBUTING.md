# Contributing

## Themes

Themes live in their own repositories and are listed in the catalog at
https://catalog.riso.re. A theme is a directory with `colors.toml` and
optional hand-written files for specific applications; the two official
ones, `riso-theme-caio` and `riso-theme-tuscany-sunset`, show the shape.

Before proposing one:

```
riso theme validate /path/to/theme
```

must come back clean. Themes are data: the validator refuses scripts and
anything else that would run on a user's machine. To get a theme listed,
open a pull request against https://github.com/eldios/riso-themes adding
it to `index.json`.

## Code

The dev shell carries every tool the project uses:

```
nix develop
just ci
```

`just ci` runs the same gates as the CI workflow: formatting, clippy
with warnings as errors over every target, the tests, and the README
command check. A change is ready when it is green locally.

Tests go with the behaviour they cover. Unit tests sit next to the code;
`crates/riso-cli/tests/cli` runs the real binary in a sandboxed home and
is the place for anything a user could observe.

Commit messages follow `type(scope): summary` in the imperative
(`feat(wire): ...`, `fix(catalog): ...`). Keep a commit to one change.

## Plugins

A plugin teaches riso to theme one more application: a `manifest.toml`
and the templates it names, installed from a git repository with
`riso plugin install`. The manifest keys are documented in `riso(1)`
under PLUGINS. A plugin runs as code on the user's machine, so keep it
to templates and one reload command.

## Reporting

Bugs and feature requests go through the issue templates. Security
problems go through the process in `SECURITY.md`, never a public issue.

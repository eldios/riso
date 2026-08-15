# riso

A modular ricing framework: a theme is data, and every config file the desktop
reads is generated from it.

Today `riso` renders [Omarchy](https://github.com/basecamp/omarchy) 4 themes.
It reads a theme's `colors.toml`, resolves the full palette, renders the
templates, and writes the result. Output is byte-identical to what Omarchy's
own shell pipeline produces, so it works as a drop-in replacement for
`omarchy-theme-set-templates` on systems where that pipeline cannot run.

## Use

```
riso palette --theme themes/tokyo-night
riso render --theme themes/tokyo-night --out ~/.local/state/riso/theme --templates default/themed
```

`render` copies the theme into the output directory first, then renders every
template whose output is not already there. That ordering is what gives a theme
the last word: a file the theme ships by hand is never replaced by a generated
one.

Repeat `--templates` to add more template directories. Earlier directories win,
so user templates go before built-in ones.

`--dry-run` reports what would be written without writing it.

## Layering

Four levels decide the value of any given setting, strongest last:

1. the module's template, rendered from the theme's tokens
2. the theme's tokens
3. a file the theme hand-writes, which replaces the generated one entirely
4. a user override

A theme therefore cannot be incomplete: level 1 always produces every file.
Adding a new template gives every existing theme support for that application
without touching any of them.

## Develop

```
nix develop      # toolchain plus every tool the gates need
just             # list the tasks
just ci          # format check, lint, tests - what CI runs
just conformance # diff riso against the real Omarchy pipeline, per theme
```

`just conformance` clones upstream Omarchy on first use and compares the two
renderers across every shipped theme. It is the test that matters: the unit
suite proves internal consistency, this one proves interoperability.

## Status

Early. The template engine, palette resolution, and file writing are done and
verified against upstream. Theme switching, module reload, snapshots, and the
GUI are not built yet.

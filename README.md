# riso

A ricing framework for Linux desktops. Community-driven, and yours to extend.

A theme is data: a palette, some typography, optionally a wallpaper. Everything
the desktop reads is generated from it, so adding support for an application
does not go back and make every existing theme incomplete. And when an author
wants to hand-tune one file, what the theme ships wins over what was generated.

Runs on Arch and derivatives, Debian and its family, Fedora and the rpm world,
and NixOS. One binary, a manpage, and `git` and `curl` for fetching things.

## Install

```bash
# Arch
cd packaging && makepkg -si

# Debian, Ubuntu, Mint
cargo deb && sudo dpkg -i target/debian/riso_*.deb

# Fedora, RHEL, openSUSE
cargo build --release && cargo generate-rpm && sudo rpm -i target/generate-rpm/*.rpm

# NixOS
nix profile install github:eldios/riso
```

## Use

```bash
riso theme install rose-pine     # from the catalog
riso theme install <git-url>     # from anywhere
riso set "Rose Pine"
```

```
riso set <name>              apply a theme and tell the desktop
riso theme list              what is installed, and what is read-only
riso theme validate <path>   is this safe to install
riso plugin list             what teaches riso about more applications
riso render                  render into a directory without applying
riso palette                 the resolved palette, key and value
riso restore                 put back what riso wrote over
riso uninstall --yes         put everything back and forget the state
```

`riso(1)` has the options.

## How a theme is put together

Four layers decide any given file, strongest first:

1. a file the theme ships by hand
2. a user template directory
3. a template directory the desktop provides
4. the templates compiled into `riso`

Layer 4 is what makes a theme impossible to leave incomplete; layer 1 is what
lets an author overrule the generator on the file they care about.

## Your configuration stays yours

`riso` renders fragments into its own tree and never rewrites an
application's config file. The application's config, which belongs to you,
includes the fragment: mako's `include=`, waybar's `@import`, hyprlock's
`source`, a terminal's import directive. A theme change rewrites the
fragment; every rule you wrote around the include survives it, and your own
overrides can always be included after the fragment so they win.

The one exception is a plugin, whose whole point is reaching a path the
application dictates. What was at that path first is copied aside for
`riso restore`, and the file opens with a comment saying it is generated and
where edits belong, in whatever comment syntax its format speaks. A format
with no comments, JSON above all, gets none.

Fonts are tokens like colours, so a theme carries its own typography instead of
leaving it to a setting somewhere else.

## Desktops

`riso` writes the files and then tells the desktop, in whatever way that
desktop expects: Hyprland, Sway, niri, and Omarchy are recognized from the
session. A desktop it does not know still gets its files written, which is all
some of them need.

## Extending

A plugin teaches `riso` to theme an application it does not know about: a
directory with a manifest and its templates, installed from git.

```toml
id = "eldios.zed"
api = 1
reload = ["zed", "--reload"]

[[render]]
template = "zed.json.tpl"
target = "~/.config/zed/themes/riso.json"
```

Whatever was at that path first is copied aside, so `riso restore` can put it
back byte for byte.

Themes and plugins are ordinary git repositories. The catalog is a static index
that points at them, so publishing one needs no account anywhere and installing
one is a clone with a checksum.

## Safety

A theme is data and `riso` enforces it. Executable files, directives that name
a program to run, symlinks, and paths that climb out of the theme directory are
all refused, on the client as well as in the catalog. A theme that arrives from
a git URL never passed through a catalog at all, which is exactly why the check
runs twice.

## Compatibility

`riso` also reads themes written for [Omarchy](https://github.com/basecamp/omarchy),
and renders them identically: the conformance suite resolves every palette that
project publishes byte for byte against its pipeline, and renders its templates
to the same bytes. A theme written for either works on both. See
[NOTICE](NOTICE).

## Develop

```bash
nix develop        # toolchain plus every tool the gates need
just ci            # format, lint, tests
just conformance   # the interoperability check
```

## Status

Early, and honest about it. Rendering, applying, backgrounds, themes, plugins,
ownership tracking and validation work and are tested. Not built yet: a theme
carousel, capture from a running desktop, and more desktops.

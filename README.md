# riso

An independent, community-driven ricing framework. A theme is data, and every
config file the desktop reads is generated from it.

riso reads Omarchy themes natively and renders them identically, so a theme
written for either works on both. Beyond that it goes its own way: any distro,
any Wayland desktop, themes and extensions from any git repository, and a
binary that carries what it needs instead of a tree of scripts.

Adding support for an application does not make existing themes incomplete,
because a theme that says nothing about it still renders through the template.
And a theme that wants to hand-write one file still can: what it ships wins.

## Install

```bash
riso theme install rose-pine                          # from the catalog
riso theme install https://github.com/someone/x.git   # from any git repo
riso set "Rose Pine"
```

Themes are validated before they are kept, on the client as well as in the
catalog: installing from a git URL never passed through a catalog at all.

## Use

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

Fonts are tokens like colours, so a theme carries its own typography rather
than leaving it to a separate setting.

## Compatibility

`riso` reads Omarchy themes natively and renders them byte-for-byte the way
Omarchy's own pipeline does, verified against every theme it ships. The point
is not to be a copy: it is that a theme written for either works on both, and
that this claim is checked rather than asserted.

```bash
just conformance   # diff riso against Omarchy's renderer, theme by theme
```

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

## Develop

```bash
nix develop        # toolchain plus every tool the gates need
just ci            # format, lint, tests
just conformance   # interoperability against upstream
```

`riso` calls `git` and `curl` at run time and nothing else; everything it
needs is in the binary. Packaging notes are in `packaging/`.

## Where riso came from

The theme format riso speaks was designed in
[Omarchy](https://github.com/basecamp/omarchy), and riso implements it rather
than inventing a second one that would split themes into two incompatible
worlds. Credit for the format goes there; the implementation, the direction and
the governance are riso's own. See [NOTICE](NOTICE).

## Status

Early, and honest about it. Rendering, applying, backgrounds, themes, plugins,
ownership tracking and validation work and are tested. Not built yet: a theme
carousel, capture from a running desktop, and the desktops beyond Omarchy,
Hyprland, Sway and niri.

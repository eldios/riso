<p align="center">
  <img src="assets/riso-logo.png" alt="riso" width="240" />
</p>

<h1 align="center">riso</h1>

<p align="center">
  A ricing framework for Linux desktops. Community-driven, and yours to extend.
</p>

<p align="center">
  <a href="https://github.com/eldios/riso/actions"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/eldios/riso/ci.yml?branch=main&label=ci&style=flat-square" /></a>
  <a href="https://github.com/eldios/riso/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/eldios/riso?style=flat-square" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/eldios/riso?style=flat-square" /></a>
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> &middot;
  <a href="#commands">Commands</a> &middot;
  <a href="#how-a-theme-is-put-together">How it works</a> &middot;
  <a href="#extending">Extending</a>
</p>

> screenshot placeholder: one desktop, three themes, side by side

## Quick start

```bash
cd packaging && makepkg -si      # Arch, see below for the other families
riso theme install caio          # from the catalog
riso theme set caio              # apply it, and tell the desktop
```

<details>
<summary>Debian, Ubuntu, Mint</summary>

```bash
cargo deb && sudo dpkg -i target/debian/riso_*.deb
```

</details>

<details>
<summary>Fedora, RHEL, openSUSE</summary>

```bash
cargo build --release && cargo generate-rpm && sudo rpm -i target/generate-rpm/*.rpm
```

</details>

<details>
<summary>NixOS</summary>

Try it without installing anything:

```bash
nix run github:eldios/riso -- theme list
```

To keep it, add `github:eldios/riso` as a flake input and pull
`overlays.default` (or `packages.<system>.default`) into your NixOS or
home-manager configuration.

</details>

Runs on Arch and derivatives, Debian and its family, Fedora and the rpm world,
and NixOS. One binary, a manpage, and `git` and `curl` for fetching things.

Themes come from anywhere, not only the catalog.

```bash
riso theme install <git-url>     # from anywhere
riso theme install --gui         # or browse the catalog with previews
```

> screenshot placeholder: the --gui carousel

## Why a framework and not a folder of dotfiles

A theme is data: a palette, some typography, optionally a wallpaper. Everything
the desktop reads is generated from it, so adding support for an application
does not go back and make every existing theme incomplete. And when an author
wants to hand-tune one file, what the theme ships wins over what was generated.

Fonts are tokens like colours, so a theme carries its own typography instead of
leaving it to a setting somewhere else. The cat in the logo is caio, and so is
the example theme.

## Commands

| Command | What it does |
| --- | --- |
| `riso theme install <name>\|<git-url>\|--gui` | from the catalog, from anywhere, or from previews |
| `riso theme set <name>` | apply a theme and tell the desktop |
| `riso theme set --gui\|--tui` | pick from previews: carousel or terminal |
| `riso theme get\|list` | what is applied, what is installed |
| `riso theme update` | bring installed themes up to date |
| `riso theme validate <path>` | is this safe to install |
| `riso backgrounds set\|next` | the wallpaper: which image... |
| `riso backgrounds mode\|get` | ...and how it scales |
| `riso plugin list` | what teaches riso about more applications |
| `riso dev palette\|render` | theme-author tools |
| `riso config` | the few persistent options |
| `riso restore` | put back what riso wrote over |
| `riso uninstall --yes` | put everything back and forget the state |

Every component has a single-letter alias (`riso t s` is `riso theme set`,
`backgrounds` also answers to `b`) and every command takes
`-o human|json|yaml`, so scripts read structure instead of parsing prose.

`riso(1)` has the options. The handful worth keeping live in
`~/.config/riso/config.toml`, managed by `riso config`: the default output
format, and `omarchy-themes` to keep Omarchy's own themes out of the list.

## How a theme is put together

Four layers decide any given file, strongest first:

1. a file the theme ships by hand
2. a user template directory
3. a template directory the desktop provides
4. the templates compiled into `riso`

Layer 4 is what makes a theme impossible to leave incomplete; layer 1 is what
lets an author overrule the generator on the file they care about.

### Your configuration stays yours

`riso` renders fragments into its own tree and never rewrites an
application's config file. Your config, which belongs to you, includes the
fragment: mako's `include=`, waybar's `@import`, hyprlock's `source`, a
terminal's import directive. A theme change rewrites the fragment; every rule
you wrote around the include survives it, and your own overrides can be
included after the fragment so they win.

The one exception is a plugin, whose whole point is reaching a path the
application dictates. What was there first is copied aside for `riso restore`,
byte for byte, and the file opens with a comment saying it is generated and
where edits belong, in whatever comment syntax the format speaks. A format
with no comments, JSON above all, gets none.

### Desktops

`riso` writes the files and then tells the desktop, in whatever way that
desktop expects: Hyprland, Sway, niri, and Omarchy are recognized from the
session. A desktop it does not know still gets its files written, which is all
some of them need.

### Safety

A theme is data and `riso` enforces it. Executable files, directives that name
a program to run, symlinks, and paths that climb out of the theme directory are
all refused, on the client as well as in the catalog. A theme that arrives from
a git URL never passed through a catalog at all, which is exactly why the check
runs twice.

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

Themes and plugins are ordinary git repositories. The catalog is a static index
that points at them, so publishing one needs no account anywhere and installing
one is a clone with a checksum.

## Omarchy compatibility

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

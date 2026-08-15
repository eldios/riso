# Omarchy 4 compatibility

What `riso` must reproduce to stand in for Omarchy's theming pipeline, and
where it deliberately does not.

## The contract

Omarchy 4 runs its desktop as a single Quickshell process. That shell reads
generated files rather than being configured directly, which is what makes it
themeable from outside:

| File | Holds |
|---|---|
| `colors.toml` | the palette, as flat semantic keys |
| `shell.toml` | surface roles, control states, spacing, typography, bar sizing |
| `shell.<section>.toml` | a replacement for one section of `shell.toml` |
| `hyprland.lua` | window borders and gaps |
| one file per app | terminals, editors, browsers |

Producing those files is the whole job. Nothing here drives the shell, patches
QML, or talks to a running process.

## The template dialect

Templates are plain files ending in `.tpl` where `{{ ... }}` tokens are
replaced. It resembles Jinja and is not Jinja: arguments are positional and
space-separated, there is no control flow, and an unrecognized token is copied
through unchanged instead of raising.

### Keys

| Token | Renders |
|---|---|
| `{{ accent }}` | `#7aa2f7` |
| `{{ accent_strip }}` | `7aa2f7` |
| `{{ accent_rgb }}` | `122,162,247` |

`_rgb` only applies to a plain `#rrggbb` value; anything else leaves the token
in place. `_strip` removes a single leading `#` and works on any value.

A plain key must be written with exactly one space per side. `{{key}}` and
`{{  key  }}` are not substituted, because upstream matches these as fixed
strings. Function tokens are whitespace-tolerant, because those are matched by
pattern.

### Blending

```
{{ mix background foreground 15% }}
{{ mix_strip background accent 0.35 }}
{{ mix_rgb color0 color7 50 }}
```

The amount is a fraction or a percentage; a bare number above 1 is read as a
percentage. Values are clamped to `0..1`. Channels round half-up, matching the
`int(x + 0.5)` of the original awk.

### Gradients

A gradient is space-separated colors plus an optional `<n>deg` angle, and may
appear anywhere a color may. Each consumer needs a different spelling:

| Token | Renders |
|---|---|
| `{{ hypr_gradient border accent }}` | `{ colors = { "rgba(33ccffee)", "rgba(00ff99ee)" }, angle = 45 }` |
| `{{ shell_gradient border accent }}` | `rgba(33ccffee) rgba(00ff99ee) 45deg` |
| `{{ gradient_start border accent }}` | `#33ccff` |

The second argument is a fallback, tried first as another palette key and then
taken literally. For a single color, `hypr_gradient` emits a quoted string
rather than a table, because Hyprland's Lua config expects a string there.

## Palette resolution

A theme states around 26 keys; consumers see around 56. The rest come from a
cascade that must run in this order, because each step may consume what an
earlier one produced:

1. canonical names absorb their legacy short forms (`bg` becomes `background`)
2. `background` and `foreground` fall back to `color0` and `color7`
3. semantic color names fall back to their ANSI slots
4. `magenta` absorbs `purple`
5. foregrounds, `muted`, and `selection` fill from their nearest neighbour
6. `orange` falls back to `yellow`
7. missing shades are derived by blending: `dark_background` is `background`
   25% towards black, each `bright_*` is its base 20% towards white
8. ANSI slots are filled back in from the semantic names
9. legacy short names are republished from the canonical values
10. the mode is taken from `mode`, then `theme_type`, then a `light.mode`
    marker file, then background luminance, then `dark`

Step 10's luminance test sums the three channels and calls anything above 382
light.

## Deliberate divergences

Three, each narrowing a silent failure. All are unreachable for a well-formed
theme, so output stays identical for every theme Omarchy ships.

**A derivation from a non-color is skipped and reported.** Upstream feeds the
value into the blend anyway and emits a malformed color. Here the key is left
unset and a warning names it.

**Malformed keys and values are reported per line.** Upstream reports them too,
but without a line number.

**Keys that resolve to nothing are reported.** They still render as an empty
string, matching upstream, but the run says which ones.

Warnings go to stderr so piped output stays clean.

## Known gaps

`colors.toml` is read line by line, not as TOML, because that is what upstream
does and themes rely on the tolerance. A section header inside one is reported
and skipped rather than parsed.

The luminance threshold in step 10 is covered by unit tests only: every theme
Omarchy ships states its mode explicitly, so the conformance suite never
exercises that path.

Themes are rendered, not applied. Choosing a theme, reloading consumers, and
restoring previous state are not implemented.

//! Templates that ship inside the binary.
//!
//! These cover applications that theme the same way everywhere, so a working
//! `riso` needs nothing on disk. Anything a desktop defines for itself, such
//! as its shell surfaces or its compositor config, is that desktop's template
//! to provide, not ours.
//!
//! They are the weakest layer: a template directory or a plugin claiming the
//! same output name replaces the built-in one entirely.

/// A template compiled into the binary, named by what it produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builtin {
    /// Output file name, e.g. `ghostty.conf`.
    pub name: &'static str,
    pub source: &'static str,
}

pub const TEMPLATES: &[Builtin] = &[
    Builtin {
        name: "alacritty.toml",
        source: include_str!("../templates/alacritty.toml.tpl"),
    },
    Builtin {
        name: "btop.theme",
        source: include_str!("../templates/btop.theme.tpl"),
    },
    Builtin {
        name: "chromium.theme",
        source: include_str!("../templates/chromium.theme.tpl"),
    },
    Builtin {
        name: "foot.ini",
        source: include_str!("../templates/foot.ini.tpl"),
    },
    Builtin {
        name: "ghostty.conf",
        source: include_str!("../templates/ghostty.conf.tpl"),
    },
    Builtin {
        name: "helix.toml",
        source: include_str!("../templates/helix.toml.tpl"),
    },
    Builtin {
        name: "kitty.conf",
        source: include_str!("../templates/kitty.conf.tpl"),
    },
    Builtin {
        name: "neovim.lua",
        source: include_str!("../templates/neovim.lua.tpl"),
    },
    Builtin {
        name: "obsidian.css",
        source: include_str!("../templates/obsidian.css.tpl"),
    },
    Builtin {
        name: "vscode.json",
        source: include_str!("../templates/vscode.json.tpl"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Palette;
    use crate::template;

    fn palette() -> Palette {
        let (palette, _) = Palette::parse(
            "mode = \"dark\"\n\
             background = \"#1a1b26\"\n\
             foreground = \"#a9b1d6\"\n\
             accent = \"#7aa2f7\"\n\
             red = \"#f7768e\"\n\
             green = \"#9ece6a\"\n\
             yellow = \"#e0af68\"\n\
             blue = \"#7aa2f7\"\n\
             magenta = \"#ad8ee6\"\n\
             cyan = \"#449dab\"\n",
        );
        let (palette, _) = palette.resolve(false);
        palette
    }

    #[test]
    fn every_builtin_renders_without_leaving_a_placeholder() {
        let palette = palette();
        for builtin in TEMPLATES {
            let rendered = template::render(builtin.source, &palette);
            assert!(
                !rendered.contains("{{"),
                "{} left a placeholder behind:\n{}",
                builtin.name,
                rendered
                    .lines()
                    .filter(|line| line.contains("{{"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }

    #[test]
    fn names_are_unique_and_look_like_files() {
        let mut seen = std::collections::BTreeSet::new();
        for builtin in TEMPLATES {
            assert!(seen.insert(builtin.name), "duplicate name {}", builtin.name);
            assert!(
                builtin.name.contains('.'),
                "{} should name an output file",
                builtin.name
            );
        }
    }

    #[test]
    fn the_json_ones_render_to_valid_looking_json() {
        let palette = palette();
        for builtin in TEMPLATES.iter().filter(|b| b.name.ends_with(".json")) {
            let rendered = template::render(builtin.source, &palette);
            let opens = rendered.matches('{').count();
            let closes = rendered.matches('}').count();
            assert_eq!(opens, closes, "{} has unbalanced braces", builtin.name);
            assert!(rendered.trim_start().starts_with('{'));
        }
    }
}

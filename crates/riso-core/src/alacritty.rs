//! Deriving a palette from a theme's `alacritty.toml`.
//!
//! Some themes in circulation carry no `colors.toml` at all: the terminal
//! palette is the palette. Omarchy generates a `colors.toml` from it at
//! theme-set time, and interoperability means deriving exactly the same
//! one, quirk for quirk, so the same theme resolves to the same colors on
//! both pipelines.

use std::collections::HashMap;

/// The `colors.toml` text Omarchy would generate, or None when the eight
/// normal colors are not all present, which is where upstream gives up too.
pub fn derive_palette(alacritty: &str) -> Option<String> {
    let colors = collect_colors(alacritty);
    let get = |path: &str| colors.get(path).cloned();

    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];

    let mut normal = Vec::with_capacity(8);
    for name in NAMES {
        normal.push(get(&format!("colors.normal.{name}"))?);
    }
    let bright: Vec<String> = NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| get(&format!("colors.bright.{name}")).unwrap_or_else(|| normal[i].clone()))
        .collect();

    let background = get("colors.primary.background").unwrap_or_else(|| normal[0].clone());
    let foreground = get("colors.primary.foreground").unwrap_or_else(|| normal[7].clone());
    let selection = get("colors.selection.background").unwrap_or_else(|| foreground.clone());
    let accent = normal[4].clone();

    let mut palette: Vec<String> = normal;
    palette[0] = background.clone();
    palette[7] = foreground.clone();
    palette.extend(bright);

    let mut out = format!(
        "accent = \"{accent}\"\nselection = \"{selection}\"\n\n\
         background = \"{background}\"\nforeground = \"{foreground}\"\n\n"
    );
    for (i, color) in palette.iter().enumerate() {
        out.push_str(&format!("color{i} = \"{color}\"\n"));
    }
    Some(out)
}

/// Every color-valued key as `<section>.<key>` -> `#hex`, first valid
/// occurrence winning. A dotted key under `[colors]` names the same path as
/// the section form (`normal.black` is `colors.normal.black`), and the
/// section form takes precedence.
fn collect_colors(text: &str) -> HashMap<String, String> {
    let mut direct: HashMap<String, String> = HashMap::new();
    let mut dotted: HashMap<String, String> = HashMap::new();
    let mut section = String::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            section = trimmed
                .find(']')
                .filter(|&end| trimmed[end + 1..].trim().is_empty())
                .map(|end| trimmed[1..end].to_owned())
                .unwrap_or_default();
            continue;
        }
        if section.is_empty() {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim();
        if key.is_empty() || key.starts_with('#') {
            continue;
        }
        let value = line[eq + 1..].trim();
        let Some(hex) = color_value(value) else {
            continue;
        };

        let path = format!("{section}.{key}");
        if section == "colors" && key.contains('.') {
            dotted.entry(path).or_insert(hex);
        } else {
            direct.entry(path).or_insert(hex);
        }
    }

    for (path, hex) in dotted {
        direct.entry(path).or_insert(hex);
    }
    direct
}

/// A lone hex color, optionally 0x- or #-prefixed and quoted, ignoring a
/// trailing comment: the shapes Omarchy's extractor accepts, no more.
fn color_value(value: &str) -> Option<String> {
    let accepted = if let Some(rest) = value.strip_prefix(['"', '\'']) {
        let body = rest
            .strip_prefix("0x")
            .or_else(|| rest.strip_prefix("0X"))
            .or_else(|| rest.strip_prefix('#'))
            .unwrap_or(rest);
        let hex6 = body.len() >= 6 && body.as_bytes()[..6].iter().all(|b| b.is_ascii_hexdigit());
        hex6 && {
            let tail = &body[6..];
            tail.is_empty()
                || tail
                    .strip_prefix(['"', '\''])
                    .is_some_and(|after| after.is_empty() || after.trim_start().starts_with('#'))
        }
    } else {
        let body = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .unwrap_or(value);
        body.len() >= 6 && body.as_bytes()[..6].iter().all(|b| b.is_ascii_hexdigit()) && {
            let tail = &body[6..];
            tail.is_empty() || tail.trim_start().starts_with('#')
        }
    };
    if !accepted {
        return None;
    }

    // The color is the first run of six hex digits in the value.
    let bytes = value.as_bytes();
    let mut run = 0;
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_hexdigit() {
            run += 1;
            if run == 6 {
                let start = i + 1 - 6;
                return Some(format!("#{}", value[start..=i].to_ascii_lowercase()));
            }
        } else {
            run = 0;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "\
[colors.normal]
black = \"#111111\"
red = \"#ff0000\"
green = \"#00ff00\"
yellow = \"#ffff00\"
blue = \"#0000ff\"
magenta = \"#ff00ff\"
cyan = \"#00ffff\"
white = \"#eeeeee\"
";

    #[test]
    fn eight_normal_colors_are_enough() {
        let toml = derive_palette(MINIMAL).expect("derive");
        assert!(toml.contains("background = \"#111111\""));
        assert!(toml.contains("foreground = \"#eeeeee\""));
        assert!(toml.contains("accent = \"#0000ff\""));
        assert!(toml.contains("selection = \"#eeeeee\""));
        // Bright colors fall back to the normal ones.
        assert!(toml.contains("color9 = \"#ff0000\""));
    }

    #[test]
    fn a_missing_normal_color_gives_up_like_upstream() {
        let partial = MINIMAL.replace("white = \"#eeeeee\"\n", "");
        assert_eq!(derive_palette(&partial), None);
    }

    #[test]
    fn primary_overrides_the_edges_but_not_the_accent() {
        let toml = format!(
            "{MINIMAL}[colors.primary]\nbackground = \"#101010\"\nforeground = \"#fafafa\"\n"
        );
        let derived = derive_palette(&toml).expect("derive");
        assert!(derived.contains("color0 = \"#101010\""));
        assert!(derived.contains("color7 = \"#fafafa\""));
        assert!(derived.contains("selection = \"#fafafa\""));
        assert!(derived.contains("accent = \"#0000ff\""));
    }

    #[test]
    fn dotted_keys_yield_to_the_section_form() {
        let toml = format!("[colors]\nnormal.black = \"#222222\"\n{MINIMAL}");
        let derived = derive_palette(&toml).expect("derive");
        assert!(derived.contains("color0 = \"#111111\""));
    }

    #[test]
    fn quoted_prefixed_and_commented_values_all_read() {
        for value in [
            "\"0xAABBCC\"",
            "'#aabbcc'",
            "\"aabbcc\" # like this",
            "aabbcc",
            "0xAABBCC # bare",
        ] {
            assert_eq!(color_value(value).as_deref(), Some("#aabbcc"), "{value}");
        }
        for value in ["\"not a color\"", "aabbcc dd", "\"#aabb\"", "[1, 2, 3]"] {
            assert_eq!(color_value(value), None, "{value}");
        }
    }

    #[test]
    fn first_occurrence_wins() {
        let toml = format!("{MINIMAL}black = \"#333333\"\n");
        let derived = derive_palette(&toml).expect("derive");
        assert!(derived.contains("color0 = \"#111111\""));
    }
}

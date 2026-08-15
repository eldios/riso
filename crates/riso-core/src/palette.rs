//! Parsing and resolution of a theme's `colors.toml`.
//!
//! Two behaviours are deliberate and non-obvious:
//!
//! * The file is read line by line rather than as TOML. Upstream does the same,
//!   and a strict TOML parser would reject files that themes ship today.
//! * Resolution order is load-bearing. Each step may consume what an earlier
//!   step produced, so the cascade below must not be reordered.

use std::collections::BTreeMap;

use crate::color::{mix, parse_amount, Rgb};

const BLACK: &str = "#000000";
const WHITE: &str = "#ffffff";

/// Canonical name paired with the short name older themes use.
const LEGACY_PALETTE_ALIASES: &[(&str, &str)] = &[
    ("background", "bg"),
    ("dark_background", "dark_bg"),
    ("darker_background", "darker_bg"),
    ("lighter_background", "lighter_bg"),
    ("foreground", "fg"),
    ("dark_foreground", "dark_fg"),
    ("light_foreground", "light_fg"),
    ("bright_foreground", "bright_fg"),
];

/// Semantic name paired with the ANSI slot it falls back to.
const ANSI_FALLBACKS: &[(&str, &str)] = &[
    ("red", "color1"),
    ("green", "color2"),
    ("yellow", "color3"),
    ("blue", "color4"),
    ("magenta", "color5"),
    ("cyan", "color6"),
    ("bright_red", "color9"),
    ("bright_green", "color10"),
    ("bright_yellow", "color11"),
    ("bright_blue", "color12"),
    ("bright_magenta", "color13"),
    ("bright_cyan", "color14"),
];

/// ANSI slot paired with the semantic name that fills it when absent.
const ANSI_FROM_SEMANTIC: &[(&str, &str)] = &[
    ("color0", "background"),
    ("color1", "red"),
    ("color2", "green"),
    ("color3", "yellow"),
    ("color4", "blue"),
    ("color5", "magenta"),
    ("color6", "cyan"),
    ("color7", "foreground"),
    ("color8", "muted"),
    ("color9", "bright_red"),
    ("color10", "bright_green"),
    ("color11", "bright_yellow"),
    ("color12", "bright_blue"),
    ("color13", "bright_magenta"),
    ("color14", "bright_cyan"),
    ("color15", "bright_foreground"),
];

/// Colors derived by lightening a base by 20% when the theme omits them.
const BRIGHT_DERIVATIONS: &[(&str, &str)] = &[
    ("bright_red", "red"),
    ("bright_yellow", "yellow"),
    ("bright_green", "green"),
    ("bright_cyan", "cyan"),
    ("bright_blue", "blue"),
    ("bright_magenta", "magenta"),
];

/// Something the theme got wrong that would otherwise vanish silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    UnsupportedKey { line: usize },
    UnsupportedValue { line: usize, key: String },
    /// A key resolved to nothing, so `{{ key }}` will render as an empty string.
    EmptyValue { key: String },
    /// A derivation was skipped because its source was not a `#rrggbb` color.
    UnderivableColor { key: String, source: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette {
    values: BTreeMap<String, String>,
}

impl Palette {
    /// Read a `colors.toml` without resolving anything.
    ///
    /// `line` numbers in warnings are 1-based so they point at the file.
    pub fn parse(input: &str) -> (Self, Vec<Warning>) {
        let mut values = BTreeMap::new();
        let mut warnings = Vec::new();

        for (index, line) in input.lines().enumerate() {
            let number = index + 1;
            let (raw_key, raw_value) = match line.split_once('=') {
                Some((key, value)) => (key, value),
                None => (line, ""),
            };

            let key: String = raw_key
                .chars()
                .filter(|c| !matches!(c, '"' | '\'' | ' '))
                .collect();
            if key.is_empty() || key.starts_with('#') {
                continue;
            }

            let value = extract_value(raw_value);

            if !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                warnings.push(Warning::UnsupportedKey { line: number });
                continue;
            }
            if !value.chars().all(is_supported_value_char) {
                warnings.push(Warning::UnsupportedValue { line: number, key });
                continue;
            }

            values.insert(key, value);
        }

        (Self { values }, warnings)
    }

    /// Apply the full alias and derivation cascade.
    ///
    /// `has_light_mode_file` reports whether a `light.mode` marker sits next to
    /// the `colors.toml`; it only matters when the theme states no mode.
    pub fn resolve(mut self, has_light_mode_file: bool) -> (Self, Vec<Warning>) {
        let mut warnings = Vec::new();

        for (canonical, legacy) in LEGACY_PALETTE_ALIASES {
            self.fill_from(canonical, legacy);
        }

        self.fill_from("background", "color0");
        self.fill_from("foreground", "color7");
        self.copy_if_present("background", "color0");
        self.copy_if_present("foreground", "color7");

        for (semantic, ansi) in ANSI_FALLBACKS {
            self.fill_from(semantic, ansi);
        }
        self.fill_from("magenta", "purple");
        self.fill_from("bright_magenta", "bright_purple");

        self.fill_from_first("light_foreground", &["color7", "foreground"]);
        self.fill_from_first("bright_foreground", &["color15", "foreground"]);
        self.set("cursor", self.get("bright_foreground").unwrap_or_default());
        self.fill_from_first("lighter_background", &["color0", "background"]);
        self.fill_from_first("dark_foreground", &["color8", "foreground"]);
        self.fill_from_first("muted", &["color8", "dark_foreground"]);
        self.fill_from_first(
            "selection",
            &["selection_background", "color8", "color0", "background"],
        );
        self.fill_from("selection_background", "selection");
        self.fill_from("selection_foreground", "bright_foreground");
        self.fill_from("orange", "yellow");

        self.derive("brown", "orange", BLACK, "50%", &mut warnings);
        self.derive("dark_background", "background", BLACK, "25%", &mut warnings);
        self.derive("darker_background", "background", BLACK, "50%", &mut warnings);
        for (target, source) in BRIGHT_DERIVATIONS {
            self.derive(target, source, WHITE, "20%", &mut warnings);
        }

        self.fill_from("purple", "magenta");
        self.fill_from("bright_purple", "bright_magenta");

        for (ansi, semantic) in ANSI_FROM_SEMANTIC {
            self.fill_from(ansi, semantic);
        }

        // Publish canonical values back under their legacy spellings, so a
        // template written against either name resolves.
        for (canonical, legacy) in LEGACY_PALETTE_ALIASES {
            if let Some(value) = self.non_empty(canonical) {
                self.set(legacy, value);
            }
        }

        self.resolve_mode(has_light_mode_file);
        self.set("theme_type", self.get("mode").unwrap_or_default());

        for (key, value) in &self.values {
            if value.is_empty() {
                warnings.push(Warning::EmptyValue { key: key.clone() });
            }
        }

        (self, warnings)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    /// The value only when it is present and not empty.
    pub fn non_empty(&self, key: &str) -> Option<String> {
        self.values.get(key).filter(|v| !v.is_empty()).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.values.iter()
    }

    fn set(&mut self, key: &str, value: String) {
        self.values.insert(key.to_owned(), value);
    }

    /// Give `key` the value of `source` unless it already holds something.
    ///
    /// A missing `source` still creates `key` as empty, matching upstream: the
    /// key must exist so its placeholder is substituted rather than left raw.
    fn fill_from(&mut self, key: &str, source: &str) {
        if self.non_empty(key).is_some() {
            return;
        }
        let value = self.get(source).unwrap_or_default();
        self.set(key, value);
    }

    fn fill_from_first(&mut self, key: &str, sources: &[&str]) {
        if self.non_empty(key).is_some() {
            return;
        }
        let value = sources
            .iter()
            .find_map(|source| self.non_empty(source))
            .unwrap_or_default();
        self.set(key, value);
    }

    fn copy_if_present(&mut self, source: &str, target: &str) {
        if let Some(value) = self.non_empty(source) {
            self.set(target, value);
        }
    }

    /// Blend `source` towards `towards` to fill `key`, when `key` is absent.
    ///
    /// Upstream feeds a non-color source straight into the blend and emits
    /// garbage. Here the derivation is skipped and reported instead.
    fn derive(
        &mut self,
        key: &str,
        source: &str,
        towards: &str,
        amount: &str,
        warnings: &mut Vec<Warning>,
    ) {
        if self.non_empty(key).is_some() {
            return;
        }
        let Some(raw) = self.non_empty(source) else {
            return;
        };
        let Ok(start) = Rgb::from_hex(&raw) else {
            warnings.push(Warning::UnderivableColor {
                key: key.to_owned(),
                source: raw,
            });
            return;
        };
        let end = Rgb::from_hex(towards).expect("constant is valid hex");
        let amount = parse_amount(amount).expect("constant is a valid amount");
        self.set(key, mix(start, end, amount).to_hex());
    }

    fn resolve_mode(&mut self, has_light_mode_file: bool) {
        self.fill_from("mode", "theme_type");
        if self.non_empty("mode").is_some() {
            return;
        }

        let mode = if has_light_mode_file {
            "light"
        } else {
            match self.non_empty("background").as_deref().map(Rgb::from_hex) {
                // Upstream sums the channels and compares against 382.
                Some(Ok(bg)) => {
                    let luminance = u32::from(bg.r) + u32::from(bg.g) + u32::from(bg.b);
                    if luminance > 382 {
                        "light"
                    } else {
                        "dark"
                    }
                }
                _ => "dark",
            }
        };
        self.set("mode", mode.to_owned());
    }
}

/// Take the text between the first pair of quotes, else trim the raw value.
///
/// Quoting is what lets a value carry a trailing comment: everything after the
/// closing quote is dropped.
fn extract_value(raw: &str) -> String {
    let is_quote = |c: char| c == '"' || c == '\'';
    match raw.find(is_quote) {
        Some(open) => {
            let rest = &raw[open + 1..];
            match rest.find(is_quote) {
                Some(close) => rest[..close].to_owned(),
                None => rest.to_owned(),
            }
        }
        None => raw.trim().to_owned(),
    }
}

fn is_supported_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "#(),._+/% -".contains(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(input: &str) -> Palette {
        let (palette, _) = Palette::parse(input);
        let (palette, _) = palette.resolve(false);
        palette
    }

    #[test]
    fn parses_quoted_values_and_ignores_trailing_comments() {
        let (palette, warnings) = Palette::parse("accent = \"#7aa2f7\" # the blue\nmode = 'dark'\n");
        assert_eq!(palette.get("accent").as_deref(), Some("#7aa2f7"));
        assert_eq!(palette.get("mode").as_deref(), Some("dark"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let (palette, warnings) = Palette::parse("# a comment\n\n   \naccent = \"#111111\"\n");
        assert_eq!(palette.iter().count(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn reports_a_section_header_instead_of_dropping_it() {
        let (_, warnings) = Palette::parse("[colors]\naccent = \"#111111\"\n");
        assert_eq!(warnings, vec![Warning::UnsupportedKey { line: 1 }]);
    }

    #[test]
    fn reports_a_value_with_unsupported_characters() {
        let (palette, warnings) = Palette::parse("accent = $(rm -rf /)\n");
        assert!(palette.get("accent").is_none());
        assert_eq!(
            warnings,
            vec![Warning::UnsupportedValue {
                line: 1,
                key: "accent".into()
            }]
        );
    }

    #[test]
    fn canonical_names_win_over_legacy_ones() {
        let palette = resolved("background = \"#111111\"\nbg = \"#222222\"\n");
        assert_eq!(palette.get("background").as_deref(), Some("#111111"));
        // The legacy spelling is republished from the canonical value.
        assert_eq!(palette.get("bg").as_deref(), Some("#111111"));
    }

    #[test]
    fn fills_semantic_names_from_ansi_slots() {
        let palette = resolved("color0 = \"#111111\"\ncolor1 = \"#ff0000\"\ncolor7 = \"#eeeeee\"\n");
        assert_eq!(palette.get("background").as_deref(), Some("#111111"));
        assert_eq!(palette.get("red").as_deref(), Some("#ff0000"));
        assert_eq!(palette.get("foreground").as_deref(), Some("#eeeeee"));
    }

    #[test]
    fn derives_missing_shades_by_blending() {
        let palette = resolved("background = \"#1a1b26\"\nforeground = \"#a9b1d6\"\nred = \"#f7768e\"\n");
        assert_eq!(palette.get("dark_background").as_deref(), Some("#14141d"));
        assert_eq!(palette.get("darker_background").as_deref(), Some("#0d0e13"));
        assert_eq!(palette.get("bright_red").as_deref(), Some("#f991a5"));
    }

    #[test]
    fn detects_mode_from_background_luminance() {
        assert_eq!(
            resolved("background = \"#1a1b26\"\n").get("mode").as_deref(),
            Some("dark")
        );
        assert_eq!(
            resolved("background = \"#eff1f5\"\n").get("mode").as_deref(),
            Some("light")
        );
    }

    #[test]
    fn a_stated_mode_beats_luminance() {
        let palette = resolved("mode = \"light\"\nbackground = \"#000000\"\n");
        assert_eq!(palette.get("mode").as_deref(), Some("light"));
        assert_eq!(palette.get("theme_type").as_deref(), Some("light"));
    }

    #[test]
    fn light_mode_marker_applies_when_the_theme_states_nothing() {
        let (palette, _) = Palette::parse("background = \"#000000\"\n");
        let (palette, _) = palette.resolve(true);
        assert_eq!(palette.get("mode").as_deref(), Some("light"));
    }

    #[test]
    fn reports_a_derivation_it_could_not_perform() {
        let (palette, _) = Palette::parse("background = \"#1a1b26\"\norange = notacolor\n");
        let (_, warnings) = palette.resolve(false);
        assert!(warnings.contains(&Warning::UnderivableColor {
            key: "brown".into(),
            source: "notacolor".into()
        }));
    }
}

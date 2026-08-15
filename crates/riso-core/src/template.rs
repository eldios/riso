//! The Omarchy template dialect.
//!
//! A template is a plain file where `{{ ... }}` tokens are replaced. It is not
//! Jinja: functions take positional arguments separated by spaces, there is no
//! control flow, and an unrecognized token is copied through verbatim rather
//! than raising. That last rule is upstream behaviour and is what lets a
//! template carry literal braces.
//!
//! Plain keys match a strict `{{ key }}` shape with exactly one space per side,
//! because upstream substitutes them as fixed strings. Function tokens are
//! whitespace-tolerant, because those are matched by pattern.

use crate::color::{mix, parse_amount, Rgb};
use crate::gradient;
use crate::palette::Palette;

const MIX_FUNCTIONS: &[&str] = &["mix", "mix_strip", "mix_rgb"];
const GRADIENT_FUNCTIONS: &[&str] = &["hypr_gradient", "gradient_start", "shell_gradient"];

/// Replace every recognized token in `template`.
pub fn render(template: &str, palette: &Palette) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];

        let Some(close) = after.find("}}") else {
            // An unterminated token is not a token.
            out.push_str(&rest[open..]);
            return out;
        };

        let body = &after[..close];
        let token_len = 2 + close + 2;
        match substitute(body, palette) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[open..open + token_len]),
        }
        rest = &rest[open + token_len..];
    }

    out.push_str(rest);
    out
}

/// Resolve the inside of a `{{ ... }}` token, or `None` to leave it in place.
fn substitute(body: &str, palette: &Palette) -> Option<String> {
    if let Some(name) = plain_key(body) {
        if let Some(value) = lookup(name, palette) {
            return Some(value);
        }
    }

    let parts: Vec<&str> = body.split_whitespace().collect();
    let function = *parts.first()?;

    if MIX_FUNCTIONS.contains(&function) {
        return apply_mix(&parts, palette);
    }
    if GRADIENT_FUNCTIONS.contains(&function) {
        return apply_gradient(&parts, palette);
    }
    None
}

/// The key inside a strict `{{ key }}` token.
fn plain_key(body: &str) -> Option<&str> {
    let inner = body.strip_prefix(' ')?.strip_suffix(' ')?;
    if inner.is_empty() || inner.contains(char::is_whitespace) {
        return None;
    }
    Some(inner)
}

/// A palette key, or the same key with a `_strip` or `_rgb` suffix.
///
/// An exact match wins, so a theme may define a key that itself ends in one of
/// those suffixes.
fn lookup(name: &str, palette: &Palette) -> Option<String> {
    if let Some(value) = palette.get(name) {
        return Some(value);
    }
    if let Some(base) = name.strip_suffix("_strip") {
        let value = palette.get(base)?;
        return Some(value.strip_prefix('#').unwrap_or(&value).to_owned());
    }
    if let Some(base) = name.strip_suffix("_rgb") {
        // Only a plain hex color has a decimal triple.
        let value = palette.get(base)?;
        return Rgb::from_hex(&value).ok().map(Rgb::to_decimal_triple);
    }
    None
}

fn apply_mix(parts: &[&str], palette: &Palette) -> Option<String> {
    let [function, start_key, end_key, raw_amount] = parts else {
        return None;
    };
    if !is_key(start_key) || !is_key(end_key) || !is_amount(raw_amount) {
        return None;
    }

    let start = Rgb::from_hex(&palette.get(start_key)?).ok()?;
    let end = Rgb::from_hex(&palette.get(end_key)?).ok()?;
    let blended = mix(start, end, parse_amount(raw_amount)?);

    Some(match *function {
        "mix_strip" => blended.to_hex().trim_start_matches('#').to_owned(),
        "mix_rgb" => blended.to_decimal_triple(),
        _ => blended.to_hex(),
    })
}

fn apply_gradient(parts: &[&str], palette: &Palette) -> Option<String> {
    let function = *parts.first()?;
    let reference = *parts.get(1)?;
    // A fallback may itself be a multi-word gradient, so keep the rest whole.
    let fallback = parts[2..].join(" ");

    Some(match function {
        "hypr_gradient" => gradient::hypr(palette, reference, &fallback),
        "gradient_start" => gradient::start(palette, reference, &fallback),
        _ => gradient::shell(palette, reference, &fallback),
    })
}

fn is_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `50`, `0.5` or `50%`. A leading dot or a sign is not accepted.
fn is_amount(value: &str) -> bool {
    let value = value.strip_suffix('%').unwrap_or(value);
    let mut sides = value.split('.');
    let whole = sides.next().unwrap_or_default();
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match sides.next() {
        None => true,
        Some(fraction) => {
            !fraction.is_empty()
                && fraction.chars().all(|c| c.is_ascii_digit())
                && sides.next().is_none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        let (palette, _) = Palette::parse(
            "background = \"#1a1b26\"\n\
             foreground = \"#a9b1d6\"\n\
             accent = \"#7aa2f7\"\n\
             mode = \"dark\"\n\
             fancy = \"rgba(33ccffee) rgba(00ff99ee) 45deg\"\n",
        );
        palette
    }

    fn render_str(template: &str) -> String {
        render(template, &palette())
    }

    #[test]
    fn substitutes_a_plain_key() {
        assert_eq!(render_str("bg={{ background }};"), "bg=#1a1b26;");
    }

    #[test]
    fn substitutes_strip_and_rgb_suffixes() {
        assert_eq!(render_str("{{ accent_strip }}"), "7aa2f7");
        assert_eq!(render_str("{{ accent_rgb }}"), "122,162,247");
    }

    #[test]
    fn leaves_rgb_alone_for_a_non_color_value() {
        // `mode` is a word, so it has no decimal triple.
        assert_eq!(render_str("{{ mode_rgb }}"), "{{ mode_rgb }}");
        // But stripping a word is still defined.
        assert_eq!(render_str("{{ mode_strip }}"), "dark");
    }

    #[test]
    fn requires_exactly_one_space_around_a_plain_key() {
        assert_eq!(render_str("{{background}}"), "{{background}}");
        assert_eq!(render_str("{{  background  }}"), "{{  background  }}");
    }

    #[test]
    fn copies_through_an_unknown_token() {
        assert_eq!(render_str("{{ nope }}"), "{{ nope }}");
    }

    #[test]
    fn copies_through_an_unterminated_token() {
        assert_eq!(render_str("a {{ background"), "a {{ background");
    }

    #[test]
    fn substitutes_every_occurrence() {
        assert_eq!(
            render_str("{{ accent }}/{{ accent }}"),
            "#7aa2f7/#7aa2f7"
        );
    }

    #[test]
    fn applies_mix_in_all_three_spellings() {
        assert_eq!(render_str("{{ mix background foreground 15% }}"), "#2f3240");
        assert_eq!(render_str("{{ mix_strip background foreground 15% }}"), "2f3240");
        assert_eq!(render_str("{{ mix_rgb background foreground 15% }}"), "47,50,64");
    }

    #[test]
    fn tolerates_extra_space_around_a_function() {
        assert_eq!(render_str("{{  mix background foreground 15%  }}"), "#2f3240");
    }

    #[test]
    fn refuses_a_mix_it_cannot_compute() {
        // A malformed amount is not a mix at all.
        assert_eq!(
            render_str("{{ mix background foreground .15 }}"),
            "{{ mix background foreground .15 }}"
        );
        // Neither is a missing key.
        assert_eq!(
            render_str("{{ mix background nope 15% }}"),
            "{{ mix background nope 15% }}"
        );
    }

    #[test]
    fn applies_gradient_helpers() {
        assert_eq!(render_str("{{ hypr_gradient accent }}"), "\"#7aa2f7\"");
        assert_eq!(
            render_str("{{ hypr_gradient fancy accent }}"),
            "{ colors = { \"rgba(33ccffee)\", \"rgba(00ff99ee)\" }, angle = 45 }"
        );
        assert_eq!(render_str("{{ shell_gradient accent }}"), "#7aa2f7");
        assert_eq!(render_str("{{ gradient_start fancy accent }}"), "#33ccff");
    }

    #[test]
    fn keeps_a_multi_word_gradient_fallback_together() {
        assert_eq!(
            render_str("{{ hypr_gradient missing rgba(595959aa) }}"),
            "\"rgba(595959aa)\""
        );
    }

    #[test]
    fn leaves_surrounding_text_untouched() {
        assert_eq!(
            render_str("a{{ accent }}b{{ nope }}c"),
            "a#7aa2f7b{{ nope }}c"
        );
    }
}

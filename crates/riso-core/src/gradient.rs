//! Gradient values, which a theme may write wherever a plain color is allowed.
//!
//! A gradient is whitespace-separated colors plus an optional `<n>deg` angle.
//! Each consumer needs a different spelling of the same value, which is why
//! there are three renderers rather than one.

use crate::color::to_shell_hex;
use crate::palette::Palette;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gradient {
    pub colors: Vec<String>,
    pub angle: Option<String>,
}

/// Split a gradient spec, resolving every color through the palette.
pub fn parse(palette: &Palette, spec: &str) -> Gradient {
    let mut gradient = Gradient::default();
    for part in spec.split_whitespace() {
        if is_angle(part) {
            gradient.angle = Some(part.trim_end_matches("deg").to_owned());
        } else {
            gradient
                .colors
                .push(palette.get(part).unwrap_or_else(|| part.to_owned()));
        }
    }
    gradient
}

/// `<n>deg`, `-45deg` or `12.5deg`.
fn is_angle(part: &str) -> bool {
    let Some(number) = part.strip_suffix("deg") else {
        return false;
    };
    let number = number.strip_prefix('-').unwrap_or(number);
    let mut sides = number.split('.');
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

/// Resolve a reference that may be a palette key, with an optional fallback
/// that is itself tried as a key before being taken literally.
pub fn resolve_ref(palette: &Palette, reference: &str, fallback: &str) -> String {
    if let Some(value) = palette.get(reference) {
        return value;
    }
    if !fallback.is_empty() {
        if let Some(value) = palette.get(fallback) {
            return value;
        }
        return fallback.to_owned();
    }
    reference.to_owned()
}

/// Hyprland's Lua config: a quoted string for one color, a table for several.
pub fn hypr(palette: &Palette, reference: &str, fallback: &str) -> String {
    let spec = resolve_ref(palette, reference, fallback);
    let gradient = parse(palette, &spec);

    match gradient.colors.len() {
        0 => format!("\"{spec}\""),
        1 => format!("\"{}\"", gradient.colors[0]),
        _ => {
            let colors = gradient
                .colors
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let angle = gradient
                .angle
                .map(|a| format!(", angle = {a}"))
                .unwrap_or_default();
            format!("{{ colors = {{ {colors} }}{angle} }}")
        }
    }
}

/// The shell's own spelling: the spec, normalized.
pub fn shell(palette: &Palette, reference: &str, fallback: &str) -> String {
    let spec = resolve_ref(palette, reference, fallback);
    let gradient = parse(palette, &spec);

    if gradient.colors.is_empty() {
        return spec;
    }
    let mut out = gradient.colors.join(" ");
    if let Some(angle) = gradient.angle {
        out.push_str(&format!(" {angle}deg"));
    }
    out
}

/// The first stop, for consumers that cannot render a gradient at all.
pub fn start(palette: &Palette, reference: &str, fallback: &str) -> String {
    let spec = resolve_ref(palette, reference, fallback);
    let gradient = parse(palette, &spec);

    match gradient.colors.first() {
        Some(first) => to_shell_hex(first),
        None => to_shell_hex(&spec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        let (palette, _) = Palette::parse(
            "accent = \"#7aa2f7\"\nfancy = \"rgba(33ccffee) rgba(00ff99ee) 45deg\"\n",
        );
        palette
    }

    #[test]
    fn recognizes_angles_only_in_their_exact_form() {
        assert!(is_angle("45deg"));
        assert!(is_angle("-45deg"));
        assert!(is_angle("12.5deg"));
        assert!(!is_angle("deg"));
        assert!(!is_angle("45"));
        assert!(!is_angle("45degrees"));
        assert!(!is_angle("4.5.6deg"));
    }

    #[test]
    fn splits_colors_from_the_angle() {
        let gradient = parse(&palette(), "rgba(33ccffee) rgba(00ff99ee) 45deg");
        assert_eq!(gradient.colors, ["rgba(33ccffee)", "rgba(00ff99ee)"]);
        assert_eq!(gradient.angle.as_deref(), Some("45"));
    }

    #[test]
    fn resolves_bare_words_through_the_palette() {
        assert_eq!(parse(&palette(), "accent").colors, ["#7aa2f7"]);
    }

    #[test]
    fn falls_back_to_a_key_then_to_a_literal() {
        let palette = palette();
        assert_eq!(resolve_ref(&palette, "missing", "accent"), "#7aa2f7");
        assert_eq!(
            resolve_ref(&palette, "missing", "rgba(595959aa)"),
            "rgba(595959aa)"
        );
        assert_eq!(resolve_ref(&palette, "missing", ""), "missing");
    }

    #[test]
    fn renders_hyprland_solid_colors_as_quoted_strings() {
        assert_eq!(hypr(&palette(), "accent", ""), "\"#7aa2f7\"");
        assert_eq!(
            hypr(&palette(), "missing", "rgba(595959aa)"),
            "\"rgba(595959aa)\""
        );
    }

    #[test]
    fn renders_hyprland_gradients_as_lua_tables() {
        assert_eq!(
            hypr(&palette(), "fancy", ""),
            "{ colors = { \"rgba(33ccffee)\", \"rgba(00ff99ee)\" }, angle = 45 }"
        );
    }

    #[test]
    fn renders_shell_gradients_as_a_flat_spec() {
        assert_eq!(
            shell(&palette(), "fancy", ""),
            "rgba(33ccffee) rgba(00ff99ee) 45deg"
        );
        assert_eq!(shell(&palette(), "accent", ""), "#7aa2f7");
    }

    #[test]
    fn reduces_a_gradient_to_its_first_stop() {
        assert_eq!(start(&palette(), "fancy", ""), "#33ccff");
        assert_eq!(start(&palette(), "accent", ""), "#7aa2f7");
    }
}

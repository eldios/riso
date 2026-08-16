//! Color values and the blending arithmetic the template dialect uses.
//!
//! The arithmetic here mirrors `bin/omarchy-theme-color` byte for byte,
//! including its half-up rounding, so a rendered file is identical to the one
//! the shell pipeline produces.

use crate::error::ColorError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn from_hex(s: &str) -> Result<Self, ColorError> {
        let body = s
            .strip_prefix('#')
            .ok_or_else(|| ColorError::MissingHash(s.to_owned()))?;
        if body.len() != 6 {
            return Err(ColorError::Length(s.to_owned()));
        }
        let byte = |i: usize| {
            u8::from_str_radix(&body[i..i + 2], 16).map_err(|_| ColorError::Digits(s.to_owned()))
        };
        Ok(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
        })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Decimal triple as consumed by `{{ key_rgb }}`, e.g. `30,30,46`.
    pub fn to_decimal_triple(self) -> String {
        format!("{},{},{}", self.r, self.g, self.b)
    }
}

/// A blend amount in `0.0..=1.0`.
///
/// `30%`, `0.30` and `30` all denote the same blend: a bare number above 1 is
/// read as a percentage, which is what lets `{{ mix a b 50 }}` work.
pub fn parse_amount(raw: &str) -> Option<f64> {
    let amount = match raw.strip_suffix('%') {
        Some(number) => number.trim().parse::<f64>().ok()? / 100.0,
        None => {
            let value = raw.trim().parse::<f64>().ok()?;
            if value > 1.0 {
                value / 100.0
            } else {
                value
            }
        }
    };
    Some(amount.clamp(0.0, 1.0))
}

/// Linear blend from `start` towards `end`.
pub fn mix(start: Rgb, end: Rgb, amount: f64) -> Rgb {
    let channel = |from: u8, to: u8| {
        let value = f64::from(from) * (1.0 - amount) + f64::from(to) * amount;
        // awk's int() truncates, so +0.5 is a half-up round for positive values.
        (value + 0.5).floor().clamp(0.0, 255.0) as u8
    };
    Rgb {
        r: channel(start.r, end.r),
        g: channel(start.g, end.g),
        b: channel(start.b, end.b),
    }
}

/// Normalize any color spelling a theme may use down to `#rrggbb`.
///
/// Values that match nothing are returned unchanged, so an unrecognized
/// spelling reaches the output instead of becoming an empty string.
pub fn to_shell_hex(value: &str) -> String {
    let value = value.trim();

    if let Some(body) = value.strip_prefix('#') {
        if (body.len() == 6 || body.len() == 8) && body.chars().all(|c| c.is_ascii_hexdigit()) {
            return format!("#{}", &body[..6]);
        }
    }

    if let Some(body) = strip_rgb_call(value) {
        if (body.len() == 6 || body.len() == 8) && body.chars().all(|c| c.is_ascii_hexdigit()) {
            return format!("#{}", &body[..6]);
        }
        if let Some(rgb) = parse_decimal_channels(body) {
            return rgb.to_hex();
        }
    }

    if let Some(body) = value.strip_prefix("0x") {
        if body.len() == 8 && body.chars().all(|c| c.is_ascii_hexdigit()) {
            // 0xAARRGGBB: the alpha leads, so the color starts at offset 2.
            return format!("#{}", &body[2..8]);
        }
    }

    value.to_owned()
}

/// Inner text of an `rgb(...)` or `rgba(...)` call, case-insensitively.
fn strip_rgb_call(value: &str) -> Option<&str> {
    let open = value.find('(')?;
    let name = &value[..open];
    if !name.eq_ignore_ascii_case("rgb") && !name.eq_ignore_ascii_case("rgba") {
        return None;
    }
    value.strip_suffix(')')?.get(open + 1..)
}

fn parse_decimal_channels(body: &str) -> Option<Rgb> {
    let mut parts = body.split(',');
    let mut channel = || -> Option<u8> {
        let raw = parts.next()?.trim();
        if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(raw.parse::<u32>().ok()?.min(255) as u8)
    };
    let rgb = Rgb {
        r: channel()?,
        g: channel()?,
        b: channel()?,
    };
    // A trailing alpha is allowed and ignored; anything else is not an rgb().
    match parts.next() {
        None => Some(rgb),
        Some(alpha) if !alpha.trim().is_empty() && parts.next().is_none() => Some(rgb),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Rgb {
        Rgb::from_hex(s).expect("valid hex")
    }

    #[test]
    fn parses_hex_and_round_trips() {
        let c = hex("#1a2b3c");
        assert_eq!(
            c,
            Rgb {
                r: 0x1a,
                g: 0x2b,
                b: 0x3c
            }
        );
        assert_eq!(c.to_hex(), "#1a2b3c");
    }

    #[test]
    fn rejects_malformed_hex() {
        assert_eq!(
            Rgb::from_hex("1a2b3c"),
            Err(ColorError::MissingHash("1a2b3c".into()))
        );
        assert_eq!(
            Rgb::from_hex("#1a2b"),
            Err(ColorError::Length("#1a2b".into()))
        );
        assert_eq!(
            Rgb::from_hex("#gggggg"),
            Err(ColorError::Digits("#gggggg".into()))
        );
    }

    #[test]
    fn renders_decimal_triple() {
        assert_eq!(hex("#1e1e2e").to_decimal_triple(), "30,30,46");
    }

    // Expected values captured from the upstream awk implementation.
    #[test]
    fn mixes_like_upstream() {
        let cases = [
            ("#eb927b", "#000000", "50%", "#76493e"),
            ("#eb927b", "#000000", "0.5", "#76493e"),
            ("#eb927b", "#000000", "50", "#76493e"),
            ("#1a1b26", "#000000", "25%", "#14141d"),
            ("#1a1b26", "#000000", "50%", "#0d0e13"),
            ("#f7768e", "#ffffff", "20%", "#f991a5"),
            ("#1a1b26", "#a9b1d6", "15%", "#2f3240"),
            ("#000000", "#ffffff", "0%", "#000000"),
            ("#000000", "#ffffff", "100%", "#ffffff"),
            ("#000000", "#ffffff", "1", "#ffffff"),
            // 1.5 rounds up, not to even.
            ("#010101", "#020202", "0.5", "#020202"),
        ];
        for (start, end, amount, expected) in cases {
            let got = mix(hex(start), hex(end), parse_amount(amount).expect("amount"));
            assert_eq!(got.to_hex(), expected, "mix {start} {end} {amount}");
        }
    }

    #[test]
    fn clamps_out_of_range_amounts() {
        assert_eq!(parse_amount("-10%"), Some(0.0));
        assert_eq!(parse_amount("400%"), Some(1.0));
        assert_eq!(parse_amount("not a number"), None);
    }

    #[test]
    fn normalizes_every_color_spelling() {
        assert_eq!(to_shell_hex("#7aa2f7"), "#7aa2f7");
        assert_eq!(to_shell_hex("#7aa2f7ee"), "#7aa2f7");
        assert_eq!(to_shell_hex("rgba(33ccffee)"), "#33ccff");
        assert_eq!(to_shell_hex("rgb(33ccff)"), "#33ccff");
        assert_eq!(to_shell_hex("rgb(122,162,247)"), "#7aa2f7");
        assert_eq!(to_shell_hex("rgba(122,162,247,0.5)"), "#7aa2f7");
        assert_eq!(to_shell_hex("0xee33ccff"), "#33ccff");
        // Channels above 255 are clamped rather than wrapped.
        assert_eq!(to_shell_hex("rgb(999,0,0)"), "#ff0000");
        // Anything unrecognized survives untouched.
        assert_eq!(to_shell_hex("teal"), "teal");
    }
}

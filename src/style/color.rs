//! CSS colors, snapped to the 16-color EGA palette.

use crate::palette::Ega;

/// Parses a CSS color and returns the EGA color it means.
///
/// - The 16 DOS names (`blue`, `lightgray`, `brown`, …) mean exactly that
///   EGA color, even where CSS defines the name differently. Hyphens,
///   underscores and `grey` are accepted: `light-grey` is `lightgray`.
/// - The other basic CSS names (`navy`, `teal`, `silver`, …), `#rgb`,
///   `#rrggbb` and `rgb(r, g, b)` snap to the nearest EGA color.
pub fn parse_color(value: &str) -> Option<Ega> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let (r, g, b) = parse_hex(hex)?;
        return Some(Ega::nearest(r, g, b));
    }
    if let Some(args) = value
        .strip_prefix("rgb(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let (r, g, b) = parse_rgb_args(args)?;
        return Some(Ega::nearest(r, g, b));
    }
    named(&value)
}

fn named(name: &str) -> Option<Ega> {
    let key: String = name
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .collect::<String>()
        .replace("grey", "gray");
    Some(match key.as_str() {
        "black" => Ega::Black,
        "blue" | "navy" => Ega::Blue,
        "green" => Ega::Green,
        "cyan" | "teal" => Ega::Cyan,
        "red" | "maroon" => Ega::Red,
        "magenta" | "purple" => Ega::Magenta,
        "brown" | "olive" | "orange" => Ega::Brown,
        "lightgray" | "silver" => Ega::LightGray,
        "darkgray" | "gray" => Ega::DarkGray,
        "lightblue" => Ega::LightBlue,
        "lightgreen" | "lime" => Ega::LightGreen,
        "lightcyan" | "aqua" => Ega::LightCyan,
        "lightred" => Ega::LightRed,
        "lightmagenta" | "fuchsia" | "pink" => Ega::LightMagenta,
        "yellow" => Ega::Yellow,
        "white" => Ega::White,
        _ => return None,
    })
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 => Some((digit(0)? * 17, digit(1)? * 17, digit(2)? * 17)),
        6 => Some((pair(0)?, pair(2)?, pair(4)?)),
        _ => None,
    }
}

fn parse_rgb_args(args: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<u8> = args
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts[..] {
        [r, g, b] => Some((r, g, b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_names_mean_ega_colors() {
        assert_eq!(parse_color("blue"), Some(Ega::Blue));
        assert_eq!(parse_color("LightGray"), Some(Ega::LightGray));
        assert_eq!(parse_color("light-grey"), Some(Ega::LightGray));
        assert_eq!(parse_color("dark_gray"), Some(Ega::DarkGray));
        assert_eq!(parse_color("brown"), Some(Ega::Brown));
    }

    #[test]
    fn css_names_snap() {
        assert_eq!(parse_color("navy"), Some(Ega::Blue));
        assert_eq!(parse_color("aqua"), Some(Ega::LightCyan));
        assert_eq!(parse_color("silver"), Some(Ega::LightGray));
        assert_eq!(parse_color("gray"), Some(Ega::DarkGray));
    }

    #[test]
    fn hex_and_rgb_snap_to_nearest() {
        assert_eq!(parse_color("#00a"), Some(Ega::Blue));
        assert_eq!(parse_color("#FFFF55"), Some(Ega::Yellow));
        assert_eq!(parse_color("#b05a08"), Some(Ega::Brown));
        assert_eq!(parse_color("rgb(250, 250, 250)"), Some(Ega::White));
        assert_eq!(parse_color("rgb(0 170 0)"), Some(Ega::Green));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("#ggg"), None);
        assert_eq!(parse_color("rgb(1, 2)"), None);
        assert_eq!(parse_color("rgb(300, 0, 0)"), None);
        assert_eq!(parse_color("beige"), None);
    }
}

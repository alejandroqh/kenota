//! Parsing individual CSS property values.

use super::color::parse_color;
use super::parse::split_top_level;
use super::{Edges, Length, ListStyleType};
use crate::palette::Ega;

/// A case-insensitive keyword lookup.
pub(super) fn keyword<T: Copy>(value: &str, table: &[(&str, T)]) -> Option<T> {
    let value = value.trim();
    table
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(value))
        .map(|(_, v)| *v)
}

/// A length in cells. Plain numbers, `ch`, `em`, `rem` and `lh` all mean
/// cells. Zero is accepted with any unit (`0px`).
#[allow(clippy::cast_possible_truncation)]
pub(super) fn cells(value: &str) -> Option<i32> {
    let value = value.trim().to_ascii_lowercase();
    let unit_start = value
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_start);
    let number: f32 = number.parse().ok()?;
    if !number.is_finite() {
        return None;
    }
    if number == 0.0 {
        return Some(0);
    }
    matches!(unit, "" | "ch" | "em" | "rem" | "lh").then(|| number.round() as i32)
}

/// `width`: cells, a percentage, or `auto` (`Some(None)`).
pub(super) fn length(value: &str) -> Option<Option<Length>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    if let Some(percent) = value.strip_suffix('%') {
        let p: f32 = percent.trim().parse().ok()?;
        return p.is_finite().then_some(Some(Length::Percent(p)));
    }
    cells(value).map(|n| Some(Length::Cells(n)))
}

/// The `margin`/`padding` shorthand: 1 to 4 values, clockwise from the top.
pub(super) fn edges(value: &str) -> Option<Edges> {
    let parts = split_top_level(value, ' ')
        .into_iter()
        .map(cells)
        .collect::<Option<Vec<_>>>()?;
    let (top, right, bottom, left) = match parts[..] {
        [all] => (all, all, all, all),
        [vertical, horizontal] => (vertical, horizontal, vertical, horizontal),
        [top, horizontal, bottom] => (top, horizontal, bottom, horizontal),
        [top, right, bottom, left] => (top, right, bottom, left),
        _ => return None,
    };
    Some(Edges {
        top,
        right,
        bottom,
        left,
    })
}

/// `background`: a color, or `transparent`/`none` (`Some(None)`).
pub(super) fn background(value: &str) -> Option<Option<Ega>> {
    if keyword(value, &[("transparent", ()), ("none", ())]).is_some() {
        return Some(None);
    }
    parse_color(value).map(Some)
}

pub(super) fn font_weight(value: &str) -> Option<bool> {
    if let Some(bold) = keyword(
        value,
        &[
            ("bold", true),
            ("bolder", true),
            ("normal", false),
            ("lighter", false),
        ],
    ) {
        return Some(bold);
    }
    value.trim().parse::<u16>().ok().map(|weight| weight >= 600)
}

/// `text-decoration`: returns (underline, line-through).
pub(super) fn text_decoration(value: &str) -> Option<(bool, bool)> {
    let (mut underline, mut line_through) = (false, false);
    for part in split_top_level(value, ' ') {
        match part.to_ascii_lowercase().as_str() {
            "none" => {}
            "underline" => underline = true,
            "line-through" => line_through = true,
            _ => return None,
        }
    }
    Some((underline, line_through))
}

pub(super) fn list_style_type(value: &str) -> Option<ListStyleType> {
    let value = value.trim();
    if let Some(text) = unquote(value) {
        return Some(ListStyleType::Text(text.to_string()));
    }
    keyword(
        value,
        &[
            ("none", 0),
            ("disc", 1),
            ("circle", 2),
            ("square", 3),
            ("decimal", 4),
            ("lower-alpha", 5),
            ("lower-latin", 5),
            ("upper-alpha", 6),
            ("upper-latin", 6),
            ("lower-roman", 7),
            ("upper-roman", 8),
        ],
    )
    .map(|n| match n {
        0 => ListStyleType::None,
        1 => ListStyleType::Disc,
        2 => ListStyleType::Circle,
        3 => ListStyleType::Square,
        4 => ListStyleType::Decimal,
        5 => ListStyleType::LowerAlpha,
        6 => ListStyleType::UpperAlpha,
        7 => ListStyleType::LowerRoman,
        _ => ListStyleType::UpperRoman,
    })
}

/// The inside of a `"…"` or `'…'` string.
fn unquote(value: &str) -> Option<&str> {
    let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    value
        .strip_prefix(quote)?
        .strip_suffix(quote)
        .filter(|_| value.len() >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_accepts_cell_units_and_zero() {
        assert_eq!(cells("2"), Some(2));
        assert_eq!(cells(" 3ch "), Some(3));
        assert_eq!(cells("1em"), Some(1));
        assert_eq!(cells("1.5rem"), Some(2));
        assert_eq!(cells("-1"), Some(-1));
        assert_eq!(cells("0px"), Some(0));
        assert_eq!(cells("10px"), None);
        assert_eq!(cells("auto"), None);
        assert_eq!(cells(""), None);
    }

    #[test]
    fn edges_shorthand() {
        let e = |top, right, bottom, left| {
            Some(Edges {
                top,
                right,
                bottom,
                left,
            })
        };
        assert_eq!(edges("1"), e(1, 1, 1, 1));
        assert_eq!(edges("1 2"), e(1, 2, 1, 2));
        assert_eq!(edges("1 2 3"), e(1, 2, 3, 2));
        assert_eq!(edges("1 2 3 4"), e(1, 2, 3, 4));
        assert_eq!(edges("1 2 3 4 5"), None);
        assert_eq!(edges("1 auto"), None);
    }

    #[test]
    fn width_values() {
        assert_eq!(length("auto"), Some(None));
        assert_eq!(length("50%"), Some(Some(Length::Percent(50.0))));
        assert_eq!(length("30"), Some(Some(Length::Cells(30))));
        assert_eq!(length("wide"), None);
    }

    #[test]
    fn decorations_and_weights() {
        assert_eq!(
            text_decoration("underline line-through"),
            Some((true, true))
        );
        assert_eq!(text_decoration("none"), Some((false, false)));
        assert_eq!(text_decoration("wavy"), None);
        assert_eq!(font_weight("700"), Some(true));
        assert_eq!(font_weight("400"), Some(false));
        assert_eq!(font_weight("BOLD"), Some(true));
    }

    #[test]
    fn list_style_types() {
        assert_eq!(list_style_type("square"), Some(ListStyleType::Square));
        assert_eq!(
            list_style_type("\"→ \""),
            Some(ListStyleType::Text("→ ".into()))
        );
        assert_eq!(list_style_type("'"), None);
        assert_eq!(list_style_type("hexagon"), None);
    }
}

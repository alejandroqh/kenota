//! Parsing individual CSS property values.

use unicode_width::UnicodeWidthChar;

use super::color::parse_color;
use super::parse::split_top_level;
use super::{AutoMargin, BorderSide, BorderStyle, Edges, Length, ListStyleType, Pattern, Shadow};
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

/// 1 to 4 values, clockwise from the top, the way CSS shorthands spell them.
pub(super) fn four<T: Copy>(value: &str, parse: impl Fn(&str) -> Option<T>) -> Option<[T; 4]> {
    let parts = split_top_level(value, ' ')
        .into_iter()
        .map(parse)
        .collect::<Option<Vec<_>>>()?;
    Some(match parts[..] {
        [all] => [all, all, all, all],
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] => [top, right, bottom, left],
        _ => return None,
    })
}

/// The `padding` shorthand.
pub(super) fn edges(value: &str) -> Option<Edges> {
    four(value, cells).map(Edges::from)
}

/// One margin: cells, or `auto` (`Some(None)`).
pub(super) fn margin(value: &str) -> Option<Option<i32>> {
    if value.trim().eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    cells(value).map(Some)
}

/// The `margin` shorthand. Vertical `auto` margins are 0, as in CSS block flow.
pub(super) fn margins(value: &str) -> Option<(Edges, AutoMargin)> {
    let [top, right, bottom, left] = four(value, margin)?;
    let edges = Edges::from([top, right, bottom, left].map(|m| m.unwrap_or(0)));
    let auto = AutoMargin {
        left: left.is_none(),
        right: right.is_none(),
    };
    Some((edges, auto))
}

/// `height`: cells, or `auto` (`Some(None)`).
pub(super) fn height(value: &str) -> Option<Option<i32>> {
    margin(value)
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

pub(super) fn border_style(value: &str) -> Option<BorderStyle> {
    keyword(
        value,
        &[
            ("none", BorderStyle::None),
            ("hidden", BorderStyle::None),
            ("single", BorderStyle::Single),
            ("solid", BorderStyle::Single),
            ("double", BorderStyle::Double),
            ("thick", BorderStyle::Thick),
        ],
    )
}

/// A border width. Text borders are always one cell, so this only tells
/// whether there is one: `0` is none, anything else (`1`, `thin`, `1px`) is one.
fn border_width(value: &str) -> Option<bool> {
    if keyword(value, &[("thin", ()), ("medium", ())]).is_some() {
        return Some(true);
    }
    let value = value.trim().to_ascii_lowercase();
    let unit_start = value
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_start);
    let number: f32 = number.parse().ok()?;
    matches!(unit, "" | "px" | "ch" | "em" | "rem").then_some(number > 0.0)
}

/// The `border` shorthand (and `border-top`, …): a style, a color and a
/// width in any order. Like CSS, a border without a style is not drawn.
pub(super) fn border(value: &str) -> Option<BorderSide> {
    let (mut style, mut color, mut visible) = (None, None, true);
    for part in split_top_level(value, ' ') {
        if let Some(s) = border_style(part) {
            if style.replace(s).is_some() {
                return None;
            }
        } else if let Some(c) = parse_color(part) {
            if color.replace(c).is_some() {
                return None;
            }
        } else {
            visible &= border_width(part)?;
        }
    }
    let style = style.filter(|_| visible).unwrap_or_default();
    Some(BorderSide { style, color })
}

/// `box-shadow`: `none`, or a right and down offset in cells and an
/// optional color (black by default).
pub(super) fn shadow(value: &str) -> Option<Option<Shadow>> {
    if keyword(value, &[("none", ())]).is_some() {
        return Some(None);
    }
    let mut offsets = Vec::new();
    let mut color = None;
    for part in split_top_level(value, ' ') {
        if let Some(n) = cells(part) {
            offsets.push(n);
        } else if color.replace(parse_color(part)?).is_some() {
            return None;
        }
    }
    let [x, y] = offsets[..] else { return None };
    Some(Some(Shadow {
        x,
        y,
        color: color.unwrap_or(Ega::Black),
    }))
}

/// `--fill`: `none`, or a character (`"▒"`, `▒`, or `light`/`medium`/`dark`
/// for `░▒▓`) and an optional color.
pub(super) fn pattern(value: &str) -> Option<Option<Pattern>> {
    let parts = split_top_level(value, ' ');
    let (symbol, color) = match parts[..] {
        [symbol] => (symbol, None),
        [symbol, color] => (symbol, Some(parse_color(color)?)),
        _ => return None,
    };
    if symbol.eq_ignore_ascii_case("none") && color.is_none() {
        return Some(None);
    }
    let symbol = match keyword(symbol, &[("light", '░'), ("medium", '▒'), ("dark", '▓')]) {
        Some(c) => c,
        None => {
            let mut chars = unquote(symbol).unwrap_or(symbol).chars();
            let c = chars.next()?;
            (chars.next().is_none() && c.width() == Some(1)).then_some(c)?
        }
    };
    Some(Some(Pattern { symbol, color }))
}

/// `--font`: a font name (bare or quoted, any case), or `none` (`Some(None)`).
pub(super) fn font(value: &str) -> Option<Option<String>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(None);
    }
    let name = unquote(value).unwrap_or(value);
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    valid.then(|| Some(name.to_lowercase()))
}

/// `z-index`: an integer, or `auto` (0).
pub(super) fn z_index(value: &str) -> Option<i32> {
    if value.trim().eq_ignore_ascii_case("auto") {
        return Some(0);
    }
    value.trim().parse().ok()
}

/// A non-negative number, as `flex-grow` takes.
pub(super) fn grow(value: &str) -> Option<f32> {
    let n: f32 = value.trim().parse().ok()?;
    (n.is_finite() && n >= 0.0).then_some(n)
}

/// The `flex` shorthand: returns the grow factor and the basis (`None`
/// is `auto`). As in CSS, `flex: 1` means grow from a basis of 0, so items
/// with equal factors get equal widths. A shrink factor may sit between
/// the two; it is accepted and ignored.
pub(super) fn flex(value: &str) -> Option<(f32, Option<Length>)> {
    if let Some(grow) = keyword(value, &[("none", 0.0), ("initial", 0.0), ("auto", 1.0)]) {
        return Some((grow, None));
    }
    let zero = Some(Length::Cells(0));
    match split_top_level(value, ' ')[..] {
        [one] => match grow(one) {
            Some(n) => Some((n, zero)),
            None => Some((1.0, flex_basis(one)?)),
        },
        [first, second] => match grow(second) {
            Some(_) => Some((grow(first)?, zero)),
            None => Some((grow(first)?, flex_basis(second)?)),
        },
        [first, second, basis] => {
            grow(second)?;
            Some((grow(first)?, flex_basis(basis)?))
        }
        _ => None,
    }
}

/// `flex-basis`: a non-negative width, or `auto` (`Some(None)`).
pub(super) fn flex_basis(value: &str) -> Option<Option<Length>> {
    length(value).filter(|basis| match basis {
        Some(Length::Cells(n)) => *n >= 0,
        Some(Length::Percent(p)) => *p >= 0.0,
        None => true,
    })
}

/// `gap`: one value, or a row gap and a column gap (only columns matter in a flex row).
pub(super) fn gap(value: &str) -> Option<i32> {
    match split_top_level(value, ' ')[..] {
        [gap] | [_, gap] => cells(gap),
        _ => None,
    }
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

    #[test]
    fn border_shorthand_in_any_order() {
        let side = |style, color| Some(BorderSide { style, color });
        assert_eq!(border("double"), side(BorderStyle::Double, None));
        assert_eq!(
            border("yellow thick"),
            side(BorderStyle::Thick, Some(Ega::Yellow))
        );
        assert_eq!(
            border("1px solid #fff"),
            side(BorderStyle::Single, Some(Ega::White))
        );
        assert_eq!(border("0 single"), side(BorderStyle::None, None));
        assert_eq!(border("red"), side(BorderStyle::None, Some(Ega::Red)));
        assert_eq!(border("single double"), None);
        assert_eq!(border("dashed"), None);
    }

    #[test]
    fn shadows() {
        assert_eq!(
            shadow("2 1"),
            Some(Some(Shadow {
                x: 2,
                y: 1,
                color: Ega::Black
            }))
        );
        assert_eq!(
            shadow("darkgray -1 1"),
            Some(Some(Shadow {
                x: -1,
                y: 1,
                color: Ega::DarkGray
            }))
        );
        assert_eq!(shadow("none"), Some(None));
        assert_eq!(shadow("2"), None);
        assert_eq!(shadow("2 1 4 black"), None);
    }

    #[test]
    fn fill_patterns() {
        let some = |symbol, color| Some(Some(Pattern { symbol, color }));
        assert_eq!(pattern("\"░\" blue"), some('░', Some(Ega::Blue)));
        assert_eq!(pattern("▓ red"), some('▓', Some(Ega::Red)));
        assert_eq!(pattern("medium"), some('▒', None));
        assert_eq!(pattern("'x'"), some('x', None));
        assert_eq!(pattern("none"), Some(None));
        assert_eq!(pattern("\"ab\""), None);
        assert_eq!(pattern("\"漢\""), None, "double width");
    }

    #[test]
    fn auto_margins() {
        let (edges, auto) = margins("1 auto 0").unwrap();
        assert_eq!(edges, Edges::from([1, 0, 0, 0]));
        assert_eq!(
            auto,
            AutoMargin {
                left: true,
                right: true
            }
        );
        let (edges, auto) = margins("auto 2").unwrap();
        assert_eq!(edges, Edges::from([0, 2, 0, 2]), "vertical auto is 0");
        assert_eq!(auto, AutoMargin::default());
    }

    #[test]
    fn font_names() {
        assert_eq!(font("Block"), Some(Some("block".into())));
        assert_eq!(font("'my-font_2'"), Some(Some("my-font_2".into())));
        assert_eq!(font("none"), Some(None));
        assert_eq!(font("two words"), None);
        assert_eq!(font("\"\""), None);
    }

    #[test]
    fn flex_and_gap() {
        let zero = Some(Length::Cells(0));
        assert_eq!(flex("2"), Some((2.0, zero)));
        assert_eq!(flex("1 1"), Some((1.0, zero)));
        assert_eq!(flex("1 1 auto"), Some((1.0, None)));
        assert_eq!(flex("2 30%"), Some((2.0, Some(Length::Percent(30.0)))));
        assert_eq!(
            flex("20"),
            Some((20.0, zero)),
            "a lone number is the grow factor"
        );
        assert_eq!(flex("50%"), Some((1.0, Some(Length::Percent(50.0)))));
        assert_eq!(flex("auto"), Some((1.0, None)));
        assert_eq!(flex("none"), Some((0.0, None)));
        assert_eq!(flex("-1"), None);
        assert_eq!(flex("1 wide"), None);
        assert_eq!(gap("2"), Some(2));
        assert_eq!(gap("1 3"), Some(3));
        assert_eq!(z_index("auto"), Some(0));
        assert_eq!(z_index("-2"), Some(-2));
    }
}

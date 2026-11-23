//! List markers: bullets and numbers for `display: list-item` children.

use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use super::text_style;
use crate::style::{Display, ListStyleType, StyledNode};

/// A marker drawn left of a list item's first line.
#[derive(Debug, PartialEq)]
pub(super) struct Marker {
    /// Right-aligned to `width`, so numbers line up: ` 9. ` and `10. `.
    pub text: String,
    /// Columns reserved for markers in this list; the item's text starts after them.
    pub width: i32,
    pub style: Style,
}

/// One entry per child of `list`: a marker for list items, `None` for the rest.
pub(super) fn for_children(list: &StyledNode) -> Vec<Option<Marker>> {
    let start = list
        .attr("start")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(1);
    let mut number = start;

    let labels: Vec<Option<(String, Style)>> = list
        .children
        .iter()
        .map(|child| {
            if child.tag().is_none() || child.style.display != Display::ListItem {
                return None;
            }
            let text = label(&child.style.list_style_type, number);
            number += 1;
            Some((text, text_style(&child.style, None)))
        })
        .collect();

    let width = labels
        .iter()
        .flatten()
        .map(|(text, _)| text.width())
        .max()
        .unwrap_or(0);

    labels
        .into_iter()
        .map(|label| {
            label.map(|(text, style)| Marker {
                text: format!("{text:>width$}"),
                width: i32::try_from(width).unwrap_or(0),
                style,
            })
        })
        .collect()
}

/// The marker text for item number `n`, including its trailing space.
fn label(kind: &ListStyleType, n: i64) -> String {
    match kind {
        ListStyleType::None => String::new(),
        ListStyleType::Disc => "• ".into(),
        ListStyleType::Circle => "○ ".into(),
        ListStyleType::Square => "■ ".into(),
        ListStyleType::Text(text) => text.clone(),
        ListStyleType::Decimal => format!("{n}. "),
        ListStyleType::LowerAlpha => format!("{}. ", alpha(n).to_lowercase()),
        ListStyleType::UpperAlpha => format!("{}. ", alpha(n)),
        ListStyleType::LowerRoman => format!("{}. ", roman(n).to_lowercase()),
        ListStyleType::UpperRoman => format!("{}. ", roman(n)),
    }
}

/// 1 → A, 26 → Z, 27 → AA. Falls back to digits outside 1…
fn alpha(n: i64) -> String {
    if n < 1 {
        return n.to_string();
    }
    let mut n = n;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(char::from(b'A' + u8::try_from(n % 26).unwrap_or(0)));
        n /= 26;
    }
    out.iter().rev().collect()
}

/// Roman numerals for 1…3999; digits outside that range.
fn roman(n: i64) -> String {
    const NUMERALS: [(i64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    if !(1..=3999).contains(&n) {
        return n.to_string();
    }
    let mut n = n;
    let mut out = String::new();
    for (value, numeral) in NUMERALS {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::*;
    use crate::html;
    use crate::layout::layout;
    use crate::render::{buffer_text, paint};

    fn text(html: &str, width: u16, height: u16) -> String {
        let source = format!("<style>body {{ margin: 0 }}</style>{html}");
        let slide = html::parse(&source, None);
        assert!(slide.warnings.is_empty(), "{:?}", slide.warnings);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        paint(&layout(&slide.root, area).items, &mut buf, area);
        buffer_text(&buf)
    }

    #[test]
    fn numbering_helpers() {
        assert_eq!(alpha(1), "A");
        assert_eq!(alpha(26), "Z");
        assert_eq!(alpha(28), "AB");
        assert_eq!(alpha(0), "0");
        assert_eq!(roman(1994), "MCMXCIV");
        assert_eq!(roman(4000), "4000");
    }

    #[test]
    fn bullets_hang_and_nest() {
        assert_eq!(
            text(
                "<ul><li>first item wraps here</li><li>second<ul><li>inner</li></ul></li></ul><p>after</p>",
                16,
                7
            ),
            "  • first item\n    wraps here\n  • second\n      ■ inner\n\nafter\n"
        );
    }

    #[test]
    fn numbers_align_and_respect_start() {
        assert_eq!(
            text("<ol start='9'><li>nine</li><li>ten</li></ol>", 12, 2),
            "   9. nine\n  10. ten"
        );
    }

    #[test]
    fn custom_and_roman_markers() {
        assert_eq!(
            text(
                "<ul style='list-style-type: \"► \"'><li>a</li></ul>
                 <ol style='list-style: upper-roman'><li>a</li><li>b</li><li>c</li><li>d</li></ol>",
                12,
                6
            ),
            "  ► a\n\n    I. a\n   II. b\n  III. c\n   IV. d"
        );
    }

    #[test]
    fn list_style_none_has_no_indent() {
        assert_eq!(
            text(
                "<ul style='list-style: none; padding-left: 0'><li>plain</li></ul>",
                10,
                1
            ),
            "plain"
        );
    }
}

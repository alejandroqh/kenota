//! Box-drawing borders, and the junction characters where lines meet.

use ratatui::style::Style;

use super::{Area, Item};
use crate::palette::Ega;
use crate::style::{Border, BorderSide, BorderStyle};

/// A horizontal line. Thick lines use the outer half of the cell: `▀` on
/// top edges, `▄` on bottom edges.
pub(super) fn horizontal(style: BorderStyle, bottom: bool) -> char {
    match style {
        BorderStyle::None => ' ',
        BorderStyle::Single => '─',
        BorderStyle::Double => '═',
        BorderStyle::Thick if bottom => '▄',
        BorderStyle::Thick => '▀',
    }
}

pub(super) fn vertical(style: BorderStyle) -> char {
    match style {
        BorderStyle::None => ' ',
        BorderStyle::Single => '│',
        BorderStyle::Double => '║',
        BorderStyle::Thick => '█',
    }
}

/// The character where lines meet, given the style of each arm. `bottom`
/// says whether the point lies on a bottom edge (it picks `▄` over `▀`).
pub(super) fn junction(
    up: BorderStyle,
    down: BorderStyle,
    left: BorderStyle,
    right: BorderStyle,
    bottom: bool,
) -> char {
    use BorderStyle::{Double, None, Single, Thick};
    // Corners, tees and crosses for each (horizontal, vertical) style pair.
    const SHAPES: [[char; 9]; 4] = [
        ['┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼'],
        ['╒', '╕', '╘', '╛', '╞', '╡', '╤', '╧', '╪'],
        ['╓', '╖', '╙', '╜', '╟', '╢', '╥', '╨', '╫'],
        ['╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩', '╬'],
    ];

    let v = up.max(down);
    let h = left.max(right);
    match (h, v) {
        (_, Thick) => return '█',
        (Thick, _) => return horizontal(Thick, bottom),
        (None, None) => return ' ',
        (h, None) => return horizontal(h, bottom),
        (None, v) => return vertical(v),
        _ => {}
    }
    let set = match (h, v) {
        (Double, Single) => 1,
        (Single, Double) => 2,
        (Double, Double) => 3,
        _ => 0,
    };
    let (u, d, l, r) = (up != None, down != None, left != None, right != None);
    let shape = match (u, d, l, r) {
        (false, true, false, true) => 0,
        (false, true, true, false) => 1,
        (true, false, false, true) => 2,
        (true, false, true, false) => 3,
        (true, true, false, true) => 4,
        (true, true, true, false) => 5,
        (false, true, true, true) => 6,
        (true, false, true, true) => 7,
        _ => 8,
    };
    SHAPES[set][shape]
}

/// Draws `border` around `area`. `text_color` colors sides without a color of their own.
pub(super) fn draw(border: &Border, text_color: Ega, area: Area, out: &mut Vec<Item>) {
    let widths = border.widths();
    if area.width <= 0 || area.height <= 0 || widths == Default::default() {
        return;
    }
    let style = |side: &BorderSide| Style::new().fg(side.color.unwrap_or(text_color).color());
    let mut text = |x: i32, y: i32, text: String, side: &BorderSide| {
        out.push(Item::Text {
            x,
            y,
            text,
            style: style(side),
        });
    };

    let edge = |side: &BorderSide, bottom: bool| {
        let (up, down) = if bottom {
            (border.left.style, BorderStyle::None)
        } else {
            (BorderStyle::None, border.left.style)
        };
        let first = junction(up, down, BorderStyle::None, side.style, bottom);
        let (up, down) = if bottom {
            (border.right.style, BorderStyle::None)
        } else {
            (BorderStyle::None, border.right.style)
        };
        let last = junction(up, down, side.style, BorderStyle::None, bottom);
        let middle = horizontal(side.style, bottom);
        (0..area.width)
            .map(|i| match i {
                0 => first,
                i if i == area.width - 1 => last,
                _ => middle,
            })
            .collect::<String>()
    };

    if widths.top > 0 {
        text(area.x, area.y, edge(&border.top, false), &border.top);
    }
    if widths.bottom > 0 && area.height > widths.top {
        let y = area.bottom() - 1;
        text(area.x, y, edge(&border.bottom, true), &border.bottom);
    }
    for y in area.y + widths.top..area.bottom() - widths.bottom {
        if widths.left > 0 {
            text(area.x, y, vertical(border.left.style).into(), &border.left);
        }
        if widths.right > 0 && area.width > widths.left {
            let x = area.right() - 1;
            text(x, y, vertical(border.right.style).into(), &border.right);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BorderStyle::{Double, None, Single, Thick};

    #[test]
    fn junctions_mix_single_and_double() {
        assert_eq!(junction(None, Single, None, Single, false), '┌');
        assert_eq!(junction(None, Single, Double, Double, false), '╤');
        assert_eq!(junction(Double, Double, None, Single, false), '╟');
        assert_eq!(junction(Single, Single, Single, Single, false), '┼');
        assert_eq!(junction(Double, None, Double, None, true), '╝');
        assert_eq!(junction(None, None, None, Single, false), '─');
        assert_eq!(junction(Single, None, None, None, false), '│');
    }

    #[test]
    fn thick_lines_use_blocks() {
        assert_eq!(junction(None, Thick, None, Thick, false), '█');
        assert_eq!(junction(None, Single, Thick, Thick, false), '▀');
        assert_eq!(junction(Single, None, Thick, Thick, true), '▄');
    }
}

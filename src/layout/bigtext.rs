//! Big text: a block's inline content drawn with a FIGlet font.
//!
//! The font comes from `--font` (inherited; `<h1>` uses `block`) or the
//! `data-font` attribute. Whitespace collapses as in normal text, `<br>`
//! starts a new line, and lines wrap between words. Colors come from the
//! inline elements as usual, so `<h1>Big <em>news</em></h1>` draws "news"
//! in the emphasis color. Blank cells are left alone, so a pattern or
//! background behind big text shows between the letters.

use ratatui::style::Style;

use super::Item;
use super::inline::Atom;
use super::measure::Intrinsic;
use crate::figlet::{Art, Font};
use crate::fonts::Fonts;
use crate::style::{ComputedStyle, TextAlign};

/// The font a block's text is drawn with, if it has a known one.
pub(super) fn font<'f>(style: &ComputedStyle, fonts: &'f Fonts) -> Option<&'f Font> {
    style.font.as_deref().and_then(|name| fonts.get(name))
}

/// Lays out `atoms` in `font` at (`x`, `y`), wrapping at `width`.
/// Returns the rows used.
pub(super) fn layout(
    atoms: &[Atom],
    font: &Font,
    align: TextAlign,
    x: i32,
    y: i32,
    width: i32,
    out: &mut Vec<Item>,
) -> i32 {
    let text = characters(atoms);
    if text.is_empty() {
        return 0;
    }
    let lines = font.render_wrapped(&text, usize::try_from(width.max(1)).unwrap_or(1));
    let height = cells(font.height());
    let mut top = y;
    for art in &lines {
        let line_width = cells(art.width());
        let offset = match align {
            TextAlign::Left => 0,
            TextAlign::Center => (width - line_width) / 2,
            TextAlign::Right => width - line_width,
        }
        .max(0);
        emit(art, x + offset, top, out);
        top += height;
    }
    top - y
}

/// The narrowest (longest word) and widest (no wrapping) the text can be drawn.
pub(super) fn intrinsic(atoms: &[Atom], font: &Font) -> Intrinsic {
    let text = characters(atoms);
    let widest = |separators: &[char]| {
        text.split(|(c, _)| separators.contains(c))
            .map(|piece| cells(font.render(piece).width()))
            .max()
            .unwrap_or(0)
    };
    Intrinsic {
        min: widest(&[' ', '\n']),
        max: widest(&['\n']),
    }
}

fn cells(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// The text as styled characters: runs of whitespace become one space,
/// none at line ends, and `<br>` becomes `'\n'`.
fn characters(atoms: &[Atom]) -> Vec<(char, Style)> {
    let mut out: Vec<(char, Style)> = Vec::new();
    let trim_space = |out: &mut Vec<(char, Style)>| {
        if matches!(out.last(), Some((' ', _))) {
            out.pop();
        }
    };
    for atom in atoms {
        match atom {
            Atom::Word(word, style) => out.extend(word.chars().map(|c| (c, *style))),
            Atom::Space(style, _) => {
                if !matches!(out.last(), None | Some((' ' | '\n', _))) {
                    out.push((' ', *style));
                }
            }
            Atom::Break => {
                trim_space(&mut out);
                out.push(('\n', Style::new()));
            }
        }
    }
    trim_space(&mut out);
    out
}

/// One text item per run of drawn cells with the same style.
fn emit(art: &Art<Style>, x: i32, y: i32, out: &mut Vec<Item>) {
    for (row_y, row) in (y..).zip(&art.rows) {
        let mut column = 0;
        while column < row.len() {
            let Some((_, style)) = row[column] else {
                column += 1;
                continue;
            };
            let start = column;
            let mut text = String::new();
            while let Some(Some((c, cell_style))) = row.get(column) {
                if *cell_style != style {
                    break;
                }
                text.push(*c);
                column += 1;
            }
            out.push(Item::Text {
                x: x + cells(start),
                y: row_y,
                text,
                style,
            });
        }
    }
}

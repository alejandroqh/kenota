//! `<img>`: a picture drawn on the pixel canvas in the element's content box.
//!
//! One column is one pixel wide and one row is two pixels tall, so pixels
//! are square. With only a width (or only a height) the picture keeps its
//! shape; with neither it is drawn one picture pixel per canvas pixel,
//! shrunk to fit its container. A picture that could not be loaded shows
//! its `alt` text in brackets.

use ratatui::style::Style;

use super::measure::Intrinsic;
use super::{Ctx, Item, text_style};
use crate::assets::Assets;
use crate::images::Picture;
use crate::pixels::Cell;
use crate::style::StyledNode;

/// Lays out the picture of `node` at (`x`, `y`), `width` columns wide and
/// `height` rows tall if given. Returns the rows used.
pub(super) fn layout(
    node: &StyledNode,
    x: i32,
    y: i32,
    width: i32,
    height: Option<i32>,
    ctx: &mut Ctx<'_>,
) -> i32 {
    let Some(picture) = picture(node, ctx.assets) else {
        ctx.layer.push(Item::Text {
            x,
            y,
            text: alt_text(node),
            style: text_style(&node.style, None),
        });
        return height.unwrap_or(1);
    };
    let rows = height.unwrap_or_else(|| rows_for_width(picture, width));
    let pixels = picture.render(
        usize::try_from(width).unwrap_or(0),
        usize::try_from(rows * 2).unwrap_or(0),
        node.style.dither,
    );
    let mut items = Vec::new();
    cell_items(&pixels.cells(), x, y, &mut items);
    ctx.layer.extend(items);
    rows
}

/// The picture's border-box width: its natural width (or the width its
/// set height implies), and no minimum since it scales down.
pub(super) fn intrinsic(node: &StyledNode, assets: &Assets) -> Intrinsic {
    let style = &node.style;
    let border = style.border.widths();
    let frame_x = style.padding.left + style.padding.right + border.left + border.right;
    let frame_y = style.padding.top + style.padding.bottom + border.top + border.bottom;
    let Some(picture) = picture(node, assets) else {
        let width = i32::try_from(alt_text(node).chars().count()).unwrap_or(0);
        return Intrinsic::fixed(width + frame_x);
    };
    let width = match style.height {
        Some(height) => width_for_rows(picture, height - frame_y),
        None => i32::try_from(picture.width()).unwrap_or(i32::MAX),
    };
    Intrinsic {
        min: frame_x,
        max: width.saturating_add(frame_x),
    }
}

/// Text items for the cells of a pixel canvas whose top-left cell is
/// (`x`, `y`): one item per run of cells with the same colors. Empty
/// cells are skipped.
pub(super) fn cell_items(cells: &[Vec<Option<Cell>>], x: i32, y: i32, out: &mut Vec<Item>) {
    let style = |cell: &Cell| {
        let style = Style::new().fg(cell.fg.color());
        cell.bg.map_or(style, |bg| style.bg(bg.color()))
    };
    for (row_y, row) in (y..).zip(cells) {
        let mut column = 0;
        while column < row.len() {
            let Some(first) = &row[column] else {
                column += 1;
                continue;
            };
            let run_style = style(first);
            let start = column;
            let mut text = String::new();
            while let Some(Some(cell)) = row.get(column) {
                if style(cell) != run_style {
                    break;
                }
                text.push(cell.symbol);
                column += 1;
            }
            out.push(Item::Text {
                x: x + i32::try_from(start).unwrap_or(i32::MAX),
                y: row_y,
                text,
                style: run_style,
            });
        }
    }
}

fn picture<'a>(node: &StyledNode, assets: &'a Assets) -> Option<&'a Picture> {
    assets.images.get(node.attr("src")?)
}

fn alt_text(node: &StyledNode) -> String {
    let alt = node
        .attr("alt")
        .map(str::trim)
        .filter(|alt| !alt.is_empty());
    format!("[{}]", alt.or_else(|| node.attr("src")).unwrap_or("image"))
}

/// Rows a picture `width` columns wide needs to keep its shape.
#[allow(clippy::cast_possible_truncation)]
fn rows_for_width(picture: &Picture, width: i32) -> i32 {
    let pixels = f64::from(width) * f64::from(picture.height()) / f64::from(picture.width());
    ((pixels / 2.0).ceil() as i32).max(1)
}

/// Columns a picture `rows` rows tall needs to keep its shape.
#[allow(clippy::cast_possible_truncation)]
fn width_for_rows(picture: &Picture, rows: i32) -> i32 {
    (f64::from(rows) * 2.0 * f64::from(picture.width()) / f64::from(picture.height())).round()
        as i32
}

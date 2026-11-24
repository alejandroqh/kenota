//! Tables: rows of cells on a box-drawing grid.
//!
//! The outer frame uses the table's `border` style (single by default,
//! `none` for no frame); lines inside the grid are single. The HTML `rules`
//! attribute picks the inner lines: `groups` (the default: lines between
//! columns, and between `<thead>`, `<tbody>` and `<tfoot>`), `rows`,
//! `cols`, `all` or `none`. A first row made only of `<th>` counts as a
//! header group. Columns size to their content; cells wrap when the table
//! is too narrow.

use ratatui::style::Style;

use super::border::{horizontal, junction, vertical};
use super::measure::{self, Intrinsic};
use super::{Area, Ctx, Item, Layer, Tag, block, distribute, layout_box};
use crate::assets::Assets;
use crate::style::{BorderStyle, StyledNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rules {
    None,
    Groups,
    Rows,
    Cols,
    All,
}

struct Row<'a> {
    node: &'a StyledNode,
    cells: Vec<&'a StyledNode>,
    /// Rows in the same `<thead>`, `<tbody>` or `<tfoot>` share a group.
    group: usize,
}

impl<'a> Row<'a> {
    fn new(node: &'a StyledNode, group: usize) -> Self {
        let cells = node
            .children
            .iter()
            .filter(|cell| matches!(cell.tag(), Some("td" | "th")))
            .collect();
        Self { node, cells, group }
    }
}

struct Grid<'a> {
    caption: Option<&'a StyledNode>,
    rows: Vec<Row<'a>>,
    columns: usize,
    frame: BorderStyle,
    rules: Rules,
}

impl<'a> Grid<'a> {
    fn new(table: &'a StyledNode) -> Self {
        let mut caption = None;
        let (mut head, mut body, mut foot) = (Vec::new(), Vec::new(), Vec::new());
        let mut groups = 0;
        // Consecutive <tr> placed directly in the table form one group.
        let mut loose = None;
        for child in &table.children {
            match child.tag() {
                Some("caption") if caption.is_none() => caption = Some(child),
                Some("tr") => {
                    let group = *loose.get_or_insert_with(|| {
                        groups += 1;
                        groups
                    });
                    body.push(Row::new(child, group));
                }
                Some(section @ ("thead" | "tbody" | "tfoot")) => {
                    loose = None;
                    groups += 1;
                    let rows = child
                        .children
                        .iter()
                        .filter(|row| row.tag() == Some("tr"))
                        .map(|row| Row::new(row, groups));
                    match section {
                        "thead" => head.extend(rows),
                        "tfoot" => foot.extend(rows),
                        _ => body.extend(rows),
                    }
                }
                _ => {}
            }
        }

        let has_head = !head.is_empty();
        let mut rows: Vec<Row> = head.into_iter().chain(body).chain(foot).collect();
        if !has_head
            && rows.len() > 1
            && !rows[0].cells.is_empty()
            && rows[0].cells.iter().all(|cell| cell.tag() == Some("th"))
        {
            rows[0].group = 0;
        }

        let rules = match table.attr("rules").map(str::trim) {
            Some(r) if r.eq_ignore_ascii_case("none") => Rules::None,
            Some(r) if r.eq_ignore_ascii_case("rows") => Rules::Rows,
            Some(r) if r.eq_ignore_ascii_case("cols") => Rules::Cols,
            Some(r) if r.eq_ignore_ascii_case("all") => Rules::All,
            _ => Rules::Groups,
        };
        Self {
            caption,
            columns: rows.iter().map(|row| row.cells.len()).max().unwrap_or(0),
            rows,
            frame: table.style.border.top.style,
            rules,
        }
    }

    fn column_lines(&self) -> bool {
        matches!(self.rules, Rules::Groups | Rules::Cols | Rules::All)
    }

    fn line_between(&self, above: usize, below: usize) -> bool {
        match self.rules {
            Rules::All | Rules::Rows => true,
            Rules::Groups => self.rows[above].group != self.rows[below].group,
            Rules::None | Rules::Cols => false,
        }
    }

    fn frame_width(&self) -> i32 {
        i32::from(self.frame != BorderStyle::None)
    }

    /// Columns taken by vertical lines: the frame and the lines between columns.
    fn overhead(&self) -> i32 {
        let inner = if self.column_lines() {
            i32::try_from(self.columns.saturating_sub(1)).unwrap_or(0)
        } else {
            0
        };
        2 * self.frame_width() + inner
    }

    fn column_intrinsics(&self, assets: &Assets) -> Vec<Intrinsic> {
        let mut columns = vec![Intrinsic::default(); self.columns];
        for row in &self.rows {
            for (column, cell) in columns.iter_mut().zip(&row.cells) {
                *column = column.max(measure::intrinsic(cell, assets));
            }
        }
        columns
    }

    /// Column widths filling `available` cells (between the lines).
    #[allow(clippy::cast_precision_loss)]
    fn column_widths(&self, available: i32, assets: &Assets) -> Vec<i32> {
        let columns = self.column_intrinsics(assets);
        let max: i32 = columns.iter().map(|c| c.max).sum();
        let min: i32 = columns.iter().map(|c| c.min).sum();
        let (base, weights): (Vec<i32>, Vec<f32>) = if max <= available {
            columns.iter().map(|c| (c.max, c.max.max(1) as f32)).unzip()
        } else if min <= available {
            columns
                .iter()
                .map(|c| (c.min, (c.max - c.min) as f32))
                .unzip()
        } else {
            return columns.iter().map(|c| c.min).collect();
        };
        let spare = available - base.iter().sum::<i32>();
        base.iter()
            .zip(distribute(spare, &weights))
            .map(|(width, extra)| width + extra)
            .collect()
    }
}

/// The narrowest and widest `table` can be.
pub(super) fn intrinsic(table: &StyledNode, assets: &Assets) -> Intrinsic {
    let grid = Grid::new(table);
    let columns = grid
        .column_intrinsics(assets)
        .into_iter()
        .fold(Intrinsic::default(), |sum, column| sum + column);
    let mut result = columns + grid.overhead();
    if let Some(caption) = grid.caption {
        // A caption wraps rather than widening the table, down to its longest word.
        let margin = caption.style.margin;
        let caption_min = measure::intrinsic(caption, assets).min + margin.left + margin.right;
        result = result.max(Intrinsic::fixed(caption_min));
    }
    result
}

/// Lays out `table` with its border box at (`x`, `y`), `width` wide.
/// Returns its height.
pub(super) fn layout<'a>(
    table: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    ctx: &mut Ctx<'a>,
) -> i32 {
    let grid = Grid::new(table);
    let mut cursor = y;
    if let Some(caption) = grid.caption {
        let top = cursor + caption.style.margin.top;
        let size = block(caption, x, top, width, None, ctx);
        cursor = top + size.height + size.margin_bottom;
    }
    if grid.columns == 0 {
        return cursor - y;
    }

    let widths = grid.column_widths(width - grid.overhead(), ctx.assets);
    let frame = grid.frame_width();
    // Vertical lines (column and style) and where each column's cells start.
    let mut lines = Vec::new();
    let mut starts = Vec::new();
    let mut column = x;
    if frame > 0 {
        lines.push((column, grid.frame));
        column += 1;
    }
    for (i, width) in widths.iter().enumerate() {
        if i > 0 && grid.column_lines() {
            lines.push((column, BorderStyle::Single));
            column += 1;
        }
        starts.push(column);
        column += width;
    }
    if frame > 0 {
        lines.push((column, grid.frame));
        column += 1;
    }
    let (left, right) = (x, column);

    let color = table
        .style
        .border
        .top
        .color
        .unwrap_or_else(|| table.style.text_color());
    let line_style = Style::new().fg(color.color());
    let rule = |y: i32, style: BorderStyle, edge: Edge, out: &mut Layer| {
        out.push(Item::Text {
            x: left,
            y,
            text: rule_text(left, right, style, edge, &lines),
            style: line_style,
        });
    };

    if frame > 0 {
        rule(cursor, grid.frame, Edge::Top, &mut ctx.layer);
        cursor += 1;
    }
    for (i, row) in grid.rows.iter().enumerate() {
        if i > 0 && grid.line_between(i - 1, i) {
            rule(cursor, BorderStyle::Single, Edge::Middle, &mut ctx.layer);
            cursor += 1;
        }
        let height = row
            .cells
            .iter()
            .zip(&widths)
            .map(|(cell, &width)| measure::height(cell, width, ctx.assets))
            .max()
            .unwrap_or(0)
            .max(1);
        if let Some(background) = row.node.style.background {
            let area = Area {
                x: left + frame,
                y: cursor,
                width: right - left - 2 * frame,
                height,
            };
            // Shows with its row, which may be a build step.
            ctx.layer
                .push_tagged(area.fill(background), Tag::of(&row.node.style));
        }
        for ((cell, &start), &width) in row.cells.iter().zip(&starts).zip(&widths) {
            layout_box(cell, start, cursor, width, Some(height), None, ctx);
        }
        for row_y in cursor..cursor + height {
            for &(line_x, style) in &lines {
                ctx.layer.push(Item::Text {
                    x: line_x,
                    y: row_y,
                    text: vertical(style).into(),
                    style: line_style,
                });
            }
        }
        cursor += height;
    }
    if frame > 0 {
        rule(cursor, grid.frame, Edge::Bottom, &mut ctx.layer);
        cursor += 1;
    }
    cursor - y
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Middle,
    Bottom,
}

/// A horizontal grid line from column `left` to `right`, with junctions
/// where it crosses the vertical `lines`.
fn rule_text(
    left: i32,
    right: i32,
    style: BorderStyle,
    edge: Edge,
    lines: &[(i32, BorderStyle)],
) -> String {
    let none = BorderStyle::None;
    let bottom = edge == Edge::Bottom;
    (left..right)
        .map(|x| match lines.iter().find(|(line_x, _)| *line_x == x) {
            Some(&(_, line)) => junction(
                if edge == Edge::Top { none } else { line },
                if edge == Edge::Bottom { none } else { line },
                if x > left { style } else { none },
                if x < right - 1 { style } else { none },
                bottom,
            ),
            None => horizontal(style, bottom),
        })
        .collect()
}

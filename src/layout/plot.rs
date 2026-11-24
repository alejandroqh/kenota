//! Charts drawn on the pixel canvas; [`crate::chart`] reads their data.
//!
//! A chart is the table's caption, then the plot with the value axis on
//! its left and the category labels under it, then a legend when there is
//! more than one series. A pie puts its legend, with percentages, beside
//! it. Bars, lines and slices are pixels, two per cell row. The look of a
//! series comes from its header cell (of a slice, from its first cell):
//! `background` sets the color, `--fill` draws a `░▒▓` pattern instead;
//! cells without either take colors from a palette.

use std::f64::consts::TAU;
use std::ops::Range;

use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::measure::{self, Intrinsic};
use super::picture::cell_items;
use super::{Ctx, Item, block, distribute, text_style};
use crate::assets::Assets;
use crate::chart::{Category, Chart, Kind};
use crate::palette::Ega;
use crate::pixels::{self, Cell, Pixels};
use crate::style::StyledNode;

/// Plot rows (or pie rows) when the chart has no height.
const DEFAULT_ROWS: i32 = 10;

/// Most steps between labels on the value axis.
const MAX_TICKS: i32 = 6;

/// Columns the value labels and the axis usually take, for sizing.
const AXIS_WIDTH: i32 = 8;

/// Spaces between legend entries, and between a pie and its legend.
const LEGEND_GAP: i32 = 3;

/// Slack for rounding errors before rounding down to a pixel.
const EPSILON: f64 = 1e-9;

/// Series colors for header cells that set none.
const PALETTE: [Ega; 8] = [
    Ega::LightCyan,
    Ega::LightMagenta,
    Ega::Yellow,
    Ega::LightGreen,
    Ega::LightRed,
    Ega::White,
    Ega::Cyan,
    Ega::Magenta,
];

/// Lays out the chart of `node` with its content box at (`x`, `y`),
/// `width` wide and `height` tall if given. Returns the rows used.
pub(super) fn layout<'a>(
    node: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    height: Option<i32>,
    ctx: &mut Ctx<'a>,
) -> i32 {
    let Some(chart) = Chart::from_table(node) else {
        return 0;
    };
    let mut cursor = y;
    if let Some(caption) = chart.caption {
        let top = cursor + caption.style.margin.top;
        let size = block(caption, x, top, width, None, ctx);
        cursor = top + size.height + size.margin_bottom;
    }
    let used = cursor - y;
    if chart.categories.is_empty() || chart.series.is_empty() {
        return used;
    }
    let height = height.map(|h| h - used);
    used + match chart.kind {
        Kind::Pie => pie(&chart, x, cursor, width, height, &mut ctx.layer.items),
        _ => axes(node, &chart, x, cursor, width, height, &mut ctx.layer.items),
    }
}

/// The chart's border-box widths: its natural width fits every bar and
/// legend entry on one line; it can shrink to a column per category.
pub(super) fn intrinsic(node: &StyledNode, assets: &Assets) -> Intrinsic {
    let style = &node.style;
    let border = style.border.widths();
    let frame = style.padding.left + style.padding.right + border.left + border.right;
    let Some(chart) = Chart::from_table(node) else {
        return Intrinsic::fixed(frame);
    };
    let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    let (categories, series) = (count(chart.categories.len()), count(chart.series.len()));
    let mut result = if chart.kind == Kind::Pie {
        let legend = chart
            .categories
            .iter()
            .map(|c| 3 + width_of(&c.label) + 5)
            .max()
            .unwrap_or(0);
        Intrinsic {
            min: 4 + LEGEND_GAP + legend,
            max: 2 * DEFAULT_ROWS + LEGEND_GAP + legend,
        }
    } else {
        let entries: Vec<i32> = legend(node, &chart).iter().map(Entry::width).collect();
        let line = entries.iter().sum::<i32>() + LEGEND_GAP * (count(entries.len()) - 1).max(0);
        let widest = entries.iter().copied().max().unwrap_or(0);
        let plot = match chart.kind {
            Kind::Bar => categories * (series * 3 + 2),
            Kind::Xy => 40,
            _ => categories * 6,
        };
        let min = (AXIS_WIDTH + categories.max(1)).max(widest);
        Intrinsic {
            min,
            max: (AXIS_WIDTH + plot).max(line).max(min),
        }
    };
    if let Some(caption) = chart.caption {
        // Like a table's, the caption wraps rather than widening the chart.
        let margin = caption.style.margin;
        let caption_min = measure::intrinsic(caption, assets).min + margin.left + margin.right;
        result = result.max(Intrinsic::fixed(caption_min));
    }
    result + frame
}

/// The look of a series or a slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    Solid(Ega),
    Pattern {
        symbol: char,
        color: Ega,
        background: Option<Ega>,
    },
}

impl Ink {
    /// The look `cell` gives the `index`th series or slice.
    fn of(cell: Option<&StyledNode>, index: usize) -> Self {
        let default = PALETTE[index % PALETTE.len()];
        let Some(style) = cell.map(|cell| &cell.style) else {
            return Ink::Solid(default);
        };
        match style.pattern {
            Some(pattern) => Ink::Pattern {
                symbol: pattern.symbol,
                color: pattern.color.unwrap_or_else(|| style.text_color()),
                background: style.background,
            },
            None => Ink::Solid(style.background.unwrap_or(default)),
        }
    }

    /// The color of lines and of half cells.
    fn color(self) -> Ega {
        match self {
            Ink::Solid(color) | Ink::Pattern { color, .. } => color,
        }
    }

    fn is_pattern(self) -> bool {
        matches!(self, Ink::Pattern { .. })
    }

    /// Two cells showing the ink, for legends.
    fn swatch(self) -> (String, Style) {
        match self {
            Ink::Solid(color) => ("██".into(), Style::new().fg(color.color())),
            Ink::Pattern {
                symbol,
                color,
                background,
            } => {
                let style = Style::new().fg(color.color());
                (
                    format!("{symbol}{symbol}"),
                    background.map_or(style, |bg| style.bg(bg.color())),
                )
            }
        }
    }
}

type Canvas = Pixels<Ink>;

/// The cell for two pixels: the pattern when both share one, else half blocks.
fn ink_cell(top: Option<Ink>, bottom: Option<Ink>) -> Option<Cell> {
    if let Some(Ink::Pattern {
        symbol,
        color,
        background,
    }) = top
        && top == bottom
    {
        return Some(Cell {
            symbol,
            fg: color,
            bg: background,
        });
    }
    pixels::cell(top.map(Ink::color), bottom.map(Ink::color))
}

fn put(canvas: &mut Canvas, x: i32, y: i32, ink: Ink) {
    if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
        canvas.set(x, y, Some(ink));
    }
}

/// Fills columns `x0..x1` between the pixel edges `y0` and `y1`, in either order.
fn fill(canvas: &mut Canvas, x0: i32, x1: i32, y0: i32, y1: i32, ink: Ink) {
    for y in y0.min(y1)..y0.max(y1) {
        for x in x0..x1 {
            put(canvas, x, y, ink);
        }
    }
}

/// A one-pixel line between two pixels (Bresenham).
fn line(canvas: &mut Canvas, from: (i32, i32), to: (i32, i32), ink: Ink) {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (sx, sy) = ((to.0 - x).signum(), (to.1 - y).signum());
    let mut error = dx + dy;
    loop {
        put(canvas, x, y, ink);
        if (x, y) == to {
            break;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

/// An axis from `lo` to `hi` with a tick every `step`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Scale {
    lo: f64,
    hi: f64,
    step: f64,
}

impl Scale {
    /// Round bounds covering `lo..=hi`, with at most `intervals` steps of
    /// 1, 2 or 5 times a power of ten, or 25 times one.
    fn new(lo: f64, hi: f64, intervals: i32) -> Self {
        let (mut lo, mut hi) = if lo.is_finite() && hi.is_finite() && lo <= hi {
            (lo, hi)
        } else {
            (0.0, 1.0)
        };
        if hi - lo <= f64::EPSILON * hi.abs().max(1.0) {
            if lo > 0.0 {
                lo = 0.0;
            } else if hi < 0.0 {
                hi = 0.0;
            } else {
                hi = lo + 1.0;
            }
        }
        let intervals = f64::from(intervals.max(1));
        let magnitude = 10f64.powf(((hi - lo) / intervals).log10().floor());
        let steps = |step: f64| (hi / step - 1e-9).ceil() - (lo / step + 1e-9).floor();
        let step = [1.0, 2.0, 2.5, 5.0, 10.0, 20.0, 25.0, 50.0]
            .into_iter()
            .map(|m| m * magnitude)
            // 2.5 would add a decimal: 0, 2.5, 5.0.
            .filter(|&step| step >= 10.0 || step != 2.5 * magnitude)
            .find(|&step| steps(step) <= intervals)
            .unwrap_or(100.0 * magnitude);
        Self {
            lo: (lo / step + 1e-9).floor() * step,
            hi: (hi / step - 1e-9).ceil() * step,
            step,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn ticks(self) -> Vec<f64> {
        let count = ((self.hi - self.lo) / self.step).round() as i32;
        (0..=count)
            .map(|i| {
                let tick = self.lo + f64::from(i) * self.step;
                if tick.abs() < self.step * 1e-9 {
                    0.0
                } else {
                    tick
                }
            })
            .collect()
    }

    /// Where `value` falls: 0 at `lo`, 1 at `hi`.
    fn fraction(self, value: f64) -> f64 {
        (value - self.lo) / (self.hi - self.lo)
    }

    /// The labels of the ticks, sharing one format: the decimals the step
    /// needs, `k` or `M` for big round numbers, then `suffix`.
    fn labels(self, suffix: &str) -> Vec<String> {
        let biggest = self.lo.abs().max(self.hi.abs());
        let (divisor, unit) = if biggest >= 1e6 && self.step >= 1e5 {
            (1e6, "M")
        } else if biggest >= 1e4 && self.step >= 1e3 {
            (1e3, "k")
        } else {
            (1.0, "")
        };
        let step = self.step / divisor;
        let decimals = (0..4)
            .find(|&d| {
                let scaled = step * 10f64.powi(d);
                (scaled - scaled.round()).abs() < 1e-6
            })
            .unwrap_or(4);
        let decimals = usize::try_from(decimals).unwrap_or(0);
        self.ticks()
            .into_iter()
            .map(|tick| format!("{:.*}{unit}{suffix}", decimals, tick / divisor))
            .collect()
    }
}

/// Starts an axis at zero unless the values bunch up far from it: their
/// spread is less than a sixth of the largest.
fn with_zero(lo: f64, hi: f64) -> (f64, f64) {
    if lo > 0.0 && hi - lo > hi / 6.0 {
        (0.0, hi)
    } else if hi < 0.0 && hi - lo > -lo / 6.0 {
        (lo, 0.0)
    } else {
        (lo, hi)
    }
}

fn bounds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
}

/// The value axis of a plot `rows` tall, in pixel rows (two per text row).
/// A value on a tick lands in the middle of that tick's text row; `lo` is
/// the middle of the axis row under the plot.
#[derive(Clone, Copy, Debug)]
struct Vertical {
    scale: Scale,
    rows: i32,
    /// Bar ends snap to whole cells so patterns are not cut in half.
    snap: bool,
}

impl Vertical {
    /// The position of `value` in pixel edges: 1 at `hi`, `2·rows + 1` at `lo`.
    fn position(self, value: f64) -> f64 {
        1.0 + (1.0 - self.scale.fraction(value)) * f64::from(2 * self.rows)
    }

    /// The pixel edge where a bar reaching `value` ends. Snapped, a bar
    /// fills the whole cell its end falls in: `down` bars (below zero)
    /// end at the bottom of that cell, others at its top.
    #[allow(clippy::cast_possible_truncation)]
    fn edge(self, value: f64, down: bool) -> i32 {
        let position = self.position(value);
        let edge = if self.snap {
            let cell = (position / 2.0 + EPSILON).floor() as i32;
            2 * cell + if down { 2 } else { 0 }
        } else {
            position.round() as i32
        };
        edge.clamp(0, 2 * self.rows)
    }

    /// The pixel row of a point at `value`.
    #[allow(clippy::cast_possible_truncation)]
    fn pixel(self, value: f64) -> i32 {
        ((self.position(value) + EPSILON).floor() as i32).clamp(0, 2 * self.rows - 1)
    }

    /// The text row of a tick at `value`: 0 is the top row, `rows` the axis.
    #[allow(clippy::cast_possible_truncation)]
    fn row(self, value: f64) -> i32 {
        ((self.position(value) / 2.0 + EPSILON).floor() as i32).clamp(0, self.rows)
    }

    /// The edge bars grow from: zero, or the axis end nearest to it.
    /// Snapped, zero's own cell stays empty.
    fn base(self, down: bool) -> i32 {
        self.edge(0.0f64.clamp(self.scale.lo, self.scale.hi), down)
    }
}

/// A legend entry: a swatch of the ink, then the label.
struct Entry {
    ink: Ink,
    label: String,
    style: Style,
}

impl Entry {
    fn width(&self) -> i32 {
        3 + width_of(&self.label)
    }

    fn draw(&self, x: i32, y: i32, out: &mut Vec<Item>) {
        let (swatch, style) = self.ink.swatch();
        out.push(Item::Text {
            x,
            y,
            text: swatch,
            style,
        });
        out.push(Item::Text {
            x: x + 3,
            y,
            text: self.label.clone(),
            style: self.style,
        });
    }
}

/// The legend under a chart with axes: the series, when there are several.
fn legend(node: &StyledNode, chart: &Chart<'_>) -> Vec<Entry> {
    if chart.series.len() < 2 || chart.kind == Kind::HiLo {
        return Vec::new();
    }
    chart
        .series
        .iter()
        .enumerate()
        .map(|(i, series)| Entry {
            ink: Ink::of(series.header, i),
            label: series.name.clone(),
            style: text_style(&series.header.unwrap_or(node).style, None),
        })
        .collect()
}

/// Splits legend entries into lines that fit `width`.
fn wrap(entries: &[Entry], width: i32) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut used = 0;
    for (i, entry) in entries.iter().enumerate() {
        if i > start && used + LEGEND_GAP + entry.width() > width {
            lines.push(start..i);
            start = i;
        }
        used = if i == start {
            entry.width()
        } else {
            used + LEGEND_GAP + entry.width()
        };
    }
    if start < entries.len() {
        lines.push(start..entries.len());
    }
    lines
}

/// A chart with a value axis: bars, lines, areas, hi-lo and X-Y.
fn axes(
    node: &StyledNode,
    chart: &Chart<'_>,
    x: i32,
    y: i32,
    width: i32,
    height: Option<i32>,
    out: &mut Vec<Item>,
) -> i32 {
    let entries = legend(node, chart);
    let lines = wrap(&entries, width);
    let legend_rows = if lines.is_empty() {
        0
    } else {
        1 + i32::try_from(lines.len()).unwrap_or(0)
    };
    // The plot, the axis row and the labels row, then the legend.
    let rows = height.map_or(DEFAULT_ROWS, |h| h - 2 - legend_rows).max(1);
    let inks: Vec<Ink> = (chart.series.iter().enumerate())
        .map(|(i, series)| Ink::of(series.header, i))
        .collect();
    let filled = matches!(
        chart.kind,
        Kind::Bar | Kind::Stacked | Kind::Stacked100 | Kind::Area
    );
    let vertical = Vertical {
        scale: value_scale(chart, rows),
        rows,
        snap: filled && inks.iter().any(|ink| ink.is_pattern()),
    };

    let style = text_style(&node.style, None);
    let suffix = if chart.kind == Kind::Stacked100 {
        "%"
    } else {
        ""
    };
    let labels = vertical.scale.labels(suffix);
    let label_width = labels.iter().map(|l| width_of(l)).max().unwrap_or(0);
    let axis_x = x + label_width + 1;
    let plot_x = axis_x + 1;
    let plot_width = (x + width - plot_x).max(1);

    // The value axis and its labels.
    let tick_rows: Vec<i32> = (vertical.scale.ticks().into_iter())
        .map(|tick| vertical.row(tick))
        .collect();
    for row in 0..rows {
        let symbol = if tick_rows.contains(&row) {
            "┤"
        } else {
            "│"
        };
        out.push(Item::Text {
            x: axis_x,
            y: y + row,
            text: symbol.into(),
            style,
        });
    }
    out.push(Item::Text {
        x: axis_x,
        y: y + rows,
        text: format!("└{}", "─".repeat(usize::try_from(plot_width).unwrap_or(0))),
        style,
    });
    let mut labelled = Vec::new();
    for (label, &row) in labels.iter().zip(&tick_rows) {
        if !labelled.contains(&row) {
            labelled.push(row);
            out.push(Item::Text {
                x: x + label_width - width_of(label),
                y: y + row,
                text: label.clone(),
                style,
            });
        }
    }

    // The plot.
    let size = |n: i32| usize::try_from(n).unwrap_or(0);
    let mut canvas = Canvas::new(size(plot_width), size(rows * 2));
    let slots = slots(plot_width, chart.categories.len());
    let below = y + rows + 1;
    match chart.kind {
        Kind::Bar => bars(chart, &inks, &slots, vertical, &mut canvas),
        Kind::Stacked => stacked(chart, &inks, &slots, vertical, false, &mut canvas),
        Kind::Stacked100 => stacked(chart, &inks, &slots, vertical, true, &mut canvas),
        Kind::Line => lines_chart(chart, &inks, &slots, vertical, &mut canvas),
        Kind::Area => area(chart, &inks, &slots, vertical, &mut canvas),
        Kind::HiLo => hilo(chart, &inks, &slots, vertical, &mut canvas),
        Kind::Xy => {
            let horizontal = x_scale(chart, plot_width);
            let column = |value: f64| column(horizontal, value, plot_width);
            xy(chart, &inks, column, vertical, &mut canvas);
            x_labels(
                horizontal,
                column,
                plot_x,
                below,
                (x, x + width),
                style,
                out,
            );
        }
        Kind::Pie => {}
    }
    cell_items(&canvas.cells_with(ink_cell), plot_x, y, out);
    if chart.kind != Kind::Xy {
        category_labels(&chart.categories, &slots, plot_x, below, out);
    }

    // The legend.
    for (row, line) in (below + 2..).zip(&lines) {
        let entries = &entries[line.clone()];
        let total = entries.iter().map(Entry::width).sum::<i32>()
            + LEGEND_GAP * (i32::try_from(entries.len()).unwrap_or(1) - 1);
        let mut column = x + ((width - total) / 2).max(0);
        for entry in entries {
            entry.draw(column, row, out);
            column += entry.width() + LEGEND_GAP;
        }
    }
    rows + 2 + legend_rows
}

/// The value axis for a plot `rows` tall.
fn value_scale(chart: &Chart<'_>, rows: i32) -> Scale {
    let values = || {
        (chart.series.iter())
            .flat_map(|series| series.values.iter().flatten())
            .copied()
    };
    let (lo, hi) = bounds(values());
    let (lo, hi) = match chart.kind {
        Kind::Stacked100 => (0.0, 100.0),
        Kind::Stacked | Kind::Area => {
            let totals = (0..chart.categories.len()).map(|i| {
                (chart.series.iter())
                    .map(|series| positive(series.values[i]))
                    .sum::<f64>()
            });
            (0.0, bounds(totals).1.max(0.0))
        }
        Kind::Bar => (lo.min(0.0), hi.max(0.0)),
        // Prices: only the range they move in matters.
        Kind::HiLo => (lo, hi),
        _ => with_zero(lo, hi),
    };
    Scale::new(lo, hi, (rows / 2).clamp(1, MAX_TICKS))
}

/// The horizontal axis of an X-Y chart, from the numbers in its first
/// column, spanning just their range.
fn x_scale(chart: &Chart<'_>, plot_width: i32) -> Scale {
    let (lo, hi) = bounds(chart.categories.iter().filter_map(Category::x));
    Scale::new(lo, hi, (plot_width / 10).clamp(1, 8))
}

/// The plot column of an X value.
#[allow(clippy::cast_possible_truncation)]
fn column(scale: Scale, value: f64, plot_width: i32) -> i32 {
    (scale.fraction(value) * f64::from(plot_width - 1)).round() as i32
}

/// A missing or negative value counts as nothing in stacks and slices.
fn positive(value: Option<f64>) -> f64 {
    value.unwrap_or(0.0).max(0.0)
}

/// Each category's columns: (first column, width).
fn slots(width: i32, count: usize) -> Vec<(i32, i32)> {
    let mut start = 0;
    distribute(width, &vec![1.0; count])
        .into_iter()
        .map(|slot| {
            let first = start;
            start += slot;
            (first, slot)
        })
        .collect()
}

/// The middle column of a slot, where points go.
fn center((start, slot): (i32, i32)) -> i32 {
    start + (slot - 1).max(0) / 2
}

fn bars(chart: &Chart<'_>, inks: &[Ink], slots: &[(i32, i32)], v: Vertical, canvas: &mut Canvas) {
    let count = i32::try_from(chart.series.len()).unwrap_or(1).max(1);
    for (i, &(start, slot)) in slots.iter().enumerate() {
        let bar = ((slot - (slot / 3).max(1)) / count).max(1);
        let left = start + (slot - bar * count) / 2;
        for (s, (series, &ink)) in chart.series.iter().zip(inks).enumerate() {
            let column = left + i32::try_from(s).unwrap_or(0) * bar;
            if let Some(value) = series.values[i] {
                let down = value < 0.0;
                fill(
                    canvas,
                    column,
                    column + bar,
                    v.edge(value, down),
                    v.base(down),
                    ink,
                );
            }
        }
    }
}

fn stacked(
    chart: &Chart<'_>,
    inks: &[Ink],
    slots: &[(i32, i32)],
    v: Vertical,
    percent: bool,
    canvas: &mut Canvas,
) {
    for (i, &(start, slot)) in slots.iter().enumerate() {
        let bar = (slot - (slot / 3).max(1)).max(1);
        let left = start + (slot - bar) / 2;
        let total: f64 = chart.series.iter().map(|s| positive(s.values[i])).sum();
        if percent && total <= 0.0 {
            continue;
        }
        let mut sum = 0.0;
        for (series, &ink) in chart.series.iter().zip(inks) {
            let mut value = positive(series.values[i]);
            if percent {
                value = value / total * 100.0;
            }
            let (bottom, top) = (v.edge(sum, false), v.edge(sum + value, false));
            fill(canvas, left, left + bar, bottom, top, ink);
            sum += value;
        }
    }
}

fn lines_chart(
    chart: &Chart<'_>,
    inks: &[Ink],
    slots: &[(i32, i32)],
    v: Vertical,
    canvas: &mut Canvas,
) {
    for (series, &ink) in chart.series.iter().zip(inks) {
        let points: Vec<Option<(i32, i32)>> = (series.values.iter().zip(slots))
            .map(|(value, &slot)| value.map(|value| (center(slot), v.pixel(value))))
            .collect();
        for (i, point) in points.iter().enumerate() {
            let Some(point) = *point else { continue };
            match points.get(i + 1).copied().flatten() {
                Some(next) => line(canvas, point, next, ink),
                None => put(canvas, point.0, point.1, ink),
            }
        }
    }
}

fn area(chart: &Chart<'_>, inks: &[Ink], slots: &[(i32, i32)], v: Vertical, canvas: &mut Canvas) {
    let centers: Vec<i32> = slots.iter().copied().map(center).collect();
    let last = centers.len().saturating_sub(1);
    let mut below = vec![0.0; centers.len()];
    for (series, &ink) in chart.series.iter().zip(inks) {
        let top: Vec<f64> = (below.iter().zip(&series.values))
            .map(|(below, &value)| below + positive(value))
            .collect();
        for (i, &x0) in centers.iter().enumerate() {
            let next = (i + 1).min(last);
            let x1 = centers[next];
            for x in x0..=x1 {
                let t = if x1 > x0 {
                    f64::from(x - x0) / f64::from(x1 - x0)
                } else {
                    0.0
                };
                let at = |stack: &[f64]| stack[i] + (stack[next] - stack[i]) * t;
                let (bottom, top) = (v.edge(at(&below), false), v.edge(at(&top), false));
                fill(canvas, x, x + 1, bottom, top, ink);
            }
        }
        below = top;
    }
}

/// A line from the lowest to the highest value of each category, in the
/// first series' ink; the third series ticks right (close), the fourth
/// left (open).
fn hilo(chart: &Chart<'_>, inks: &[Ink], slots: &[(i32, i32)], v: Vertical, canvas: &mut Canvas) {
    for (i, &slot) in slots.iter().enumerate() {
        let (low, high) = bounds(chart.series.iter().filter_map(|s| s.values[i]));
        if low > high {
            continue;
        }
        let middle = center(slot);
        fill(
            canvas,
            middle,
            middle + 1,
            v.pixel(high),
            v.pixel(low) + 1,
            inks[0],
        );
        for (index, columns) in [(2, middle + 1..middle + 3), (3, middle - 2..middle)] {
            if let Some(value) = chart.series.get(index).and_then(|s| s.values[i]) {
                let row = v.pixel(value);
                fill(
                    canvas,
                    columns.start,
                    columns.end,
                    row,
                    row + 1,
                    inks[index],
                );
            }
        }
    }
}

/// A one-cell mark per point.
fn xy(
    chart: &Chart<'_>,
    inks: &[Ink],
    column: impl Fn(f64) -> i32,
    v: Vertical,
    canvas: &mut Canvas,
) {
    for (series, &ink) in chart.series.iter().zip(inks) {
        for (category, value) in chart.categories.iter().zip(&series.values) {
            if let (Some(x), Some(value)) = (category.x(), value) {
                let top = (v.pixel(*value) - 1).clamp(0, (2 * v.rows - 2).max(0));
                let column = column(x);
                fill(canvas, column, column + 1, top, top + 2, ink);
            }
        }
    }
}

/// Category labels centered under their slots, cut to fit.
fn category_labels(
    categories: &[Category<'_>],
    slots: &[(i32, i32)],
    plot_x: i32,
    y: i32,
    out: &mut Vec<Item>,
) {
    for (category, &(start, slot)) in categories.iter().zip(slots) {
        let text = truncate(&category.label, if slot > 1 { slot - 1 } else { slot });
        out.push(Item::Text {
            x: plot_x + start + (slot - width_of(&text)) / 2,
            y,
            style: text_style(&category.cell.style, None),
            text,
        });
    }
}

/// Ticks on the axis row and value labels under them for an X-Y chart,
/// skipping labels that would touch the one before. Labels stay within
/// the columns `bounds`.
fn x_labels(
    scale: Scale,
    column: impl Fn(f64) -> i32,
    plot_x: i32,
    y: i32,
    (left, right): (i32, i32),
    style: Style,
    out: &mut Vec<Item>,
) {
    let mut free = left;
    for (tick, label) in scale.ticks().into_iter().zip(scale.labels("")) {
        let x = plot_x + column(tick);
        out.push(Item::Text {
            x,
            y: y - 1,
            text: "┬".into(),
            style,
        });
        let label_width = width_of(&label);
        let start = (x - label_width / 2).clamp(left, (right - label_width).max(left));
        if start >= free {
            free = start + label_width + 1;
            out.push(Item::Text {
                x: start,
                y,
                text: label,
                style,
            });
        }
    }
}

/// A pie of the first series with a legend of slices beside it.
#[allow(clippy::cast_possible_truncation)]
fn pie(
    chart: &Chart<'_>,
    x: i32,
    y: i32,
    width: i32,
    height: Option<i32>,
    out: &mut Vec<Item>,
) -> i32 {
    let values: Vec<f64> = (0..chart.categories.len())
        .map(|i| positive(chart.series[0].values[i]))
        .collect();
    let total: f64 = values.iter().sum();
    let name_width = (chart.categories.iter())
        .map(|c| width_of(&c.label))
        .max()
        .unwrap_or(0);
    let entries: Vec<Entry> = (chart.categories.iter().zip(&values).enumerate())
        .map(|(i, (category, value))| {
            let percent = if total > 0.0 {
                (value / total * 100.0).round()
            } else {
                0.0
            };
            let padding =
                " ".repeat(usize::try_from(name_width - width_of(&category.label)).unwrap_or(0));
            Entry {
                ink: Ink::of(Some(category.cell), i),
                label: format!("{}{padding} {:>4}", category.label, format!("{percent}%")),
                style: text_style(&category.cell.style, None),
            }
        })
        .collect();
    let legend_width = entries.iter().map(Entry::width).max().unwrap_or(0);
    let count = i32::try_from(entries.len()).unwrap_or(0);

    let rows = height.unwrap_or(DEFAULT_ROWS).max(1);
    let diameter = (2 * rows).min(width - legend_width - LEGEND_GAP).max(2);
    let pie_rows = (diameter + 1) / 2;
    let used = height.unwrap_or_else(|| pie_rows.max(count));
    let left = x + ((width - diameter - LEGEND_GAP - legend_width) / 2).max(0);

    let size = |n: i32| usize::try_from(n).unwrap_or(0);
    let mut canvas = Canvas::new(size(diameter), size(pie_rows * 2));
    if total > 0.0 {
        let ends: Vec<f64> = values
            .iter()
            .scan(0.0, |sum, value| {
                *sum += value / total;
                Some(*sum)
            })
            .collect();
        let last = values.iter().rposition(|&v| v > 0.0).unwrap_or(0);
        let radius = f64::from(diameter) / 2.0;
        let center_y = f64::from(pie_rows);
        for py in 0..pie_rows * 2 {
            for px in 0..diameter {
                let dx = f64::from(px) + 0.5 - radius;
                let dy = f64::from(py) + 0.5 - center_y;
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                // Clockwise from twelve o'clock.
                let turn = dx.atan2(-dy).rem_euclid(TAU) / TAU;
                let slice = ends.iter().position(|&end| turn < end).unwrap_or(last);
                let ink = Ink::of(Some(chart.categories[slice].cell), slice);
                put(&mut canvas, px, py, ink);
            }
        }
    }
    let top = y + ((used - pie_rows) / 2).max(0);
    cell_items(&canvas.cells_with(ink_cell), left, top, out);

    let legend_x = left + diameter + LEGEND_GAP;
    let legend_y = y + ((used - count) / 2).max(0);
    for (row, entry) in (legend_y..).zip(&entries) {
        entry.draw(legend_x, row, out);
    }
    used
}

fn width_of(text: &str) -> i32 {
    i32::try_from(text.width()).unwrap_or(i32::MAX)
}

/// The start of `text` that fits in `width` columns.
fn truncate(text: &str, width: i32) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += i32::try_from(c.width().unwrap_or(0)).unwrap_or(0);
            used <= width
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_round_their_bounds() {
        let scale = Scale::new(0.0, 42.0, 5);
        assert_eq!((scale.lo, scale.hi, scale.step), (0.0, 50.0, 10.0));
        assert_eq!(scale.labels(""), ["0", "10", "20", "30", "40", "50"]);
        let scale = Scale::new(0.0, 100.0, 4);
        assert_eq!(scale.labels("%"), ["0%", "25%", "50%", "75%", "100%"]);
        let scale = Scale::new(-3.0, 7.0, 5);
        assert_eq!(scale.labels(""), ["-5", "0", "5", "10"]);
        let scale = Scale::new(0.0, 0.3, 3);
        assert_eq!(scale.labels(""), ["0.0", "0.1", "0.2", "0.3"]);
        let scale = Scale::new(0.0, 250_000.0, 5);
        assert_eq!(
            scale.labels(""),
            ["0k", "50k", "100k", "150k", "200k", "250k"]
        );
    }

    #[test]
    fn flat_and_empty_scales() {
        let scale = Scale::new(5.0, 5.0, 5);
        assert_eq!((scale.lo, scale.hi), (0.0, 5.0));
        let scale = Scale::new(f64::INFINITY, f64::NEG_INFINITY, 5);
        assert_eq!((scale.lo, scale.hi), (0.0, 1.0));
    }

    #[test]
    fn zero_joins_the_axis_unless_far() {
        assert_eq!(with_zero(10.0, 42.0), (0.0, 42.0));
        assert_eq!(with_zero(40.0, 45.0), (40.0, 45.0));
        assert_eq!(with_zero(150.0, 180.0), (150.0, 180.0));
        assert_eq!(with_zero(-42.0, -10.0), (-42.0, 0.0));
        assert_eq!(with_zero(-5.0, 5.0), (-5.0, 5.0));
    }

    #[test]
    fn values_on_ticks_meet_their_rows() {
        let v = Vertical {
            scale: Scale::new(0.0, 50.0, 5),
            rows: 10,
            snap: false,
        };
        assert_eq!(v.row(50.0), 0);
        assert_eq!(v.row(40.0), 2);
        assert_eq!(v.row(0.0), 10, "zero is on the axis row");
        assert_eq!(
            v.edge(50.0, false),
            1,
            "the top bar ends mid-row, like its tick"
        );
        assert_eq!(v.edge(0.0, false), 20);
        assert_eq!(v.pixel(40.0), 5);
        let snapped = Vertical { snap: true, ..v };
        assert_eq!(snapped.edge(50.0, false), 0);
        assert_eq!(snapped.edge(40.0, false), 4);
        let below = Vertical {
            scale: Scale::new(-10.0, 20.0, 3),
            rows: 6,
            snap: true,
        };
        assert_eq!(below.row(0.0), 4);
        assert_eq!(below.base(false), 8, "bars up stop above zero's row");
        assert_eq!(below.base(true), 10, "bars down start below it");
        assert_eq!(below.edge(-4.0, true), 12);
    }

    #[test]
    fn legends_wrap() {
        let entry = |label: &str| Entry {
            ink: Ink::Solid(Ega::Red),
            label: label.into(),
            style: Style::new(),
        };
        let entries = [entry("North"), entry("South"), entry("East")];
        let one_line = wrap(&entries, 40);
        assert_eq!((one_line.len(), one_line[0].clone()), (1, 0..3));
        assert_eq!(wrap(&entries, 19), [0..2, 2..3]);
        assert_eq!(wrap(&entries, 3), [0..1, 1..2, 2..3]);
    }

    #[test]
    fn slots_share_the_width() {
        assert_eq!(slots(10, 3), [(0, 4), (4, 3), (7, 3)]);
        assert_eq!(center((4, 3)), 5);
        assert_eq!(center((0, 4)), 1);
    }

    #[test]
    fn truncates_by_width() {
        assert_eq!(truncate("January", 3), "Jan");
        assert_eq!(truncate("日本語", 3), "日");
        assert_eq!(truncate("Q1", 5), "Q1");
    }
}

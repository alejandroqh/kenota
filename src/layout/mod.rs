//! Layout: turns a styled tree into a display list of positioned paint items.
//!
//! Blocks stack top to bottom inside their container's content box.
//! Consecutive inline children (text, `<b>`, `<br>`, …) form lines that
//! wrap at the container's width. `display: flex` puts children side by
//! side, tables get a box-drawing grid, and `position: absolute` boxes are
//! placed once their containing block has its size. Positioned boxes paint
//! in layers ordered by `z-index`. Coordinates are cells and may run past
//! the canvas; painting clips them.

mod bigtext;
mod border;
mod flex;
mod inline;
mod marker;
mod measure;
mod table;

use std::mem;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::fonts::Fonts;
use crate::palette::Ega;
use crate::style::{ComputedStyle, Display, Length, Position, StyledNode};
use marker::Marker;

/// One thing to paint.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Fills a rectangle with a background color, clearing any text under it.
    Fill {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: Ega,
    },
    /// Repeats `symbol` in `color` over a rectangle, keeping the background.
    Pattern {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        symbol: char,
        color: Ega,
    },
    /// Text starting at (x, y). Cells keep their background unless `style` sets one.
    Text {
        x: i32,
        y: i32,
        text: String,
        style: Style,
    },
}

impl Item {
    fn translate(&mut self, dx: i32, dy: i32) {
        let (Item::Fill { x, y, .. } | Item::Pattern { x, y, .. } | Item::Text { x, y, .. }) = self;
        *x += dx;
        *y += dy;
    }
}

#[derive(Debug, Default)]
pub struct DisplayList {
    pub items: Vec<Item>,
    /// Rows used by the normal flow below the top of the layout area.
    /// Absolutely positioned boxes do not count.
    pub height: i32,
}

/// Lays out `root` (normally the `<html>` element) inside `area`. Big
/// text looks its fonts up in `fonts`.
pub fn layout(root: &StyledNode, area: Rect, fonts: &Fonts) -> DisplayList {
    let area = Area::from(area);
    let mut ctx = Ctx::new(fonts);
    let size = block(
        root,
        area.x,
        area.y + root.style.margin.top,
        area.width,
        None,
        &mut ctx,
    );
    place_pending(area, &mut ctx);
    let mut items = Vec::new();
    ctx.layer.flatten(&mut items);
    DisplayList {
        items,
        height: root.style.margin.top + size.height,
    }
}

/// How the terminal draws text in this style. `background` comes from an
/// enclosing inline element such as `<mark>`.
pub(crate) fn text_style(style: &ComputedStyle, background: Option<Ega>) -> Style {
    let mut out = Style::new().fg(style.text_color().color());
    if let Some(bg) = background {
        out = out.bg(bg.color());
    }
    if style.underline {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    if style.line_through {
        out = out.add_modifier(Modifier::CROSSED_OUT);
    }
    out
}

/// A rectangle in cells. Unlike [`Rect`], it may start off the canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Area {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<Rect> for Area {
    fn from(rect: Rect) -> Self {
        Self {
            x: i32::from(rect.x),
            y: i32::from(rect.y),
            width: i32::from(rect.width),
            height: i32::from(rect.height),
        }
    }
}

impl Area {
    fn right(self) -> i32 {
        self.x + self.width
    }

    fn bottom(self) -> i32 {
        self.y + self.height
    }

    fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// The parts of `self` outside `other`, as up to four rectangles.
    fn minus(self, other: Area) -> Vec<Area> {
        let left = self.x.max(other.x);
        let right = self.right().min(other.right());
        let top = self.y.max(other.y);
        let bottom = self.bottom().min(other.bottom());
        if left >= right || top >= bottom {
            return vec![self];
        }
        let middle = bottom - top;
        [
            Area {
                height: top - self.y,
                ..self
            },
            Area {
                y: bottom,
                height: self.bottom() - bottom,
                ..self
            },
            Area {
                y: top,
                width: left - self.x,
                height: middle,
                ..self
            },
            Area {
                x: right,
                y: top,
                width: self.right() - right,
                height: middle,
            },
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect()
    }

    fn fill(self, color: Ega) -> Item {
        Item::Fill {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            color,
        }
    }
}

/// Items of one stacking layer. Each positioned box gets its own.
#[derive(Debug, Default)]
struct Layer {
    z: i32,
    items: Vec<Item>,
    children: Vec<Layer>,
}

impl Layer {
    fn translate(&mut self, dx: i32, dy: i32) {
        for item in &mut self.items {
            item.translate(dx, dy);
        }
        for child in &mut self.children {
            child.translate(dx, dy);
        }
    }

    /// Paint order: child layers with a negative `z-index`, this layer's
    /// items, then the other child layers. Equal `z-index` keeps document order.
    fn flatten(mut self, out: &mut Vec<Item>) {
        self.children.sort_by_key(|layer| layer.z);
        let split = self.children.partition_point(|layer| layer.z < 0);
        let above = self.children.split_off(split);
        for layer in self.children {
            layer.flatten(out);
        }
        out.extend(self.items);
        for layer in above {
            layer.flatten(out);
        }
    }
}

/// An absolutely positioned box waiting for its containing block, with
/// the position it would have had in the flow.
struct Pending<'a> {
    node: &'a StyledNode,
    x: i32,
    y: i32,
}

struct Ctx<'a> {
    fonts: &'a Fonts,
    /// The layer new items go to.
    layer: Layer,
    /// Absolutely positioned boxes found since the nearest positioned ancestor.
    pending: Vec<Pending<'a>>,
}

impl<'a> Ctx<'a> {
    fn new(fonts: &'a Fonts) -> Self {
        Self {
            fonts,
            layer: Layer::default(),
            pending: Vec::new(),
        }
    }
}

struct BlockSize {
    /// Border-box height.
    height: i32,
    /// Bottom margin, after collapsing with the last child's.
    margin_bottom: i32,
}

/// Lays out an in-flow block whose border box starts at row `y`, inside a
/// container whose content box starts at column `x` and is `width` wide.
fn block<'a>(
    node: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    marker: Option<&Marker>,
    ctx: &mut Ctx<'a>,
) -> BlockSize {
    let (offset, box_width) = horizontal(node, width, ctx.fonts);
    layout_box(node, x + offset, y, box_width, None, marker, ctx)
}

/// Where an in-flow block starts inside a container `width` wide, and how
/// wide its border box is. Tables shrink to fit; `auto` margins center.
fn horizontal(node: &StyledNode, width: i32, fonts: &Fonts) -> (i32, i32) {
    let style = &node.style;
    let margin = style.margin;
    let available = width - margin.left - margin.right;
    let box_width = match style.width {
        Some(w) => w.resolve(width),
        None if style.display == Display::Table => measure::intrinsic(node, fonts).fit(available),
        None => available,
    }
    .max(0);
    let free = (available - box_width).max(0);
    let offset = match (style.auto_margin.left, style.auto_margin.right) {
        (true, true) => margin.left + free / 2,
        (true, false) => margin.left + free,
        _ => margin.left,
    };
    (offset, box_width)
}

/// Lays out `node` with its border box at (`x`, `y`), `width` wide. The
/// height comes from the style, else from `stretch` (set by a flex row or
/// by absolute insets), else from the content.
fn layout_box<'a>(
    node: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    stretch: Option<i32>,
    marker: Option<&Marker>,
    ctx: &mut Ctx<'a>,
) -> BlockSize {
    let style = &node.style;
    // A positioned box paints in its own layer and holds its absolute descendants.
    let outer = (style.position != Position::Static).then(|| {
        let layer = Layer {
            z: style.z_index,
            ..Layer::default()
        };
        (
            mem::replace(&mut ctx.layer, layer),
            mem::take(&mut ctx.pending),
        )
    });
    let first_item = ctx.layer.items.len();
    let fixed_height = style.height.or(stretch).map(|h| h.max(0));

    let is_table = style.display == Display::Table;
    let size = if is_table {
        BlockSize {
            height: table::layout(node, x, y, width, ctx).max(fixed_height.unwrap_or(0)),
            margin_bottom: style.margin.bottom,
        }
    } else {
        content_box(node, x, y, width, fixed_height, marker, ctx)
    };

    // Shadow, background and border go under the content.
    let area = Area {
        x,
        y,
        width,
        height: size.height,
    };
    // Like CSS, the body's background and pattern belong to the whole
    // canvas, which `render` paints first; painting them here again would
    // cover boxes with a negative `z-index`.
    let canvas = matches!(node.tag(), Some("html" | "body"));
    let under = decorations(style, area, !canvas, !is_table);
    ctx.layer.items.splice(first_item..first_item, under);

    if let Some((parent, pending)) = outer {
        let border = style.border.widths();
        let padding_box = Area {
            x: x + border.left,
            y: y + border.top,
            width: width - border.left - border.right,
            height: size.height - border.top - border.bottom,
        };
        place_pending(padding_box, ctx);
        ctx.pending = pending;
        let mut layer = mem::replace(&mut ctx.layer, parent);
        if style.position == Position::Relative {
            let inset = style.inset;
            let dx = match (inset.left, inset.right) {
                (Some(left), _) => left.resolve(width),
                (None, Some(right)) => -right.resolve(width),
                (None, None) => 0,
            };
            let dy = match (inset.top, inset.bottom) {
                (Some(top), _) => top.resolve(size.height),
                (None, Some(bottom)) => -bottom.resolve(size.height),
                (None, None) => 0,
            };
            layer.translate(dx, dy);
        }
        ctx.layer.children.push(layer);
    }
    size
}

/// Border, padding, list marker and content of a box that is not a table.
fn content_box<'a>(
    node: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    fixed_height: Option<i32>,
    marker: Option<&Marker>,
    ctx: &mut Ctx<'a>,
) -> BlockSize {
    let style = &node.style;
    let border = style.border.widths();
    let padding = style.padding;
    let top = border.top + padding.top;
    let bottom = border.bottom + padding.bottom;
    let mut content_x = x + border.left + padding.left;
    let mut content_width =
        (width - border.left - border.right - padding.left - padding.right).max(0);
    let content_y = y + top;

    if let Some(marker) = marker {
        ctx.layer.items.push(Item::Text {
            x: content_x,
            y: content_y,
            text: marker.text.clone(),
            style: marker.style,
        });
        content_x += marker.width;
        content_width = (content_width - marker.width).max(0);
    }

    let (mut content_height, trailing_margin) = match style.display {
        Display::Flex => {
            let inner = fixed_height.map(|h| h - top - bottom);
            let row = flex::layout(node, content_x, content_y, content_width, inner, ctx);
            (row, 0)
        }
        _ if node.tag() == Some("hr") => {
            ctx.layer.items.push(Item::Text {
                x: content_x,
                y: content_y,
                text: "─".repeat(usize::try_from(content_width).unwrap_or(0)),
                style: text_style(style, None),
            });
            (1, 0)
        }
        _ => flow(node, content_x, content_y, content_width, ctx),
    };
    if marker.is_some() {
        content_height = content_height.max(1);
    }

    // With no bottom padding or border, the last child's bottom margin
    // collapses through this block instead of adding to its height.
    let (height, margin_bottom) = match fixed_height {
        Some(height) => (height, style.margin.bottom),
        None if bottom == 0 => (
            top + content_height,
            collapse(style.margin.bottom, trailing_margin),
        ),
        None => (
            top + content_height + trailing_margin + bottom,
            style.margin.bottom,
        ),
    };
    BlockSize {
        height,
        margin_bottom,
    }
}

/// What paints under a box's content: drop shadow, background, fill
/// pattern and border.
fn decorations(
    style: &ComputedStyle,
    area: Area,
    with_background: bool,
    with_border: bool,
) -> Vec<Item> {
    let mut items = Vec::new();
    if area.is_empty() {
        return items;
    }
    if let Some(shadow) = style.shadow {
        let shade = Area {
            x: area.x + shadow.x,
            y: area.y + shadow.y,
            ..area
        };
        // Only the part outside the box shows, even when the box is transparent.
        items.extend(
            shade
                .minus(area)
                .into_iter()
                .map(|part| part.fill(shadow.color)),
        );
    }
    if let Some(color) = style.background.filter(|_| with_background) {
        items.push(area.fill(color));
    }
    if let Some(pattern) = style.pattern.filter(|_| with_background) {
        items.push(Item::Pattern {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
            symbol: pattern.symbol,
            color: pattern.color.unwrap_or_else(|| style.text_color()),
        });
    }
    if with_border {
        border::draw(&style.border, style.text_color(), area, &mut items);
    }
    items
}

/// Lays out the children of a block container. Returns the content
/// height and the last block child's bottom margin (not included).
fn flow<'a>(node: &'a StyledNode, x: i32, y: i32, width: i32, ctx: &mut Ctx<'a>) -> (i32, i32) {
    let markers = marker::for_children(node);
    let mut cursor = y;
    let mut pending_margin = 0;
    let mut run: Vec<&StyledNode> = Vec::new();

    let flush =
        |run: &mut Vec<&'a StyledNode>, cursor: &mut i32, pending: &mut i32, ctx: &mut Ctx<'a>| {
            if run.is_empty() {
                return;
            }
            let top = *cursor + *pending;
            let (lines, absolute) = inline::layout(
                run,
                &node.style,
                ctx.fonts,
                x,
                top,
                width,
                &mut ctx.layer.items,
            );
            ctx.pending
                .extend(absolute.into_iter().map(|node| Pending { node, x, y: top }));
            if lines > 0 {
                *cursor += *pending + lines;
                *pending = 0;
            }
            run.clear();
        };

    for (child, marker) in node.children.iter().zip(&markers) {
        if is_absolute(child) {
            ctx.pending.push(Pending {
                node: child,
                x,
                y: cursor + pending_margin,
            });
        } else if is_block_level(child) {
            flush(&mut run, &mut cursor, &mut pending_margin, ctx);
            let top = cursor + collapse(pending_margin, child.style.margin.top);
            let size = block(child, x, top, width, marker.as_ref(), ctx);
            cursor = top + size.height;
            pending_margin = size.margin_bottom;
        } else {
            run.push(child);
        }
    }
    flush(&mut run, &mut cursor, &mut pending_margin, ctx);

    (cursor - y, pending_margin)
}

/// Places the absolutely positioned boxes collected so far inside `containing`.
fn place_pending(containing: Area, ctx: &mut Ctx<'_>) {
    for pending in mem::take(&mut ctx.pending) {
        absolute(&pending, containing, ctx);
    }
}

/// Lays out an absolutely positioned box inside its containing block.
/// Without `left`/`right` (or `top`/`bottom`) it stays where the flow put
/// it; without a width it shrinks to fit its content.
fn absolute<'a>(pending: &Pending<'a>, containing: Area, ctx: &mut Ctx<'a>) {
    let node = pending.node;
    let style = &node.style;
    let margin = style.margin;
    let horizontal = |length: Option<Length>| length.map(|l| l.resolve(containing.width));
    let vertical = |length: Option<Length>| length.map(|l| l.resolve(containing.height));
    let (left, right) = (horizontal(style.inset.left), horizontal(style.inset.right));
    let (top, bottom) = (vertical(style.inset.top), vertical(style.inset.bottom));

    let available = match (left, right) {
        (Some(left), Some(right)) => containing.width - left - right,
        (Some(left), None) => containing.width - left,
        (None, Some(right)) => containing.width - right,
        (None, None) => containing.right() - pending.x,
    } - margin.left
        - margin.right;
    let width = match style.width {
        Some(w) => w.resolve(containing.width),
        None if left.is_some() && right.is_some() => available,
        None => measure::intrinsic(node, ctx.fonts).fit(available),
    }
    .max(0);
    let x = match (left, right) {
        (Some(left), _) => containing.x + left + margin.left,
        (None, Some(right)) => containing.right() - right - margin.right - width,
        (None, None) => pending.x + margin.left,
    };

    let stretch = match (top, bottom) {
        (Some(top), Some(bottom)) => {
            Some(containing.height - top - bottom - margin.top - margin.bottom)
        }
        _ => None,
    };
    let y = match (top, bottom) {
        (Some(top), _) => containing.y + top + margin.top,
        (None, Some(bottom)) => {
            containing.bottom() - bottom - margin.bottom - measure::height(node, width, ctx.fonts)
        }
        (None, None) => pending.y + margin.top,
    };
    layout_box(node, x, y, width, stretch, None, ctx);
}

fn is_absolute(node: &StyledNode) -> bool {
    node.tag().is_some() && node.style.position == Position::Absolute
}

fn is_block_level(node: &StyledNode) -> bool {
    node.tag().is_some()
        && matches!(
            node.style.display,
            Display::Block | Display::ListItem | Display::Flex | Display::Table
        )
}

/// Splits `amount` cells in proportion to `weights`. Cells left over from
/// rounding go one each to the first items with a weight.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn distribute(amount: i32, weights: &[f32]) -> Vec<i32> {
    let total: f32 = weights.iter().sum();
    if amount <= 0 || total <= 0.0 {
        return vec![0; weights.len()];
    }
    let mut shares: Vec<i32> = weights
        .iter()
        .map(|w| (amount as f32 * w / total).floor() as i32)
        .collect();
    let mut left = amount - shares.iter().sum::<i32>();
    for (share, weight) in shares.iter_mut().zip(weights) {
        if left <= 0 {
            break;
        }
        if *weight > 0.0 {
            *share += 1;
            left -= 1;
        }
    }
    shares
}

/// Adjacent vertical margins combine like CSS: the larger positive one,
/// the more negative one, or the sum when the signs differ.
fn collapse(a: i32, b: i32) -> i32 {
    if a >= 0 && b >= 0 {
        a.max(b)
    } else if a < 0 && b < 0 {
        a.min(b)
    } else {
        a + b
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::html;
    use crate::render::{buffer_text, paint};

    /// Renders `html` on a `width`×`height` canvas without the default body margin.
    fn render(html: &str, width: u16, height: u16) -> Buffer {
        let source = format!("<style>body {{ margin: 0 }}</style>{html}");
        let slide = html::parse(&source, None);
        assert!(slide.warnings.is_empty(), "{:?}", slide.warnings);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        paint(
            &layout(&slide.root, area, &Fonts::bundled()).items,
            &mut buf,
            area,
        );
        buf
    }

    fn text(html: &str, width: u16, height: u16) -> String {
        buffer_text(&render(html, width, height))
    }

    #[test]
    fn collapse_margins() {
        assert_eq!(collapse(1, 2), 2);
        assert_eq!(collapse(-1, -3), -3);
        assert_eq!(collapse(2, -1), 1);
    }

    #[test]
    fn paragraphs_wrap_and_margins_collapse() {
        assert_eq!(
            text(
                "<h2 style='margin-bottom: 2'>Title</h2>
                 <p style='margin-top: 1'>The quick brown fox jumps over the lazy dog.</p>
                 <p>End</p>",
                20,
                9
            ),
            "Title\n\n\nThe quick brown fox\njumps over the lazy\ndog.\n\nEnd\n"
        );
    }

    #[test]
    fn whitespace_collapses_across_inline_tags() {
        assert_eq!(
            text("<p>  one <b> two </b>\n\n<i>three</i>  </p>", 30, 1),
            "one two three"
        );
    }

    #[test]
    fn long_words_are_split() {
        assert_eq!(text("<p>abcdefghij</p>", 4, 3), "abcd\nefgh\nij");
    }

    #[test]
    fn br_and_pre() {
        assert_eq!(
            text(
                "<p>a<br>b<br><br>c</p><pre>  x  y\n\tz\n</pre><p>after</p>",
                12,
                9
            ),
            "a\nb\n\nc\n\n  x  y\n        z\n\nafter"
        );
    }

    #[test]
    fn pre_does_not_wrap() {
        assert_eq!(text("<pre>0123456789</pre>", 6, 1), "012345");
    }

    #[test]
    fn text_align_and_transform() {
        assert_eq!(
            text(
                "<p style='text-align: center'>mid</p>
                 <p style='text-align: right; text-transform: uppercase'>right</p>",
                11,
                3
            ),
            "    mid\n\n      RIGHT"
        );
    }

    #[test]
    fn nbsp_does_not_break_or_collapse() {
        assert_eq!(text("<p>a&nbsp;&nbsp;b c</p>", 4, 2), "a  b\nc");
    }

    #[test]
    fn padding_and_background() {
        let buf = render(
            "<div style='background: blue; padding: 1 2; width: 10'>Hi</div><p>next</p>",
            12,
            5,
        );
        assert_eq!(buffer_text(&buf), "\n  Hi\n\nnext\n");
        let blue = Ega::Blue.color();
        assert_eq!(buf[(0, 0)].bg, blue);
        assert_eq!(buf[(9, 2)].bg, blue);
        assert_ne!(buf[(10, 0)].bg, blue, "width limits the box");
        assert_ne!(buf[(0, 3)].bg, blue, "box ends after padding");
    }

    #[test]
    fn inline_background_and_bold_colors() {
        let buf = render("<p>a <mark>hi there</mark> <b>b</b></p>", 20, 1);
        let cyan = Ega::Cyan.color();
        assert_ne!(buf[(1, 0)].bg, cyan, "space before mark");
        assert_eq!(buf[(2, 0)].bg, cyan);
        assert_eq!(buf[(4, 0)].bg, cyan, "space inside mark");
        assert_eq!(buf[(2, 0)].fg, Ega::Black.color());
        assert_eq!(
            buf[(11, 0)].fg,
            Ega::White.color(),
            "bold lightgray is white"
        );
    }

    #[test]
    fn underline_and_strike_modifiers() {
        let buf = render("<p><u>u</u><s>s</s></p>", 5, 1);
        assert!(buf[(0, 0)].modifier.contains(Modifier::UNDERLINED));
        assert!(buf[(1, 0)].modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn hr_spans_the_width() {
        assert_eq!(text("<p>a</p><hr><p>b</p>", 6, 5), "a\n\n──────\n\nb");
    }

    #[test]
    fn border_styles() {
        let html = |style: &str| format!("<div style='width: 6; {style}'>ab</div>");
        assert_eq!(
            text(&html("border: single"), 8, 3),
            "┌────┐\n│ab  │\n└────┘"
        );
        assert_eq!(
            text(&html("border: double"), 8, 3),
            "╔════╗\n║ab  ║\n╚════╝"
        );
        assert_eq!(text(&html("border: thick"), 8, 3), "█▀▀▀▀█\n█ab  █\n█▄▄▄▄█");
        assert_eq!(
            text(&html("border-style: double single"), 8, 3),
            "╒════╕\n│ab  │\n╘════╛"
        );
        assert_eq!(text(&html("border-bottom: single"), 8, 3), "ab\n──────\n");
    }

    #[test]
    fn border_color_defaults_to_text_color() {
        let buf = render(
            "<div style='color: white; border: double yellow'>a</div>
             <div style='color: cyan; border: single'>b</div>",
            6,
            6,
        );
        assert_eq!(buf[(0, 0)].fg, Ega::Yellow.color());
        assert_eq!(buf[(1, 1)].fg, Ega::White.color());
        assert_eq!(buf[(0, 3)].fg, Ega::Cyan.color());
    }

    #[test]
    fn drop_shadow_shows_outside_the_box() {
        let buf = render(
            "<div style='width: 6; padding: 1 0; background: blue; box-shadow: 2 1'>ab</div>",
            10,
            5,
        );
        let black = Ega::Black.color();
        assert_eq!(buf[(5, 2)].bg, Ega::Blue.color());
        assert_ne!(buf[(6, 0)].bg, black, "shadow starts one row down");
        assert_eq!(buf[(6, 1)].bg, black);
        assert_eq!(buf[(7, 3)].bg, black);
        assert_eq!(buf[(2, 3)].bg, black);
        assert_ne!(buf[(1, 3)].bg, black, "shadow starts two columns right");
        assert_ne!(buf[(8, 1)].bg, black);
    }

    #[test]
    fn fill_pattern_under_text() {
        let buf = render(
            "<div style='--fill: \"░\" cyan; background: blue; width: 4'>a</div>
             <div style='--fill: dark; color: red; width: 2'></div>",
            6,
            2,
        );
        assert_eq!(buffer_text(&buf), "a░░░\n");
        assert_eq!(buf[(1, 0)].fg, Ega::Cyan.color());
        assert_eq!(buf[(1, 0)].bg, Ega::Blue.color());
        assert_eq!(
            buf[(0, 0)].bg,
            Ega::Blue.color(),
            "text keeps the background"
        );
    }

    #[test]
    fn empty_boxes_take_no_rows() {
        assert_eq!(text("<div style='--fill: dark'></div><p>x</p>", 4, 1), "x");
    }

    #[test]
    fn auto_margins_center_and_height_is_fixed() {
        assert_eq!(
            text(
                "<div style='width: 4; margin: 0 auto; height: 3; border: single'>ab</div><p>c</p>",
                10,
                4
            ),
            "   ┌──┐\n   │ab│\n   └──┘\nc"
        );
        assert_eq!(
            text("<div style='width: 2; margin-left: auto'>ab</div>", 6, 1),
            "    ab"
        );
    }

    #[test]
    fn flex_row_grows_and_stretches() {
        assert_eq!(
            text(
                "<div style='display: flex; gap: 1'>
                   <div style='border: single'>one</div>
                   <div style='border: single; flex: 1'>two<br>lines</div>
                 </div>",
                20,
                4
            ),
            "┌───┐ ┌────────────┐\n│one│ │two         │\n│   │ │lines       │\n└───┘ └────────────┘"
        );
    }

    #[test]
    fn flex_justify_and_align_center() {
        assert_eq!(
            text(
                "<div style='display: flex; justify-content: center; align-items: center; height: 3'>
                   <span>a</span><span style='margin-left: 2'>bb<br>cc</span>
                 </div>",
                10,
                3
            ),
            "     bb\n  a  cc\n"
        );
    }

    #[test]
    fn flex_items_shrink_and_wrap() {
        assert_eq!(
            text(
                "<div style='display: flex'><p style='margin: 0'>alpha beta</p><div style='width: 6'>fixed!</div></div>",
                12,
                2
            ),
            "alpha fixed!\nbeta"
        );
    }

    #[test]
    fn absolute_boxes_overlap_the_flow() {
        let html = "<p>flow</p>
                    <div style='position: absolute; left: 4; top: 1; border: single'>abs</div>
                    <p>after</p>";
        assert_eq!(text(html, 12, 4), "flow\n    ┌───┐\nafte│abs│\n    └───┘");
        let slide = html::parse(&format!("<style>body {{ margin: 0 }}</style>{html}"), None);
        assert_eq!(
            layout(&slide.root, Rect::new(0, 0, 12, 4), &Fonts::bundled()).height,
            3,
            "only the flow counts"
        );
    }

    #[test]
    fn absolute_anchored_bottom_right() {
        assert_eq!(
            text(
                "<div style='position: absolute; right: 0; bottom: 0'>xy</div>",
                10,
                4
            ),
            "\n\n\n        xy"
        );
    }

    #[test]
    fn z_index_orders_layers() {
        assert_eq!(
            text(
                "<div style='position: absolute; left: 0; top: 0; z-index: 2'>AAA</div>
                 <div style='position: absolute; left: 1; top: 0'>BBB</div>",
                6,
                1
            ),
            "AAAB"
        );
        let buf = render(
            "<p>text</p>
             <div style='position: absolute; left: 0; top: 0; z-index: -1; background: blue; width: 8'>&nbsp;</div>",
            8,
            1,
        );
        assert_eq!(buffer_text(&buf), "text");
        assert_eq!(buf[(0, 0)].bg, Ega::Blue.color(), "drawn under the flow");
    }

    #[test]
    fn relative_box_moves_with_its_absolute_children() {
        assert_eq!(
            text(
                "<div style='position: relative; left: 1; width: 8; height: 4; border: single'>
                   <span style='position: absolute; right: 0; bottom: 0'>x</span>
                 </div>",
                10,
                4
            ),
            " ┌──────┐\n │      │\n │     x│\n └──────┘"
        );
    }

    #[test]
    fn table_with_header_row() {
        assert_eq!(
            text(
                "<table>
                   <tr><th>Item</th><th>Qty</th></tr>
                   <tr><td>Apples</td><td style='text-align: right'>12</td></tr>
                   <tr><td>Kiwis</td><td style='text-align: right'>3</td></tr>
                 </table>",
                30,
                6
            ),
            "┌────────┬─────┐\n\
             │  Item  │ Qty │\n\
             ├────────┼─────┤\n\
             │ Apples │  12 │\n\
             │ Kiwis  │   3 │\n\
             └────────┴─────┘"
        );
    }

    #[test]
    fn table_double_frame_with_all_rules() {
        assert_eq!(
            text(
                "<table style='border: double' rules='all'>
                   <thead><tr><th>A</th><th>B</th></tr></thead>
                   <tbody><tr><td>1</td><td>2</td></tr><tr><td>3</td><td>4</td></tr></tbody>
                 </table>",
                12,
                7
            ),
            "╔═══╤═══╗\n║ A │ B ║\n╟───┼───╢\n║ 1 │ 2 ║\n╟───┼───╢\n║ 3 │ 4 ║\n╚═══╧═══╝"
        );
    }

    #[test]
    fn table_without_lines() {
        assert_eq!(
            text(
                "<table style='border: none' rules='none'><tr><td>a</td><td>b</td></tr></table>",
                10,
                1
            ),
            " a  b"
        );
    }

    #[test]
    fn narrow_table_wraps_cells() {
        assert_eq!(
            text("<table><tr><td>one two</td><td>x</td></tr></table>", 12, 4),
            "┌──────┬───┐\n│ one  │ x │\n│ two  │   │\n└──────┴───┘"
        );
    }

    #[test]
    fn table_caption_and_row_background() {
        let buf = render(
            "<table><caption>Fruit</caption><tr style='background: blue'><td>a</td><td>b</td></tr></table>",
            9,
            4,
        );
        assert_eq!(
            buffer_text(&buf),
            "  Fruit\n┌───┬───┐\n│ a │ b │\n└───┴───┘"
        );
        assert_eq!(buf[(1, 2)].bg, Ega::Blue.color());
        assert_eq!(
            buf[(4, 2)].bg,
            Ega::Blue.color(),
            "under the column line too"
        );
        assert_ne!(buf[(0, 2)].bg, Ega::Blue.color(), "not under the frame");
    }

    #[test]
    fn h1_draws_big_letters() {
        assert_eq!(
            text("<h1>Hi</h1><p>after</p>", 12, 6),
            "█   █ ▀\n█▄▄▄█ █\n█   █ █\n▀   ▀ ▀\n\nafter"
        );
        assert_eq!(text("<h1 style='--font: none'>Hi</h1>", 12, 1), "Hi");
    }

    #[test]
    fn data_font_picks_a_font() {
        assert_eq!(
            text("<p data-font='small'>Hi</p>", 12, 3),
            "█ █ ▀█▀\n█▀█  █\n▀ ▀ ▀▀▀"
        );
        assert_eq!(
            text("<p data-font='small' style='--font: none'>Hi</p>", 12, 1),
            "Hi",
            "the style attribute beats data-font"
        );
    }

    #[test]
    fn big_text_wraps_and_centers() {
        assert_eq!(
            text("<h1 style='text-align: center'>I I</h1>", 9, 8),
            "   ▀█▀\n    █\n    █\n   ▀▀▀\n   ▀█▀\n    █\n    █\n   ▀▀▀"
        );
    }

    #[test]
    fn big_text_takes_inline_colors_and_leaves_blanks_alone() {
        let buf = render("<h1 style='--fill: \"░\"'>I<em>I</em></h1>", 10, 4);
        assert_eq!(buf[(0, 1)].symbol(), "░", "blank cells keep the fill");
        assert_eq!(buf[(1, 1)].symbol(), "█");
        assert_eq!(buf[(1, 1)].fg, Ega::White.color());
        assert_eq!(buf[(5, 1)].fg, Ega::Yellow.color());
    }

    #[test]
    fn big_text_in_a_flex_row_measures_its_width() {
        assert_eq!(
            text(
                "<div style='display: flex; gap: 1'><h1 style='margin: 0'>I</h1><p>x</p></div>",
                12,
                1
            ),
            "▀█▀ x"
        );
    }

    #[test]
    fn display_list_reports_overflow_height() {
        let slide = html::parse("<style>body{margin:0}</style><p>a</p><p>b</p>", None);
        let list = layout(&slide.root, Rect::new(0, 0, 10, 2), &Fonts::bundled());
        assert_eq!(
            list.height, 3,
            "two paragraphs and the margin between, past the 2-row area"
        );
    }
}

//! Layout: turns a styled tree into a display list of positioned paint items.
//!
//! Blocks stack top to bottom inside their container's content box.
//! Consecutive inline children (text, `<b>`, `<br>`, …) form lines that
//! wrap at the container's width. Coordinates are cells and may run past
//! the canvas; painting clips them.

mod inline;
mod marker;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::palette::Ega;
use crate::style::{ComputedStyle, Display, StyledNode};
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
    /// Text starting at (x, y). Cells keep their background unless `style` sets one.
    Text {
        x: i32,
        y: i32,
        text: String,
        style: Style,
    },
}

#[derive(Debug, Default)]
pub struct DisplayList {
    pub items: Vec<Item>,
    /// Rows used below the top of the layout area.
    pub height: i32,
}

/// Lays out `root` (normally the `<html>` element) inside `area`.
pub fn layout(root: &StyledNode, area: Rect) -> DisplayList {
    let mut items = Vec::new();
    let size = block(
        root,
        i32::from(area.x),
        i32::from(area.y) + root.style.margin.top,
        i32::from(area.width),
        None,
        &mut items,
    );
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

struct BlockSize {
    /// Border-box height.
    height: i32,
    /// Bottom margin, after collapsing with the last child's.
    margin_bottom: i32,
}

/// Lays out a block whose border box starts at row `y`, inside a
/// container whose content box starts at column `x` and is `width` wide.
fn block(
    node: &StyledNode,
    x: i32,
    y: i32,
    width: i32,
    marker: Option<&Marker>,
    out: &mut Vec<Item>,
) -> BlockSize {
    let style = &node.style;
    let x = x + style.margin.left;
    let width = style
        .width
        .map_or(width - style.margin.left - style.margin.right, |w| {
            w.resolve(width)
        })
        .max(0);

    // The background goes under the content, so reserve its place now and
    // fill it in once the height is known.
    let background = style.background.map(|color| {
        out.push(Item::Fill {
            x,
            y,
            width,
            height: 0,
            color,
        });
        (out.len() - 1, color)
    });

    let mut content_x = x + style.padding.left;
    let mut content_width = (width - style.padding.left - style.padding.right).max(0);
    let content_y = y + style.padding.top;

    if let Some(marker) = marker {
        out.push(Item::Text {
            x: content_x,
            y: content_y,
            text: marker.text.clone(),
            style: marker.style,
        });
        content_x += marker.width;
        content_width = (content_width - marker.width).max(0);
    }

    let (mut content_height, trailing_margin) = if node.tag() == Some("hr") {
        out.push(Item::Text {
            x: content_x,
            y: content_y,
            text: "─".repeat(usize::try_from(content_width).unwrap_or(0)),
            style: text_style(style, None),
        });
        (1, 0)
    } else {
        flow(node, content_x, content_y, content_width, out)
    };
    if marker.is_some() {
        content_height = content_height.max(1);
    }

    // With no bottom padding, the last child's bottom margin collapses
    // through this block instead of adding to its height.
    let (inner_height, margin_bottom) = if style.padding.bottom == 0 {
        (
            content_height,
            collapse(style.margin.bottom, trailing_margin),
        )
    } else {
        (content_height + trailing_margin, style.margin.bottom)
    };
    let height = style.padding.top + inner_height + style.padding.bottom;

    if let Some((index, color)) = background {
        out[index] = Item::Fill {
            x,
            y,
            width,
            height,
            color,
        };
    }
    BlockSize {
        height,
        margin_bottom,
    }
}

/// Lays out the children of a block container. Returns the content
/// height and the last block child's bottom margin (not included).
fn flow(node: &StyledNode, x: i32, y: i32, width: i32, out: &mut Vec<Item>) -> (i32, i32) {
    let markers = marker::for_children(node);
    let mut cursor = y;
    let mut pending_margin = 0;
    let mut run: Vec<&StyledNode> = Vec::new();

    let flush =
        |run: &mut Vec<&StyledNode>, cursor: &mut i32, pending: &mut i32, out: &mut Vec<Item>| {
            if run.is_empty() {
                return;
            }
            let lines = inline::layout(run, &node.style, x, *cursor + *pending, width, out);
            if lines > 0 {
                *cursor += *pending + lines;
                *pending = 0;
            }
            run.clear();
        };

    for (child, marker) in node.children.iter().zip(&markers) {
        if is_block_level(child) {
            flush(&mut run, &mut cursor, &mut pending_margin, out);
            let top = cursor + collapse(pending_margin, child.style.margin.top);
            let size = block(child, x, top, width, marker.as_ref(), out);
            cursor = top + size.height;
            pending_margin = size.margin_bottom;
        } else {
            run.push(child);
        }
    }
    flush(&mut run, &mut cursor, &mut pending_margin, out);

    (cursor - y, pending_margin)
}

fn is_block_level(node: &StyledNode) -> bool {
    node.tag().is_some() && matches!(node.style.display, Display::Block | Display::ListItem)
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
        paint(&layout(&slide.root, area).items, &mut buf, area);
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
        assert_eq!(buf[(1, 0)].bg, Ega::Black.color(), "space before mark");
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
    fn display_list_reports_overflow_height() {
        let slide = html::parse("<style>body{margin:0}</style><p>a</p><p>b</p>", None);
        let list = layout(&slide.root, Rect::new(0, 0, 10, 2));
        assert_eq!(
            list.height, 3,
            "two paragraphs and the margin between, past the 2-row area"
        );
    }
}

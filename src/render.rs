//! Rendering a slide onto its canvas.
//!
//! The canvas is a plain `ratatui` [`Buffer`] the size of the deck's screen
//! (80×25 by default), starting at (0, 0). It does not depend on the
//! terminal, so slides can be tested by comparing canvas text.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthChar;

use crate::chrome::{self, FooterInfo};
use crate::deck::Deck;
use crate::layout::{self, Item};
use crate::palette::Ega;

/// Rows taken by the footer bar when it is enabled.
pub const FOOTER_ROWS: u16 = 2;

/// Draws slide `index` of `deck`, including the footer bar.
///
/// # Panics
/// If `index` is out of range.
pub fn slide_canvas(deck: &Deck, index: usize) -> Buffer {
    let size = deck.config.size;
    let area = Rect::new(0, 0, size.width, size.height);
    let slide = &deck.slides[index];

    let mut buf = Buffer::empty(area);
    buf.set_style(
        area,
        Style::new()
            .fg(Ega::LightGray.color())
            .bg(slide.background().color()),
    );

    let footer_rows = if deck.config.footer { FOOTER_ROWS } else { 0 };
    let content = Rect {
        height: area.height - footer_rows,
        ..area
    };
    let mut items = Vec::new();
    if let Some((symbol, color)) = slide.pattern() {
        // Like the background, the body's fill pattern covers the whole slide.
        items.push(Item::Pattern {
            x: 0,
            y: 0,
            width: i32::from(content.width),
            height: i32::from(content.height),
            symbol,
            color,
        });
    }
    items.extend(layout::layout(&slide.root, content).items);
    paint(&items, &mut buf, content);

    if footer_rows > 0 {
        chrome::footer(
            &mut buf,
            Rect::new(area.x, content.bottom(), area.width, footer_rows),
            &FooterInfo {
                slide_title: &slide.title,
                deck_title: deck.config.title.as_deref(),
                index,
                count: deck.slides.len(),
            },
        );
    }

    buf
}

/// Paints display-list items into `buf`, drawing nothing outside `clip`.
pub fn paint(items: &[Item], buf: &mut Buffer, clip: Rect) {
    let clip = clip.intersection(buf.area);
    let (left, top) = (i32::from(clip.left()), i32::from(clip.top()));
    let (right, bottom) = (i32::from(clip.right()), i32::from(clip.bottom()));
    // Callers only pass coordinates inside `clip`, which is inside the buffer.
    let at = |col: i32, row: i32| {
        (
            u16::try_from(col).unwrap_or_default(),
            u16::try_from(row).unwrap_or_default(),
        )
    };

    for item in items {
        match item {
            Item::Fill {
                x,
                y,
                width,
                height,
                color,
            } => {
                for row in (*y).max(top)..(y + height).min(bottom) {
                    for col in (*x).max(left)..(x + width).min(right) {
                        buf[at(col, row)].set_symbol(" ").set_bg(color.color());
                    }
                }
            }
            Item::Pattern {
                x,
                y,
                width,
                height,
                symbol,
                color,
            } => {
                let symbol = symbol.encode_utf8(&mut [0; 4]).to_string();
                for row in (*y).max(top)..(y + height).min(bottom) {
                    for col in (*x).max(left)..(x + width).min(right) {
                        buf[at(col, row)].set_symbol(&symbol).set_fg(color.color());
                    }
                }
            }
            Item::Text { x, y, text, style } => {
                if *y < top || *y >= bottom {
                    continue;
                }
                let mut col = *x;
                for c in text.chars() {
                    let width = c.width().unwrap_or(0);
                    if width == 0 {
                        continue;
                    }
                    let cells = i32::try_from(width).unwrap_or(1);
                    if col >= left && col + cells <= right {
                        let (cx, cy) = at(col, *y);
                        buf.set_stringn(cx, cy, c.encode_utf8(&mut [0; 4]), width, *style);
                    }
                    col += cells;
                }
            }
        }
    }
}

/// The buffer's symbols, one line per row, trailing spaces removed.
#[cfg(test)]
pub(crate) fn buffer_text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            let row: String = (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;
    use crate::deck::{DeckConfig, test_deck, test_slide};

    fn single_slide_canvas(body: &str) -> Buffer {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("Test", body));
        assert!(
            deck.slides[0].warnings.is_empty(),
            "{:?}",
            deck.slides[0].warnings
        );
        slide_canvas(&deck, 0)
    }

    #[test]
    fn middle_slide_with_footer() {
        let mut deck = test_deck(&["Welcome", "Spreadsheet Charting", "The End"]);
        deck.config.title = Some("Works Tutorial".into());
        deck.slides[1] = test_slide(
            "Spreadsheet Charting",
            "<p>Once you've created a chart,<br>you can quickly and easily change its format.</p>
             <p>This paragraph is long enough that it has to wrap onto a second line
             inside the 76-column text area.</p>",
        );
        insta::assert_snapshot!(buffer_text(&slide_canvas(&deck, 1)));
    }

    #[test]
    fn typography_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<style>body { background: blue } .box { background: lightgray; color: black; padding: 1 2; width: 40 }</style>
             <h1>Text styles</h1>
             <h2>Headings, <b>bold</b>, <em>emphasis</em> and <mark>highlights</mark></h2>
             <p>Press <kbd>Ctrl</kbd>+<kbd>C</kbd> to copy. Type <code>dir /w</code> at the prompt.
                <u>Underline</u> and <s>strike</s> use terminal attributes.</p>
             <p style='text-align: center'>Centered text</p>
             <p style='text-align: right'>Right-aligned text</p>
             <hr>
             <div class='box'>A box with a background and padding.</div>"
        )));
    }

    #[test]
    fn lists_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<h2>Lists</h2>
             <ul>
               <li>Bullets hang, so long items wrap neatly under their first word instead of the bullet.</li>
               <li>Nested lists change symbol
                 <ul><li>square<ul><li>circle</li></ul></li></ul>
               </li>
             </ul>
             <ol start='9'><li>Numbers</li><li>stay aligned</li></ol>
             <ol style='list-style-type: upper-roman'><li>Roman</li><li>numerals</li></ol>
             <pre>
  ┌──────────┐
  │ pre text │
  └──────────┘</pre>"
        )));
    }

    #[test]
    fn boxes_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<style>
               .row { display: flex; gap: 3; margin-bottom: 2 }
               .row div { flex: 1; padding: 0 1; box-shadow: 2 1 }
               .desk { --fill: light; padding: 1 0 }
               .dialog { width: 30; margin: 0 auto; border: double; background: lightgray; text-align: center }
               .tag { position: absolute; right: 2; top: 1; border: thick; z-index: 1 }
             </style>
             <h2>Boxes</h2>
             <div class='row'>
               <div style='border: single'>single</div>
               <div style='border: double'>double, and wrapping text</div>
               <div style='border: thick'>thick</div>
             </div>
             <div class='desk'><div class='dialog'>centered on a fill</div></div>
             <table rules='all'>
               <thead><tr><th>Key</th><th>Action</th></tr></thead>
               <tr><td>PgDn</td><td>next slide</td></tr>
               <tr><td>q</td><td>quit</td></tr>
             </table>
             <div class='tag'>NEW</div>"
        )));
    }

    #[test]
    fn body_background_fills_the_canvas() {
        let canvas = single_slide_canvas("<style>body { background: blue }</style><p>x</p>");
        assert_eq!(canvas[(0, 0)].bg, Ega::Blue.color());
        assert_eq!(canvas[(79, 22)].bg, Ega::Blue.color());
        assert_eq!(canvas[(0, 24)].bg, Ega::Black.color(), "footer stays black");
    }

    #[test]
    fn body_pattern_fills_the_canvas() {
        let canvas = single_slide_canvas(
            "<style>body { background: blue; --fill: light cyan }</style><p>x</p>",
        );
        assert_eq!(canvas[(0, 0)].symbol(), "░");
        assert_eq!(canvas[(0, 0)].fg, Ega::Cyan.color());
        assert_eq!(canvas[(79, 22)].symbol(), "░");
        assert_eq!(canvas[(2, 1)].symbol(), "x");
        assert_ne!(canvas[(0, 24)].symbol(), "░", "footer has no pattern");
    }

    #[test]
    fn content_is_clipped_above_the_footer() {
        let canvas = single_slide_canvas(&"<div>line</div>".repeat(30));
        let text = buffer_text(&canvas);
        let rows: Vec<&str> = text.lines().collect();
        assert_eq!(rows[22], "  line");
        assert!(
            rows[23].contains("Test"),
            "footer title row intact: {}",
            rows[23]
        );
    }

    #[test]
    fn arrows_dim_at_the_ends() {
        let deck = test_deck(&["One", "Two"]);
        let dim = Ega::DarkGray.color();
        let lit = Ega::White.color();

        let first = slide_canvas(&deck, 0);
        let row = buffer_text(&first).lines().last().unwrap().to_string();
        let pgup = u16::try_from(row.chars().position(|c| c == '◄').unwrap()).unwrap();
        let pgdn = u16::try_from(row.chars().position(|c| c == '►').unwrap()).unwrap();
        assert_eq!(first[(pgup, 24)].fg, dim);
        assert_eq!(first[(pgdn, 24)].fg, lit);

        let last = slide_canvas(&deck, 1);
        assert_eq!(last[(pgup, 24)].fg, lit);
        assert_eq!(last[(pgdn, 24)].fg, dim);
    }

    #[test]
    fn no_footer_leaves_bottom_rows_for_content() {
        let mut deck = test_deck(&[]);
        deck.slides
            .push(test_slide("Only", &"<div>line</div>".repeat(30)));
        deck.config = DeckConfig {
            footer: false,
            ..DeckConfig::default()
        };
        let text = buffer_text(&slide_canvas(&deck, 0));
        assert!(!text.contains("PgDn"), "{text}");
        assert_eq!(text.lines().last(), Some("  line"));
    }

    #[test]
    fn paint_clips_and_keeps_background() {
        let area = Rect::new(0, 0, 6, 2);
        let mut buf = Buffer::empty(area);
        let items = [
            Item::Fill {
                x: -2,
                y: 0,
                width: 4,
                height: 5,
                color: Ega::Blue,
            },
            Item::Text {
                x: -1,
                y: 0,
                text: "abcdefgh".into(),
                style: Style::new().fg(Color::Red),
            },
            Item::Text {
                x: 0,
                y: 2,
                text: "below".into(),
                style: Style::new(),
            },
        ];
        paint(&items, &mut buf, Rect::new(0, 0, 5, 2));
        assert_eq!(buffer_text(&buf), "bcdef\n");
        assert_eq!(buf[(0, 0)].bg, Ega::Blue.color(), "text keeps the fill");
        assert_eq!(buf[(1, 1)].bg, Ega::Blue.color());
        assert_eq!(buf[(2, 0)].bg, Color::Reset);
    }
}

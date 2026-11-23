//! Rendering a slide onto its canvas.
//!
//! The canvas is a plain `ratatui` [`Buffer`] the size of the deck's screen
//! (80×25 by default), starting at (0, 0). It does not depend on the
//! terminal, so slides can be tested by comparing canvas text.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::chrome::{self, FooterInfo};
use crate::deck::Deck;
use crate::palette::Ega;

/// Rows taken by the footer bar when it is enabled.
pub const FOOTER_ROWS: u16 = 2;

const MARGIN_LEFT: u16 = 2;
const MARGIN_RIGHT: u16 = 2;
const MARGIN_TOP: u16 = 1;

/// Draws slide `index` of `deck`, including the footer bar.
///
/// # Panics
/// If `index` is out of range.
pub fn slide_canvas(deck: &Deck, index: usize) -> Buffer {
    let size = deck.config.size;
    let area = Rect::new(0, 0, size.width, size.height);
    let mut buf = Buffer::empty(area);
    buf.set_style(
        area,
        Style::new()
            .fg(Ega::LightGray.color())
            .bg(Ega::Black.color()),
    );

    let footer_rows = if deck.config.footer { FOOTER_ROWS } else { 0 };
    let content = Rect {
        height: area.height - footer_rows,
        ..area
    };
    let slide = &deck.slides[index];

    let text_area = Rect::new(
        content.x + MARGIN_LEFT,
        content.y + MARGIN_TOP,
        content.width - MARGIN_LEFT - MARGIN_RIGHT,
        content.height.saturating_sub(MARGIN_TOP),
    );
    Paragraph::new(slide.text.as_str())
        .wrap(Wrap { trim: false })
        .render(text_area, &mut buf);

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
    use super::*;
    use crate::deck::{DeckConfig, test_deck};

    #[test]
    fn middle_slide_with_footer() {
        let mut deck = test_deck(&["Welcome", "Spreadsheet Charting", "The End"]);
        deck.config.title = Some("Works Tutorial".into());
        deck.slides[1].text = "Once you've created a chart,\nyou can quickly and easily change \
                               its format.\n\nThis paragraph is long enough that it has to \
                               wrap onto a second line inside the 76-column text area."
            .into();
        insta::assert_snapshot!(buffer_text(&slide_canvas(&deck, 1)));
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
        let mut deck = test_deck(&["Only"]);
        deck.config = DeckConfig {
            footer: false,
            ..DeckConfig::default()
        };
        deck.slides[0].text = "line\n".repeat(30);
        let text = buffer_text(&slide_canvas(&deck, 0));
        assert!(!text.contains("PgDn"), "{text}");
        assert_eq!(text.lines().last(), Some("  line"));
    }
}

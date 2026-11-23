//! The demo deck must load cleanly and fit on screen.

use std::path::Path;

use dos_kenota::deck::Deck;
use dos_kenota::layout;
use dos_kenota::render::FOOTER_ROWS;
use ratatui::layout::Rect;

#[test]
fn demo_deck_has_no_warnings_and_fits() {
    let deck = Deck::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("decks/demo")).unwrap();
    let size = deck.config.size;
    let content = Rect::new(0, 0, size.width, size.height - FOOTER_ROWS);

    for slide in &deck.slides {
        let name = slide.path.display();
        assert!(slide.warnings.is_empty(), "{name}: {:?}", slide.warnings);
        let height = layout::layout(&slide.root, content).height;
        assert!(
            height <= i32::from(content.height),
            "{name} is {height} rows tall; only {} fit",
            content.height
        );
    }
}

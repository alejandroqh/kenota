//! The bundled decks must load cleanly and fit on screen: the demo deck and
//! the starter deck that ships with the Claude Code skill.

use std::path::Path;

use dos_kenota::deck::Deck;
use dos_kenota::layout;
use dos_kenota::render::FOOTER_ROWS;
use ratatui::layout::Rect;

fn assert_clean_and_fits(folder: &str) {
    let deck = Deck::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join(folder)).unwrap();
    let size = deck.config.size;
    let content = Rect::new(0, 0, size.width, size.height - FOOTER_ROWS);

    for slide in &deck.slides {
        let name = slide.path.display();
        assert!(slide.warnings.is_empty(), "{name}: {:?}", slide.warnings);
        let height = layout::layout(&slide.root, content, &deck.assets).height;
        assert!(
            height <= i32::from(content.height),
            "{name} is {height} rows tall; only {} fit",
            content.height
        );
    }
}

#[test]
fn demo_deck_has_no_warnings_and_fits() {
    assert_clean_and_fits("decks/demo");
}

#[test]
fn skill_starter_deck_has_no_warnings_and_fits() {
    assert_clean_and_fits("skills/dos-kenota/assets/starter-deck");
}

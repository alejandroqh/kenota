//! The bundled decks must pass `dos-kenota check`: the demo deck and the
//! starter deck that ships with the Claude Code skill.

use std::path::Path;

use dos_kenota::check::check;
use dos_kenota::deck::Deck;

fn assert_no_problems(folder: &str) {
    let deck = Deck::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join(folder)).unwrap();
    let problems: Vec<String> = check(&deck).iter().map(ToString::to_string).collect();
    assert!(problems.is_empty(), "{folder}:\n{}", problems.join("\n"));
}

#[test]
fn demo_deck_has_no_problems() {
    assert_no_problems("decks/demo");
}

#[test]
fn skill_starter_deck_has_no_problems() {
    assert_no_problems("skills/dos-kenota/assets/starter-deck");
}

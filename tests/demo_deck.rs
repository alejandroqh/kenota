//! The bundled decks must pass `kenota check`: the demo deck, the deck the
//! README pictures come from, the README tutorial and the Claude Code
//! skill's starter deck.

use std::path::Path;

use kenota::check::check;
use kenota::deck::Deck;

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
fn readme_deck_has_no_problems() {
    assert_no_problems("decks/readme");
}

#[test]
fn skill_starter_deck_has_no_problems() {
    assert_no_problems("skills/kenota/assets/starter-deck");
}

#[test]
fn tutorial_deck_has_no_problems_and_matches_the_readme() {
    assert_no_problems("decks/hello");
    let slide = include_str!("../decks/hello/01-hello.html");
    assert!(
        include_str!("../README.md").contains(slide),
        "the README tutorial must show decks/hello/01-hello.html as it is"
    );
}

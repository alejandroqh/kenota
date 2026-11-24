//! The `render` and `check` commands, run as a user would.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kenota"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn render_prints_one_slide_as_plain_text() {
    let output = run(&["render", "decks/demo", "--slide", "2", "--colors", "none"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 25, "{text}");
    assert!(text.contains("How a deck works"), "{text}");
    assert!(text.contains("2/12"), "{text}");
    assert!(!text.contains('\x1b'));
}

#[test]
fn render_picks_steps_and_moments() {
    let slide = |extra: &[&str]| {
        let mut args = vec!["render", "decks/demo", "-s", "10", "--colors", "none"];
        args.extend_from_slice(extra);
        stdout(&run(&args))
    };
    let complete = slide(&[]);
    assert!(complete.contains("data-step works on any element"));
    assert!(!slide(&["--step", "0"]).contains("PgDn shows the next build step"));
    assert!(slide(&["--step", "1"]).contains("PgDn shows the next build step"));
    // The typewriter paragraph appears at step 4; a second in, it has typed 30 letters.
    let typing = slide(&["--step", "4", "--time", "1"]);
    assert!(
        typing.contains("The typewriter effect types it"),
        "{typing}"
    );
    assert!(
        !typing.contains("The typewriter effect types its text"),
        "{typing}"
    );
}

#[test]
fn render_colors_and_every_slide() {
    let output = run(&["render", "decks/demo", "--colors", "truecolor"]);
    let text = stdout(&output);
    assert!(text.contains("\x1b[0;38;2;"));
    let headers = text
        .lines()
        .filter(|l| l.starts_with("── ") && l.ends_with(".html) ──"));
    assert_eq!(headers.count(), 12, "a header per slide");
    let missing = run(&["render", "decks/demo", "--slide", "40"]);
    assert!(!missing.status.success());
}

#[test]
fn render_saves_a_png() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slide.png");
    let file = path.to_str().unwrap();
    let output = run(&[
        "render",
        "decks/demo",
        "-s",
        "9",
        "--png",
        file,
        "--scale",
        "2",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let picture = image::open(&path).unwrap();
    assert_eq!((picture.width(), picture.height()), (1280, 800));
    // A PNG is one slide: --slide is required.
    assert!(
        !run(&["render", "decks/demo", "--png", file])
            .status
            .success()
    );
}

#[test]
fn check_passes_a_clean_deck_and_fails_a_broken_one() {
    let clean = run(&["check", "decks/demo"]);
    assert!(clean.status.success());
    assert!(stdout(&clean).ends_with("12 slides, no problems\n"));

    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("01-bad.html"),
        "<p style='colour: red'>x</p><p style='margin-top: 30'>low</p>",
    )
    .unwrap();
    let broken = run(&["check", dir.path().to_str().unwrap()]);
    assert_eq!(broken.status.code(), Some(1));
    let text = stdout(&broken);
    assert!(
        text.contains("01-bad.html: unknown CSS property `colour`"),
        "{text}"
    );
    assert!(
        text.contains("01-bad.html: content (“low”) reaches row"),
        "{text}"
    );
    assert!(text.ends_with("1 slide, 2 problems\n"), "{text}");
    assert!(Path::new(dir.path()).exists());
}

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use color_eyre::Result;
use dos_kenota::app::App;
use dos_kenota::deck::Deck;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

/// A DOS-style text presentation engine.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Deck folder: an optional deck.toml plus one .html file per slide
    deck: PathBuf,

    /// Slide number to start on
    #[arg(short, long, value_name = "N", default_value_t = 1)]
    slide: usize,

    /// How to output colors
    #[arg(long, value_enum, default_value_t = Colors::Auto)]
    colors: Colors,
}

#[derive(Clone, Copy, ValueEnum)]
enum Colors {
    /// Truecolor when $COLORTERM says the terminal supports it, otherwise ANSI-16
    Auto,
    /// Exact EGA RGB values
    Truecolor,
    /// The terminal's own 16 colors
    Ansi16,
}

impl Colors {
    fn truecolor(self) -> bool {
        match self {
            Colors::Truecolor => true,
            Colors::Ansi16 => false,
            Colors::Auto => std::env::var("COLORTERM")
                .is_ok_and(|v| matches!(v.as_str(), "truecolor" | "24bit")),
        }
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    // Load before touching the terminal, so errors print normally.
    let deck = Deck::load(&cli.deck)?;
    let start = cli.slide.saturating_sub(1);
    let mut app = App::new(deck, start, cli.colors.truecolor(), Instant::now());

    let terminal = ratatui::init();
    let result = run(terminal, &mut app);
    ratatui::restore();
    result
}

/// What wakes the event loop.
enum Message {
    Input(Event),
}

/// Draws, then sleeps until input arrives or the screen next changes by
/// itself (an animation frame); with nothing animating, until input.
fn run(mut terminal: DefaultTerminal, app: &mut App) -> Result<()> {
    let (sender, messages) = mpsc::channel();
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if sender.send(Message::Input(event)).is_err() {
                break;
            }
        }
    });

    loop {
        let now = Instant::now();
        terminal.draw(|frame| app.draw(frame, now))?;
        let message = match app.next_deadline(now) {
            Some(deadline) => {
                match messages.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            None => match messages.recv() {
                Ok(message) => Some(message),
                Err(_) => break,
            },
        };
        // Handle everything waiting before drawing again.
        for Message::Input(event) in message.into_iter().chain(messages.try_iter()) {
            if let Event::Key(key) = event {
                app.handle_key(key, Instant::now());
            }
        }
        if app.should_quit() {
            break;
        }
    }
    Ok(())
}

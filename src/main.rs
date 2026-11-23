use std::path::PathBuf;

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
    let mut app = App::new(deck, cli.slide.saturating_sub(1), cli.colors.truecolor());

    let terminal = ratatui::init();
    let result = run(terminal, &mut app);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit() {
        terminal.draw(|frame| app.draw(frame))?;
        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }
    }
    Ok(())
}

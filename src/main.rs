use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use color_eyre::eyre::bail;
use dos_kenota::anim::Clock;
use dos_kenota::ansi::{self, ColorMode};
use dos_kenota::app::App;
use dos_kenota::check;
use dos_kenota::deck::Deck;
use dos_kenota::render;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

/// A DOS-style text presentation engine.
///
/// `dos-kenota <deck>` presents a deck full screen; `render` and `check`
/// work without the interactive viewer.
#[derive(Parser)]
#[command(
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Deck folder: an optional deck.toml plus one .html file per slide
    #[arg(required = true)]
    deck: Option<PathBuf>,

    /// Slide number to start on
    #[arg(short, long, value_name = "N", default_value_t = 1)]
    slide: usize,

    /// How to output colors
    #[arg(long, value_enum, default_value_t = Colors::Auto)]
    colors: Colors,
}

#[derive(Subcommand)]
enum Command {
    /// Print slides to the terminal instead of presenting them
    Render(RenderArgs),
    /// Report problems: ignored CSS, unsupported tags, missing pictures and
    /// fonts, bad transitions, content off the screen
    Check {
        /// Deck folder
        deck: PathBuf,
    },
}

#[derive(Args)]
struct RenderArgs {
    /// Deck folder
    deck: PathBuf,

    /// Slide number; every slide when left out
    #[arg(short, long, value_name = "N")]
    slide: Option<usize>,

    /// Build step to show, from 0 (how the slide first appears); the last
    /// step when left out
    #[arg(long, value_name = "N")]
    step: Option<u32>,

    /// Seconds since the step appeared, to catch animations mid-way; when
    /// left out, typing has finished and the other effects are at rest
    #[arg(long, value_name = "SECONDS", value_parser = seconds)]
    time: Option<Duration>,

    /// How to output colors; `none` prints plain text
    #[arg(long, value_enum, default_value_t = RenderColors::Auto)]
    colors: RenderColors,
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
            Colors::Auto => terminal_has_truecolor(),
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum RenderColors {
    /// Truecolor when $COLORTERM says the terminal supports it, otherwise ANSI-16
    Auto,
    /// Exact EGA RGB values
    Truecolor,
    /// The terminal's own 16 colors
    Ansi16,
    /// Plain text without colors
    None,
}

impl RenderColors {
    fn mode(self) -> ColorMode {
        match self {
            RenderColors::Auto if terminal_has_truecolor() => ColorMode::Truecolor,
            RenderColors::Auto | RenderColors::Ansi16 => ColorMode::Ansi16,
            RenderColors::Truecolor => ColorMode::Truecolor,
            RenderColors::None => ColorMode::None,
        }
    }
}

fn terminal_has_truecolor() -> bool {
    std::env::var("COLORTERM").is_ok_and(|v| matches!(v.as_str(), "truecolor" | "24bit"))
}

/// `--time`: a number of seconds, 0 or more.
fn seconds(text: &str) -> Result<Duration, String> {
    text.parse::<f64>()
        .ok()
        .and_then(|s| Duration::try_from_secs_f64(s).ok())
        .ok_or_else(|| format!("`{text}` is not a number of seconds (0 or more)"))
}

fn main() -> Result<ExitCode> {
    color_eyre::install()?;
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Render(args)) => {
            render_slides(&args)?;
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Check { deck }) => check_deck(&deck),
        None => {
            let deck = cli.deck.expect("clap requires a deck without a command");
            present(&deck, cli.slide, cli.colors)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Presents the deck full screen until the user quits.
fn present(folder: &Path, slide: usize, colors: Colors) -> Result<()> {
    // Load before touching the terminal, so errors print normally.
    let deck = Deck::load(folder)?;
    let start = slide.saturating_sub(1);
    let mut app = App::new(deck, start, colors.truecolor(), Instant::now());

    let terminal = ratatui::init();
    let result = run(terminal, &mut app);
    ratatui::restore();
    result
}

/// `dos-kenota render`: prints slides as text, with or without colors.
fn render_slides(args: &RenderArgs) -> Result<()> {
    let deck = Deck::load(&args.deck)?;
    let count = deck.slides.len();
    let indices: Vec<usize> = match args.slide {
        Some(n) if (1..=count).contains(&n) => vec![n - 1],
        Some(n) => bail!("there is no slide {n}: the deck has {count}"),
        None => (0..count).collect(),
    };
    let mode = args.colors.mode();
    let mut text = String::new();
    for (i, &index) in indices.iter().enumerate() {
        let slide = &deck.slides[index];
        if indices.len() > 1 {
            if i > 0 {
                text.push('\n');
            }
            let file = slide.path.file_name().unwrap_or_default().to_string_lossy();
            text.push_str(&format!(
                "── {}/{count} {} ({file}) ──\n",
                index + 1,
                slide.title
            ));
        }
        let steps = slide.timeline.steps;
        let step = args.step.map_or(steps, |s| s.min(steps));
        let clock = match args.time {
            Some(time) => Clock {
                step,
                time,
                step_time: time,
                forward: true,
            },
            None => Clock {
                step,
                ..Clock::still(steps)
            },
        };
        let canvas = render::draw(&deck, index, &render::compile(&deck, index), &clock);
        text.push_str(&ansi::to_ansi(&canvas, mode));
    }
    match io::stdout().lock().write_all(text.as_bytes()) {
        // Piped into `head` or similar: stop quietly.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => Ok(result?),
    }
}

/// `dos-kenota check`: lists every problem; fails when there is one.
fn check_deck(folder: &Path) -> Result<ExitCode> {
    let deck = Deck::load(folder)?;
    let problems = check::check(&deck);
    for problem in &problems {
        println!("{problem}");
    }
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let slides = plural(deck.slides.len(), "slide");
    if problems.is_empty() {
        println!("{}: {slides}, no problems", folder.display());
        Ok(ExitCode::SUCCESS)
    } else {
        let found = plural(problems.len(), "problem");
        println!("{}: {slides}, {found}", folder.display());
        Ok(ExitCode::FAILURE)
    }
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

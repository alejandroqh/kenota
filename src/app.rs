//! Application state: which slide and build step are showing, what each
//! key does, and the transition playing between slides.
//!
//! Time comes in from outside (`now`), so the state is easy to test: the
//! event loop asks [`App::next_deadline`] when to draw again.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::anim::Clock;
use crate::anim::transition::{self, Transition};
use crate::deck::Deck;
use crate::render::{self, Compiled};
use crate::{chrome, palette};

/// How often a transition redraws.
const TRANSITION_FRAME: Duration = Duration::from_millis(33);

pub struct App {
    deck: Deck,
    current: usize,
    step: u32,
    /// `false` after going back, so typewriters show their whole text.
    forward: bool,
    slide_started: Instant,
    step_started: Instant,
    /// Each slide laid out once, when first shown.
    compiled: Vec<Option<Compiled>>,
    transition: Option<Running>,
    truecolor: bool,
    quit: bool,
}

/// A transition in progress.
struct Running {
    /// The screen as it was when the slide changed.
    from: Buffer,
    transition: Transition,
    started: Instant,
    backwards: bool,
}

impl App {
    /// `start` is a zero-based slide index; it is clamped to the last slide.
    pub fn new(deck: Deck, start: usize, truecolor: bool, now: Instant) -> Self {
        let current = start.min(deck.slides.len() - 1);
        let compiled = deck.slides.iter().map(|_| None).collect();
        Self {
            deck,
            current,
            step: 0,
            forward: true,
            slide_started: now,
            step_started: now,
            compiled,
            transition: None,
            truecolor,
            quit: false,
        }
    }

    /// Zero-based index of the slide on screen.
    pub fn current(&self) -> usize {
        self.current
    }

    /// The build step showing on the current slide.
    pub fn step(&self) -> u32 {
        self.step
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    fn steps(&self, index: usize) -> u32 {
        self.deck.slides[index].timeline.steps
    }

    /// The next build step, else the next slide.
    pub fn next(&mut self, now: Instant) {
        if self.step < self.steps(self.current) {
            self.step += 1;
            self.step_started = now;
            self.forward = true;
        } else if self.current + 1 < self.deck.slides.len() {
            self.go(self.current + 1, 0, true, now, true);
        }
    }

    /// The previous build step, else the previous slide fully built.
    pub fn prev(&mut self, now: Instant) {
        if self.step > 0 {
            self.step -= 1;
            self.step_started = now;
            self.forward = false;
        } else if self.current > 0 {
            self.go(
                self.current - 1,
                self.steps(self.current - 1),
                false,
                now,
                true,
            );
        }
    }

    pub fn first(&mut self, now: Instant) {
        self.go(0, 0, true, now, false);
    }

    /// The last slide, fully built.
    pub fn last(&mut self, now: Instant) {
        let last = self.deck.slides.len() - 1;
        self.go(last, self.steps(last), false, now, false);
    }

    /// Shows slide `index` at `step`, with its transition when `animate`.
    /// Going back plays the transition of the slide being left, mirrored.
    /// Without `forward`, typewriters on the slide show their whole text.
    fn go(&mut self, index: usize, step: u32, forward: bool, now: Instant, animate: bool) {
        let backwards = index < self.current;
        let entering = if backwards { self.current } else { index };
        let transition = self.deck.slides[entering].transition_in(&self.deck.config);
        self.transition = if animate && index != self.current && !transition.is_cut() {
            Some(Running {
                from: self.canvas(now),
                transition,
                started: now,
                backwards,
            })
        } else {
            None
        };
        self.current = index;
        self.step = step;
        self.forward = forward;
        self.slide_started = now;
        self.step_started = now;
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::PageDown
            | KeyCode::Right
            | KeyCode::Down
            | KeyCode::Enter
            | KeyCode::Char(' ') => self.next(now),
            KeyCode::PageUp | KeyCode::Left | KeyCode::Up | KeyCode::Backspace => self.prev(now),
            KeyCode::Home => self.first(now),
            KeyCode::End => self.last(now),
            _ => {}
        }
    }

    fn clock(&self, now: Instant) -> Clock {
        Clock {
            step: self.step,
            time: now.saturating_duration_since(self.slide_started),
            step_time: now.saturating_duration_since(self.step_started),
            forward: self.forward,
        }
    }

    fn compiled(&mut self, index: usize) -> &Compiled {
        let deck = &self.deck;
        self.compiled[index].get_or_insert_with(|| render::compile(deck, index))
    }

    /// The canvas as it looks at `now`, mid-transition or not.
    pub fn canvas(&mut self, now: Instant) -> Buffer {
        let clock = self.clock(now);
        let index = self.current;
        self.compiled(index);
        let compiled = self.compiled[index].as_ref().expect("compiled above");
        let canvas = render::draw(&self.deck, index, compiled, &clock);
        if let Some(running) = &self.transition {
            let elapsed = now.saturating_duration_since(running.started);
            if elapsed < running.transition.duration {
                let progress = elapsed.as_secs_f64() / running.transition.duration.as_secs_f64();
                return transition::blend(
                    &running.from,
                    &canvas,
                    running.transition.kind,
                    progress,
                    running.backwards,
                );
            }
            self.transition = None;
        }
        canvas
    }

    /// When the screen next changes by itself, if it does.
    pub fn next_deadline(&mut self, now: Instant) -> Option<Instant> {
        let mut next = None;
        if let Some(running) = &self.transition {
            let end = running.started + running.transition.duration;
            if now < end {
                next = Some((now + TRANSITION_FRAME).min(end));
            }
        }
        let clock = self.clock(now);
        let index = self.current;
        let timeline = &self.deck.slides[index].timeline;
        let compiled =
            self.compiled[index].get_or_insert_with(|| render::compile(&self.deck, index));
        if let Some(wait) = render::next_change(compiled, timeline, &clock) {
            let at = now + wait;
            next = Some(next.map_or(at, |n: Instant| n.min(at)));
        }
        next
    }

    pub fn draw(&mut self, frame: &mut Frame, now: Instant) {
        let canvas = self.canvas(now);
        let area = frame.area();
        let buf = frame.buffer_mut();
        chrome::present(buf, area, &canvas);
        if !self.truecolor {
            palette::downgrade_to_ansi16(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventState;
    use ratatui::style::Color;

    use super::*;
    use crate::deck::{test_deck, test_slide};
    use crate::render::buffer_text;

    fn app(slides: usize) -> App {
        let titles: Vec<String> = (1..=slides).map(|i| format!("Slide {i}")).collect();
        let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
        App::new(test_deck(&titles), 0, true, Instant::now())
    }

    /// An app showing slides with these bodies, starting at `now`.
    fn app_with(bodies: &[&str], now: Instant) -> App {
        let mut deck = test_deck(&[]);
        for (i, body) in bodies.iter().enumerate() {
            deck.slides.push(test_slide(&format!("S{i}"), body));
            assert!(
                deck.slides[i].warnings.is_empty(),
                "{:?}",
                deck.slides[i].warnings
            );
        }
        App::new(deck, 0, true, now)
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), Instant::now());
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// The first line of the canvas (the slide's first row).
    fn first_line(app: &mut App, now: Instant) -> String {
        buffer_text(&app.canvas(now))
            .lines()
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string()
    }

    #[test]
    fn navigation_stays_in_bounds() {
        let mut app = app(3);
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.current(), 0);
        press(&mut app, KeyCode::PageDown);
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Right);
        assert_eq!(app.current(), 2);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.current(), 1);
        press(&mut app, KeyCode::Home);
        assert_eq!(app.current(), 0);
        press(&mut app, KeyCode::End);
        assert_eq!(app.current(), 2);
    }

    #[test]
    fn start_is_clamped() {
        let app = App::new(test_deck(&["a", "b"]), 99, true, Instant::now());
        assert_eq!(app.current(), 1);
    }

    #[test]
    fn quit_keys() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = app(1);
            press(&mut app, code);
            assert!(app.should_quit());
        }
        let mut app = app(1);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Instant::now(),
        );
        assert!(app.should_quit());
    }

    #[test]
    fn key_releases_are_ignored() {
        let mut app = app(2);
        app.handle_key(
            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Release,
                state: KeyEventState::NONE,
            },
            Instant::now(),
        );
        assert_eq!(app.current(), 0);
    }

    #[test]
    fn build_steps_come_before_the_next_slide() {
        let t0 = Instant::now();
        let mut app = app_with(
            &[
                "<p>always</p><ul data-build><li>one</li><li>two</li></ul>",
                "<p>next</p>",
            ],
            t0,
        );
        let text = |app: &mut App| buffer_text(&app.canvas(t0));
        assert!(!text(&mut app).contains("one"));
        app.next(t0);
        assert_eq!((app.current(), app.step()), (0, 1));
        assert!(text(&mut app).contains("one") && !text(&mut app).contains("two"));
        app.next(t0);
        app.next(t0);
        assert_eq!((app.current(), app.step()), (1, 0));
        app.prev(t0);
        assert_eq!(
            (app.current(), app.step()),
            (0, 2),
            "back to the slide fully built"
        );
        app.prev(t0);
        assert_eq!(app.step(), 1);
        app.last(t0);
        app.first(t0);
        assert_eq!((app.current(), app.step()), (0, 0));
    }

    #[test]
    fn transitions_play_between_slides() {
        let t0 = Instant::now();
        let mut app = app_with(
            &[
                "<p>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</p>",
                "<meta name='transition' content='wipe-right 400ms'>
                 <p>bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb</p>",
            ],
            t0,
        );
        assert_eq!(
            app.next_deadline(t0),
            None,
            "a still slide never redraws by itself"
        );
        app.next(t0);
        let halfway = first_line(&mut app, t0 + ms(200));
        assert!(
            halfway.starts_with("bbbb") && halfway.ends_with("aaaa"),
            "{halfway}"
        );
        assert_eq!(app.next_deadline(t0 + ms(200)), Some(t0 + ms(233)));
        assert_eq!(app.next_deadline(t0 + ms(390)), Some(t0 + ms(400)));
        assert!(!first_line(&mut app, t0 + ms(400)).contains('a'));
        assert_eq!(app.next_deadline(t0 + ms(400)), None);

        // Going back plays it mirrored: the old slide is uncovered from the right.
        app.prev(t0 + ms(1000));
        let back = first_line(&mut app, t0 + ms(1200));
        assert!(back.starts_with("bbbb") && back.ends_with("aaaa"), "{back}");
    }

    #[test]
    fn typewriter_types_then_rests() {
        let t0 = Instant::now();
        let mut app = app_with(&["<p data-effect='typewriter'>Hello world</p>"], t0);
        assert_eq!(first_line(&mut app, t0 + ms(100)), "Hel");
        let next = app.next_deadline(t0 + ms(100)).unwrap();
        assert_eq!(
            (next - t0).as_millis(),
            133,
            "when the fourth letter appears"
        );
        assert_eq!(first_line(&mut app, t0 + ms(1000)), "Hello world");
        assert_eq!(app.next_deadline(t0 + ms(1000)), None);
    }

    #[test]
    fn typewriter_in_a_build_step_starts_with_it() {
        let t0 = Instant::now();
        let mut app = app_with(&["<p data-step data-effect='typewriter'>Hello</p>"], t0);
        assert_eq!(first_line(&mut app, t0 + ms(500)), "");
        app.next(t0 + ms(500));
        assert_eq!(first_line(&mut app, t0 + ms(600)), "Hel");
        app.prev(t0 + ms(700));
        app.next(t0 + ms(800));
        assert_eq!(first_line(&mut app, t0 + ms(800)), "", "types again");
    }

    #[test]
    fn draw_shows_slide_and_downgrades_colors() {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("Hello", "<p>Hi there</p>"));
        let mut app = App::new(deck, 0, false, Instant::now());
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();
        terminal
            .draw(|frame| app.draw(frame, Instant::now()))
            .unwrap();

        let buf = terminal.backend().buffer();
        assert!(buffer_text(buf).contains("Hi there"));
        assert!(
            buf.content
                .iter()
                .all(|c| !matches!(c.fg, Color::Rgb(..)) && !matches!(c.bg, Color::Rgb(..)))
        );
    }
}

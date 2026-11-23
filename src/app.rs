//! Application state: which slide is showing, and what each key does.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::deck::Deck;
use crate::{chrome, palette, render};

pub struct App {
    deck: Deck,
    current: usize,
    truecolor: bool,
    quit: bool,
}

impl App {
    /// `start` is a zero-based slide index; it is clamped to the last slide.
    pub fn new(deck: Deck, start: usize, truecolor: bool) -> Self {
        let current = start.min(deck.slides.len() - 1);
        Self {
            deck,
            current,
            truecolor,
            quit: false,
        }
    }

    /// Zero-based index of the slide on screen.
    pub fn current(&self) -> usize {
        self.current
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn next(&mut self) {
        if self.current + 1 < self.deck.slides.len() {
            self.current += 1;
        }
    }

    pub fn prev(&mut self) {
        self.current = self.current.saturating_sub(1);
    }

    pub fn first(&mut self) {
        self.current = 0;
    }

    pub fn last(&mut self) {
        self.current = self.deck.slides.len() - 1;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
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
            | KeyCode::Char(' ') => self.next(),
            KeyCode::PageUp | KeyCode::Left | KeyCode::Up | KeyCode::Backspace => self.prev(),
            KeyCode::Home => self.first(),
            KeyCode::End => self.last(),
            _ => {}
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let canvas = render::slide_canvas(&self.deck, self.current);
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
        App::new(test_deck(&titles), 0, true)
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
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
        let app = App::new(test_deck(&["a", "b"]), 99, true);
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
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit());
    }

    #[test]
    fn key_releases_are_ignored() {
        let mut app = app(2);
        app.handle_key(KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        });
        assert_eq!(app.current(), 0);
    }

    #[test]
    fn draw_shows_slide_and_downgrades_colors() {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("Hello", "<p>Hi there</p>"));
        let app = App::new(deck, 0, false);
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        let buf = terminal.backend().buffer();
        assert!(buffer_text(buf).contains("Hi there"));
        assert!(
            buf.content
                .iter()
                .all(|c| !matches!(c.fg, Color::Rgb(..)) && !matches!(c.bg, Color::Rgb(..)))
        );
    }
}

//! Slide transitions: how the next slide replaces the one on screen.
//!
//! A slide picks one with `<meta name="transition" content="wipe-right">`,
//! the deck with `transition = "dissolve"` in `deck.toml`. An optional
//! duration follows the name: `"push 0.8s"` or `"blinds 300ms"`. Each
//! transition decides, for every cell and moment, which cell of the old or
//! the new canvas shows. Going back plays wipes and pushes the other way.

use std::fmt;
use std::time::Duration;

use ratatui::buffer::Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cut,
    /// The new slide is uncovered from left to right.
    WipeRight,
    WipeLeft,
    WipeDown,
    WipeUp,
    /// Cells switch over in random order.
    Dissolve,
    /// Vertical slats open across the slide.
    Blinds,
    /// The new slide slides in from the right, pushing the old one out.
    Push,
}

impl Kind {
    pub const NAMES: [&str; 8] = [
        "cut",
        "wipe-right",
        "wipe-left",
        "wipe-down",
        "wipe-up",
        "dissolve",
        "blinds",
        "push",
    ];
    const ALL: [Kind; 8] = [
        Kind::Cut,
        Kind::WipeRight,
        Kind::WipeLeft,
        Kind::WipeDown,
        Kind::WipeUp,
        Kind::Dissolve,
        Kind::Blinds,
        Kind::Push,
    ];

    fn default_duration(self) -> Duration {
        Duration::from_millis(match self {
            Kind::Cut => 0,
            Kind::WipeRight | Kind::WipeLeft | Kind::WipeDown | Kind::WipeUp | Kind::Push => 400,
            Kind::Blinds => 500,
            Kind::Dissolve => 600,
        })
    }

    /// The same transition played backwards in space.
    fn reversed(self) -> Kind {
        match self {
            Kind::WipeRight => Kind::WipeLeft,
            Kind::WipeLeft => Kind::WipeRight,
            Kind::WipeDown => Kind::WipeUp,
            Kind::WipeUp => Kind::WipeDown,
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub kind: Kind,
    pub duration: Duration,
}

impl Transition {
    pub const CUT: Transition = Transition {
        kind: Kind::Cut,
        duration: Duration::ZERO,
    };

    /// Reads `name [duration]`, the duration in `s` or `ms`.
    ///
    /// # Errors
    /// A message naming the problem: an unknown name or a bad duration.
    pub fn parse(text: &str) -> Result<Transition, String> {
        let mut parts = text.split_whitespace();
        let name = parts.next().unwrap_or("cut");
        let kind = Kind::NAMES
            .iter()
            .position(|known| known.eq_ignore_ascii_case(name))
            .map(|i| Kind::ALL[i])
            .ok_or_else(|| {
                format!(
                    "unknown transition `{name}` (transitions: {})",
                    Kind::NAMES.join(", ")
                )
            })?;
        let duration = match parts.next() {
            None => kind.default_duration(),
            Some(value) => duration(value)
                .ok_or_else(|| format!("transition duration `{value}`: use `0.5s` or `500ms`"))?,
        };
        if let Some(extra) = parts.next() {
            return Err(format!("unexpected `{extra}` after the transition"));
        }
        Ok(Transition { kind, duration })
    }

    /// Whether there is anything to animate.
    pub fn is_cut(&self) -> bool {
        self.kind == Kind::Cut || self.duration.is_zero()
    }
}

impl<'de> serde::Deserialize<'de> for Transition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Transition::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = Kind::NAMES[Kind::ALL.iter().position(|k| *k == self.kind).unwrap_or(0)];
        write!(f, "{name} {}ms", self.duration.as_millis())
    }
}

fn duration(value: &str) -> Option<Duration> {
    let value = value.trim().to_ascii_lowercase();
    let seconds = if let Some(ms) = value.strip_suffix("ms") {
        ms.parse::<f64>().ok()? / 1000.0
    } else {
        value.strip_suffix('s')?.parse::<f64>().ok()?
    };
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds.min(60.0)))
}

/// Which canvas a cell shows during a transition, and from where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Old(u16, u16),
    New(u16, u16),
}

/// The screen `progress` (0 to 1) of the way from `old` to `new`, which
/// have the same size. `backwards` plays the transition mirrored, for
/// going back a slide.
pub fn blend(old: &Buffer, new: &Buffer, kind: Kind, progress: f64, backwards: bool) -> Buffer {
    let kind = if backwards { kind.reversed() } else { kind };
    let area = new.area;
    let mut out = new.clone();
    let (width, height) = (area.width, area.height);
    let t = progress.clamp(0.0, 1.0);
    for y in 0..height {
        for x in 0..width {
            let pick = pick(kind, x, y, width, height, t, backwards);
            let cell = match pick {
                Pick::Old(x, y) => old.cell((area.x + x, area.y + y)),
                Pick::New(x, y) => new.cell((area.x + x, area.y + y)),
            };
            if let (Some(cell), Some(target)) = (cell, out.cell_mut((area.x + x, area.y + y))) {
                *target = cell.clone();
            }
        }
    }
    out
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments
)]
fn pick(kind: Kind, x: u16, y: u16, width: u16, height: u16, t: f64, backwards: bool) -> Pick {
    let past = |position: u16, size: u16| f64::from(position) < t * f64::from(size);
    let new = Pick::New(x, y);
    let old = Pick::Old(x, y);
    let choose = |show_new: bool| if show_new { new } else { old };
    match kind {
        Kind::Cut => new,
        Kind::WipeRight => choose(past(x, width)),
        Kind::WipeLeft => choose(past(width - 1 - x, width)),
        Kind::WipeDown => choose(past(y, height)),
        Kind::WipeUp => choose(past(height - 1 - y, height)),
        Kind::Dissolve => choose(noise(x, y) < t),
        Kind::Blinds => {
            const SLAT: u16 = 8;
            let within = if backwards {
                SLAT - 1 - x % SLAT
            } else {
                x % SLAT
            };
            choose(past(within, SLAT))
        }
        Kind::Push => {
            let shift = (t * f64::from(width)).round() as u16;
            if backwards {
                // The old slide moves right; the new one follows from the left.
                if x < shift {
                    Pick::New(width - shift + x, y)
                } else {
                    Pick::Old(x - shift, y)
                }
            } else if x + shift >= width {
                Pick::New(x + shift - width, y)
            } else {
                Pick::Old(x + shift, y)
            }
        }
    }
}

/// A fixed pseudo-random number in `0..1` for each cell.
fn noise(x: u16, y: u16) -> f64 {
    let mut h = u32::from(x).wrapping_mul(0x9E37_79B1) ^ u32::from(y).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    f64::from(h % 10_000) / 10_000.0
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::render::buffer_text;

    fn screen(fill: &str) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        for y in 0..2 {
            buf.set_string(0, y, fill.repeat(8), ratatui::style::Style::new());
        }
        buf
    }

    #[test]
    fn parses_names_and_durations() {
        assert_eq!(
            Transition::parse("Wipe-Right").unwrap(),
            Transition {
                kind: Kind::WipeRight,
                duration: Duration::from_millis(400)
            }
        );
        assert_eq!(
            Transition::parse("push 0.8s").unwrap().duration,
            Duration::from_millis(800)
        );
        assert_eq!(
            Transition::parse("blinds 300ms").unwrap().duration,
            Duration::from_millis(300)
        );
        assert!(Transition::parse("cut").unwrap().is_cut());
        assert!(
            Transition::parse("spin")
                .unwrap_err()
                .starts_with("unknown transition `spin`")
        );
        assert!(Transition::parse("push fast").is_err());
        assert!(Transition::parse("push 1s now").is_err());
    }

    #[test]
    fn wipes_and_pushes() {
        let (old, new) = (screen("o"), screen("n"));
        let at = |kind, t, back| buffer_text(&blend(&old, &new, kind, t, back));
        assert_eq!(at(Kind::WipeRight, 0.5, false), "nnnnoooo\nnnnnoooo");
        assert_eq!(at(Kind::WipeRight, 0.5, true), "oooonnnn\noooonnnn");
        assert_eq!(at(Kind::WipeDown, 0.5, false), "nnnnnnnn\noooooooo");
        assert_eq!(at(Kind::WipeRight, 0.0, false), "oooooooo\noooooooo");
        assert_eq!(at(Kind::WipeRight, 1.0, false), "nnnnnnnn\nnnnnnnnn");
        assert_eq!(at(Kind::Blinds, 0.5, false), "nnnnoooo\nnnnnoooo");
    }

    #[test]
    fn push_moves_both_slides() {
        let mut old = Buffer::empty(Rect::new(0, 0, 4, 1));
        old.set_string(0, 0, "abcd", ratatui::style::Style::new());
        let mut new = old.clone();
        new.set_string(0, 0, "WXYZ", ratatui::style::Style::new());
        assert_eq!(
            buffer_text(&blend(&old, &new, Kind::Push, 0.25, false)),
            "bcdW"
        );
        assert_eq!(
            buffer_text(&blend(&old, &new, Kind::Push, 0.5, true)),
            "YZab"
        );
    }

    #[test]
    fn dissolve_switches_cells_gradually() {
        let (old, new) = (screen("o"), screen("n"));
        let count = |t| {
            buffer_text(&blend(&old, &new, Kind::Dissolve, t, false))
                .matches('n')
                .count()
        };
        assert_eq!(count(0.0), 0);
        assert!((3..14).contains(&count(0.5)), "{}", count(0.5));
        assert_eq!(count(1.0), 16);
    }
}

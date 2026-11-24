//! kenota: a DOS-style, text-only presentation engine.
//!
//! A deck is a folder of HTML slides. Each slide is styled with a CSS
//! subset, laid out in character cells, and painted onto a fixed text
//! canvas (80×25 by default) that is independent of the terminal. The
//! canvas is then shown centered in the terminal with a footer bar.

pub mod anim;
pub mod ansi;
pub mod app;
pub mod assets;
pub mod chart;
pub mod check;
pub mod chrome;
pub mod deck;
pub mod figlet;
pub mod fonts;
pub mod html;
pub mod images;
pub mod layout;
pub mod palette;
pub mod pixels;
pub mod render;
pub mod style;

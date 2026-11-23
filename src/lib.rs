//! dos-kenota: a DOS-style, text-only presentation engine.
//!
//! A deck is a folder of HTML slides. Each slide is rendered onto a fixed
//! text canvas (80×25 by default) that is independent of the terminal, then
//! presented centered in the terminal with a footer bar.

pub mod app;
pub mod chrome;
pub mod deck;
pub mod html;
pub mod palette;
pub mod render;

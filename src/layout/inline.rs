//! Inline layout: flowing text through lines.

use std::mem;

use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::measure::Intrinsic;
use super::{Item, Layer, Tag, bigtext, is_absolute, text_style};
use crate::assets::Assets;
use crate::palette::Ega;
use crate::style::{ComputedStyle, Content, StyledNode, TextAlign, TextTransform};

/// How a piece of text is drawn, and when it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pen {
    pub style: Style,
    pub tag: Tag,
}

/// A piece of inline content.
#[derive(Debug, PartialEq)]
pub(super) enum Atom {
    Word(String, Pen),
    /// A space; `true` when preserved (`white-space: pre…`).
    Space(Pen, bool),
    Break,
}

type Line = Vec<(String, Pen)>;

/// Lays out `run` (consecutive inline siblings) as lines starting at row
/// `y`. Returns how many rows it took (whitespace-only runs take none),
/// and the absolutely positioned elements found inside, which are not
/// part of the text. A container with a `--font` draws big text.
#[allow(clippy::too_many_arguments)]
pub(super) fn layout<'a>(
    run: &[&'a StyledNode],
    container: &ComputedStyle,
    assets: &Assets,
    x: i32,
    y: i32,
    width: i32,
    out: &mut Layer,
) -> (i32, Vec<&'a StyledNode>) {
    let (atoms, absolute) = atoms(run);
    if let Some(font) = bigtext::font(container, &assets.fonts) {
        let rows = bigtext::layout(&atoms, font, container.text_align, x, y, width, out);
        return (rows, absolute);
    }
    let lines = break_lines(
        &atoms,
        usize::try_from(width.max(1)).unwrap_or(1),
        container.white_space.wraps(),
    );
    let mut row = y;
    for line in &lines {
        emit(line, x, row, width, container.text_align, out);
        row += 1;
    }
    (row - y, absolute)
}

/// The narrowest and widest `run` can be laid out.
pub(super) fn intrinsic(
    run: &[&StyledNode],
    container: &ComputedStyle,
    assets: &Assets,
) -> Intrinsic {
    let (atoms, _) = atoms(run);
    if let Some(font) = bigtext::font(container, &assets.fonts) {
        return bigtext::intrinsic(&atoms, font);
    }
    let width = |text: &str| i32::try_from(text.width()).unwrap_or(i32::MAX);
    let max = break_lines(&atoms, usize::MAX, false)
        .iter()
        .map(|line| line.iter().map(|(text, _)| width(text)).sum())
        .max()
        .unwrap_or(0);
    let min = if container.white_space.wraps() {
        atoms
            .iter()
            .map(|atom| match atom {
                Atom::Word(word, _) => width(word),
                Atom::Space(_, true) => 1,
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    } else {
        max
    };
    Intrinsic { min, max }
}

/// The atoms of `run`, and the absolutely positioned elements inside it.
fn atoms<'a>(run: &[&'a StyledNode]) -> (Vec<Atom>, Vec<&'a StyledNode>) {
    let mut atoms = Vec::new();
    let mut absolute = Vec::new();
    for node in run {
        collect(node, None, &mut atoms, &mut absolute);
    }
    (atoms, absolute)
}

/// Flattens inline content into atoms. `background` comes from enclosing
/// inline elements, so `<mark><b>x</b></mark>` keeps the mark's color.
fn collect<'a>(
    node: &'a StyledNode,
    background: Option<Ega>,
    atoms: &mut Vec<Atom>,
    absolute: &mut Vec<&'a StyledNode>,
) {
    if is_absolute(node) {
        absolute.push(node);
        return;
    }
    match &node.content {
        Content::Text(text) => {
            let style = Pen {
                style: text_style(&node.style, background),
                tag: Tag::of(&node.style),
            };
            let text = match node.style.text_transform {
                TextTransform::None => text.clone(),
                TextTransform::Uppercase => text.to_uppercase(),
                TextTransform::Lowercase => text.to_lowercase(),
            };
            if node.style.white_space.collapses() {
                collapsed_atoms(&text, style, atoms);
            } else {
                preserved_atoms(&text, style, atoms);
            }
        }
        Content::Element { tag, .. } => {
            if tag == "br" {
                atoms.push(Atom::Break);
                return;
            }
            let background = node.style.background.or(background);
            for child in &node.children {
                collect(child, background, atoms, absolute);
            }
        }
    }
}

/// HTML whitespace. Non-breaking spaces are not included: they stay in words.
pub(super) fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

fn collapsed_atoms(text: &str, style: Pen, atoms: &mut Vec<Atom>) {
    let mut word = String::new();
    for c in text.chars() {
        if is_html_space(c) {
            if !word.is_empty() {
                atoms.push(Atom::Word(mem::take(&mut word), style));
            }
            if !matches!(atoms.last(), Some(Atom::Space(_, false))) {
                atoms.push(Atom::Space(style, false));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        atoms.push(Atom::Word(word, style));
    }
}

fn preserved_atoms(text: &str, style: Pen, atoms: &mut Vec<Atom>) {
    const TAB_STOP: usize = 8;
    let mut word = String::new();
    let mut column = 0;
    let flush = |word: &mut String, atoms: &mut Vec<Atom>| {
        if !word.is_empty() {
            atoms.push(Atom::Word(mem::take(word), style));
        }
    };
    for c in text.chars() {
        match c {
            '\n' => {
                flush(&mut word, atoms);
                atoms.push(Atom::Break);
                column = 0;
            }
            '\r' => {}
            ' ' | '\t' => {
                flush(&mut word, atoms);
                let n = if c == '\t' {
                    TAB_STOP - column % TAB_STOP
                } else {
                    1
                };
                for _ in 0..n {
                    atoms.push(Atom::Space(style, true));
                }
                column += n;
            }
            c => {
                word.push(c);
                column += c.width().unwrap_or(0);
            }
        }
    }
    flush(&mut word, atoms);
}

fn break_lines(atoms: &[Atom], width: usize, wrap: bool) -> Vec<Line> {
    let mut breaker = LineBreaker {
        lines: Vec::new(),
        current: Vec::new(),
        current_width: 0,
        pending_space: None,
        width,
        wrap,
    };
    for atom in atoms {
        match atom {
            Atom::Word(word, style) => breaker.word(word, *style),
            Atom::Space(style, false) => breaker.space(*style),
            Atom::Space(style, true) => breaker.preserved_space(*style),
            Atom::Break => breaker.newline(),
        }
    }
    breaker.finish()
}

struct LineBreaker {
    lines: Vec<Line>,
    current: Line,
    current_width: usize,
    /// A collapsible space waiting for the next word on the same line.
    pending_space: Option<Pen>,
    width: usize,
    wrap: bool,
}

impl LineBreaker {
    fn push(&mut self, text: &str, style: Pen) {
        match self.current.last_mut() {
            Some((last, last_style)) if *last_style == style => last.push_str(text),
            _ => self.current.push((text.to_string(), style)),
        }
        self.current_width += text.width();
    }

    fn word(&mut self, word: &str, style: Pen) {
        let width = word.width();
        let space = self.pending_space.take().filter(|_| self.current_width > 0);
        let space_width = usize::from(space.is_some());

        if self.wrap
            && self.current_width > 0
            && self.current_width + space_width + width > self.width
        {
            self.newline();
        } else if let Some(space_style) = space {
            self.push(" ", space_style);
        }

        if self.wrap && width > self.width {
            for c in word.chars() {
                let c_width = c.width().unwrap_or(0);
                if self.current_width > 0 && self.current_width + c_width > self.width {
                    self.newline();
                }
                self.push(c.encode_utf8(&mut [0; 4]), style);
            }
        } else {
            self.push(word, style);
        }
    }

    fn space(&mut self, style: Pen) {
        if self.current_width > 0 && self.pending_space.is_none() {
            self.pending_space = Some(style);
        }
    }

    fn preserved_space(&mut self, style: Pen) {
        if let Some(space_style) = self.pending_space.take() {
            self.push(" ", space_style);
        }
        if self.wrap && self.current_width + 1 > self.width {
            self.newline();
        } else {
            self.push(" ", style);
        }
    }

    fn newline(&mut self) {
        self.lines.push(mem::take(&mut self.current));
        self.current_width = 0;
        self.pending_space = None;
    }

    fn finish(mut self) -> Vec<Line> {
        if !self.current.is_empty() {
            self.lines.push(self.current);
        }
        self.lines
    }
}

fn emit(line: &Line, x: i32, y: i32, width: i32, align: TextAlign, out: &mut Layer) {
    let line_width: i32 = line
        .iter()
        .map(|(text, _)| i32::try_from(text.width()).unwrap_or(i32::MAX))
        .sum();
    let offset = match align {
        TextAlign::Left => 0,
        TextAlign::Center => (width - line_width) / 2,
        TextAlign::Right => width - line_width,
    }
    .max(0);
    let mut cursor = x + offset;
    for (text, pen) in line {
        out.push_tagged(
            Item::Text {
                x: cursor,
                y,
                text: text.replace('\u{a0}', " "),
                style: pen.style,
            },
            pen.tag,
        );
        cursor += i32::try_from(text.width()).unwrap_or(i32::MAX);
    }
}

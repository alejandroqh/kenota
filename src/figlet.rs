//! FIGlet fonts: large letters drawn with ordinary characters.
//!
//! Reads `.flf` files (format `flf2a`) and lays characters out the way the
//! reference `figlet` program does: full width, kerning (characters slide
//! together until they touch), or smushing (they overlap by one column,
//! merged by the font's smushing rules). Each output cell carries a tag
//! (a text style, say) from the character that drew it.

use std::collections::HashMap;
use std::fmt;

/// Smushing rule bits from the font header.
mod rule {
    pub const EQUAL: u32 = 1;
    pub const LOWLINE: u32 = 2;
    pub const HIERARCHY: u32 = 4;
    pub const PAIR: u32 = 8;
    pub const BIG_X: u32 = 16;
    pub const HARDBLANK: u32 = 32;
    pub const KERN: u32 = 64;
    pub const SMUSH: u32 = 128;
}

/// Characters every font defines after 32–126, in this order.
const DEUTSCH: [u32; 7] = [196, 214, 220, 228, 246, 252, 223];

#[derive(Clone)]
pub struct Font {
    name: String,
    height: usize,
    hardblank: char,
    /// Layout bits: the smushing rules plus [`rule::KERN`] / [`rule::SMUSH`].
    mode: u32,
    glyphs: HashMap<char, Glyph>,
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("name", &self.name)
            .field("height", &self.height)
            .field("glyphs", &self.glyphs.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct Glyph {
    /// `height` rows, all `width` long.
    rows: Vec<Vec<char>>,
    width: usize,
}

/// Rendered big text: `rows[row][column]`, `None` where nothing is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Art<T> {
    pub rows: Vec<Vec<Option<(char, T)>>>,
}

impl<T> Art<T> {
    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }
}

impl Font {
    /// Parses the text of a `.flf` file.
    ///
    /// # Errors
    /// If the header is missing or invalid, or the file ends before the
    /// printable ASCII characters are all defined.
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = lines.next().ok_or("the file is empty")?;
        let rest = header
            .strip_prefix("flf2a")
            .ok_or("not a FIGlet font (the first line must start with `flf2a`)")?;
        let mut chars = rest.chars();
        let hardblank = chars
            .next()
            .ok_or("the header has no hardblank character")?;
        let numbers = chars
            .as_str()
            .split_whitespace()
            .map(str::parse::<i64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "the header has a value that is not a number")?;
        let [
            height,
            _baseline,
            _max_length,
            old_layout,
            comment_lines,
            ..,
        ] = numbers[..]
        else {
            return Err("the header needs at least five numbers".into());
        };
        let height = usize::try_from(height)
            .ok()
            .filter(|h| *h > 0)
            .ok_or("the height must be at least 1")?;
        let full_layout = numbers.get(6).copied();
        let mode = match full_layout {
            Some(full) => u32::try_from(full & 0xff).unwrap_or(0),
            None if old_layout < 0 => 0,
            None if old_layout == 0 => rule::KERN,
            None => u32::try_from(old_layout & 31).unwrap_or(0) | rule::SMUSH,
        };

        for _ in 0..comment_lines {
            lines.next();
        }

        let mut glyphs = HashMap::new();
        let read_glyph = |lines: &mut std::str::Lines<'_>| -> Option<Glyph> {
            let mut rows = Vec::with_capacity(height);
            for _ in 0..height {
                rows.push(strip_endmarks(lines.next()?).chars().collect::<Vec<_>>());
            }
            let width = rows.iter().map(Vec::len).max().unwrap_or(0);
            for row in &mut rows {
                row.resize(width, ' ');
            }
            Some(Glyph { rows, width })
        };

        for code in (32..=126).chain(DEUTSCH) {
            match read_glyph(&mut lines) {
                Some(glyph) => {
                    if let Some(c) = char::from_u32(code) {
                        glyphs.insert(c, glyph);
                    }
                }
                None if code <= 126 => {
                    return Err(format!("the file ends before character {code} is defined"));
                }
                // Some fonts leave out the German characters.
                None => break,
            }
        }

        // Code-tagged characters: a line with the code, then the glyph.
        while let Some(tag) = lines.next() {
            let Some(code) = tag.split_whitespace().next() else {
                continue;
            };
            let Some(glyph) = read_glyph(&mut lines) else {
                break;
            };
            if let Some(c) = parse_code(code).and_then(char::from_u32) {
                glyphs.insert(c, glyph);
            }
        }

        Ok(Self {
            name: name.to_string(),
            height,
            hardblank,
            mode,
            glyphs,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rows per line of text.
    pub fn height(&self) -> usize {
        self.height
    }

    pub fn has(&self, c: char) -> bool {
        self.glyphs.contains_key(&c)
    }

    /// Renders `text` on one line. Characters missing from the font are
    /// skipped; a non-breaking space draws as a space. Blank columns at
    /// both ends are trimmed.
    pub fn render<T: Copy>(&self, text: &[(char, T)]) -> Art<T> {
        let mut line = Line::new(self);
        for &(c, tag) in text {
            let glyph = match self.glyphs.get(&c) {
                None if c == '\u{a0}' => self.glyphs.get(&' '),
                glyph => glyph,
            };
            if let Some(glyph) = glyph {
                line.add(glyph, tag);
            }
        }
        line.finish()
    }

    /// Renders `text`, starting a new line at each `'\n'` and wherever the
    /// next word would make a line wider than `width` columns. A word wider
    /// than `width` is split between characters.
    pub fn render_wrapped<T: Copy>(&self, text: &[(char, T)], width: usize) -> Vec<Art<T>> {
        let mut lines = Vec::new();
        let paragraphs: Vec<&[(char, T)]> = text.split(|(c, _)| *c == '\n').collect();
        let count = paragraphs.len();
        for (i, paragraph) in paragraphs.into_iter().enumerate() {
            // A break at the very end does not start another line.
            if i + 1 == count && paragraph.is_empty() && count > 1 {
                break;
            }
            self.wrap_paragraph(paragraph, width, &mut lines);
        }
        lines
    }

    fn wrap_paragraph<T: Copy>(&self, text: &[(char, T)], width: usize, lines: &mut Vec<Art<T>>) {
        let fits = |candidate: &[(char, T)]| self.render(candidate).width() <= width;
        let mut current: Vec<(char, T)> = Vec::new();
        let mut start = 0;
        while start < text.len() {
            if text[start].0 == ' ' {
                start += 1;
                continue;
            }
            let end = text[start..]
                .iter()
                .position(|(c, _)| *c == ' ')
                .map_or(text.len(), |n| start + n);
            let word = &text[start..end];

            let mut candidate = current.clone();
            if !candidate.is_empty() {
                // Keep the space before the word, with its own tag.
                candidate.push(text[start - 1]);
            }
            candidate.extend_from_slice(word);
            if fits(&candidate) {
                current = candidate;
            } else {
                if !current.is_empty() {
                    lines.push(self.render(&current));
                    current.clear();
                }
                for &c in word {
                    current.push(c);
                    if current.len() > 1 && !fits(&current) {
                        current.pop();
                        lines.push(self.render(&current));
                        current = vec![c];
                    }
                }
            }
            start = end;
        }
        lines.push(self.render(&current));
    }
}

/// Removes trailing whitespace, then the endmark (the last character) and
/// any copies of it before it, like `figlet` does.
fn strip_endmarks(line: &str) -> &str {
    let line = line.trim_end();
    match line.chars().last() {
        Some(mark) => line.trim_end_matches(mark),
        None => line,
    }
}

/// A code tag: decimal, `0x` hexadecimal or `0` octal. Negative codes
/// (allowed by the format for unofficial characters) are skipped.
fn parse_code(code: &str) -> Option<u32> {
    if let Some(hex) = code.strip_prefix("0x").or_else(|| code.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else if code.len() > 1 && code.starts_with('0') {
        u32::from_str_radix(&code[1..], 8).ok()
    } else {
        code.parse().ok()
    }
}

/// A line being composed, one character at a time.
struct Line<'f, T> {
    font: &'f Font,
    rows: Vec<Vec<(char, Option<T>)>>,
    previous_width: usize,
}

impl<'f, T: Copy> Line<'f, T> {
    fn new(font: &'f Font) -> Self {
        Self {
            font,
            rows: vec![Vec::new(); font.height],
            previous_width: 0,
        }
    }

    fn add(&mut self, glyph: &Glyph, tag: T) {
        let overlap = self.overlap(glyph);
        for (line, glyph_row) in self.rows.iter_mut().zip(&glyph.rows) {
            let start = line.len().saturating_sub(overlap);
            let skipped = overlap.saturating_sub(line.len());
            for (k, &right) in glyph_row.iter().enumerate().take(overlap).skip(skipped) {
                let cell = &mut line[start + k - skipped];
                let merged = self
                    .font
                    .smush(cell.0, right, self.previous_width, glyph.width)
                    .unwrap_or(right);
                // The cell belongs to whichever character it now shows.
                if merged != cell.0 || right != ' ' && cell.0 == ' ' {
                    *cell = (merged, (right != ' ').then_some(tag));
                }
            }
            line.extend(
                glyph_row[overlap.min(glyph_row.len())..]
                    .iter()
                    .map(|&c| (c, (c != ' ').then_some(tag))),
            );
        }
        self.previous_width = glyph.width;
    }

    /// How many columns the next glyph can overlap the line, as `figlet` computes it.
    fn overlap(&self, glyph: &Glyph) -> usize {
        if self.font.mode & (rule::SMUSH | rule::KERN) == 0 {
            return 0;
        }
        let signed = |n: usize| isize::try_from(n).unwrap_or(isize::MAX);
        let mut most = signed(glyph.width);
        for (line, glyph_row) in self.rows.iter().zip(&glyph.rows) {
            // The line's last visible column and the glyph row's first one.
            let (last, left) = match line.iter().rposition(|(c, _)| *c != ' ') {
                Some(i) => (i, Some(line[i].0)),
                None => (0, None),
            };
            let first = glyph_row
                .iter()
                .position(|c| *c != ' ')
                .unwrap_or(glyph_row.len());
            let right = glyph_row.get(first).copied();
            let mut amount = signed(first) + signed(line.len()) - 1 - signed(last);
            match (left, right) {
                (None, _) => amount += 1,
                (Some(left), Some(right)) => {
                    if self
                        .font
                        .smush(left, right, self.previous_width, glyph.width)
                        .is_some()
                    {
                        amount += 1;
                    }
                }
                (Some(_), None) => {}
            }
            most = most.min(amount);
        }
        usize::try_from(most).unwrap_or(0)
    }

    fn finish(self) -> Art<T> {
        let hardblank = self.font.hardblank;
        let mut rows: Vec<Vec<Option<(char, T)>>> = self
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(c, tag)| match (c, tag) {
                        (' ', _) | (_, None) => None,
                        (c, Some(tag)) if c == hardblank => Some((' ', tag)),
                        (c, Some(tag)) => Some((c, tag)),
                    })
                    .collect()
            })
            .collect();
        let used = |column: usize| rows.iter().any(|row| row[column].is_some());
        let width = rows.first().map_or(0, Vec::len);
        let start = (0..width).find(|&c| used(c)).unwrap_or(width);
        let end = (0..width).rfind(|&c| used(c)).map_or(start, |c| c + 1);
        for row in &mut rows {
            row.truncate(end);
            row.drain(..start);
        }
        Art { rows }
    }
}

impl Font {
    /// The character shown when `left` and `right` overlap, or `None`
    /// when the font's rules do not allow them to.
    fn smush(
        &self,
        left: char,
        right: char,
        left_width: usize,
        right_width: usize,
    ) -> Option<char> {
        if left == ' ' {
            return Some(right);
        }
        if right == ' ' {
            return Some(left);
        }
        if left_width < 2 || right_width < 2 || self.mode & rule::SMUSH == 0 {
            return None;
        }
        let hardblank = self.hardblank;
        if self.mode & 63 == 0 {
            // Universal smushing: the later character wins, visible ones over hardblanks.
            return Some(if right == hardblank { left } else { right });
        }
        if self.mode & rule::HARDBLANK != 0 && left == hardblank && right == hardblank {
            return Some(left);
        }
        if left == hardblank || right == hardblank {
            return None;
        }
        if self.mode & rule::EQUAL != 0 && left == right {
            return Some(left);
        }
        if self.mode & rule::LOWLINE != 0 {
            const BORDERS: &str = "|/\\[]{}()<>";
            if left == '_' && BORDERS.contains(right) {
                return Some(right);
            }
            if right == '_' && BORDERS.contains(left) {
                return Some(left);
            }
        }
        if self.mode & rule::HIERARCHY != 0 {
            let class = |c: char| {
                ["|", "/\\", "[]", "{}", "()", "<>"]
                    .iter()
                    .position(|class| class.contains(c))
            };
            if let (Some(l), Some(r)) = (class(left), class(right))
                && l != r
            {
                return Some(if r > l { right } else { left });
            }
        }
        if self.mode & rule::PAIR != 0 {
            let pair = matches!(
                (left, right),
                ('[', ']') | (']', '[') | ('{', '}') | ('}', '{') | ('(', ')') | (')', '(')
            );
            if pair {
                return Some('|');
            }
        }
        if self.mode & rule::BIG_X != 0 {
            match (left, right) {
                ('/', '\\') => return Some('|'),
                ('\\', '/') => return Some('Y'),
                ('>', '<') => return Some('X'),
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font with 2-row glyphs for a few letters; every other printable
    /// character is blank and one column wide.
    fn font(old_layout: i32, full_layout: Option<i32>, glyphs: &[(char, [&str; 2])]) -> Font {
        let full = full_layout.map(|f| format!(" 0 {f}")).unwrap_or_default();
        let mut text = format!("flf2a$ 2 2 8 {old_layout} 1{full}\nA test font.\n");
        for code in 32..=126u8 {
            let c = char::from(code);
            match glyphs.iter().find(|(g, _)| *g == c) {
                Some((_, [top, bottom])) => text.push_str(&format!("{top}@\n{bottom}@@\n")),
                None => text.push_str(" @\n @@\n"),
            }
        }
        Font::parse("test", &text).unwrap()
    }

    fn show(art: &Art<()>) -> String {
        art.rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| cell.map_or('.', |(c, ())| c))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn plain(text: &str) -> Vec<(char, ())> {
        text.chars().map(|c| (c, ())).collect()
    }

    const BOXES: [(char, [&str; 2]); 3] = [
        ('o', ["/\\ ", "\\/ "]),
        ('x', ["|  |", "|__|"]),
        ('h', ["$|", "$|"]),
    ];

    #[test]
    fn full_width_keeps_every_column() {
        let f = font(-1, Some(0), &BOXES);
        assert_eq!(show(&f.render(&plain("oo"))), "/\\./\\\n\\/.\\/");
    }

    #[test]
    fn kerning_slides_until_touching() {
        let f = font(0, None, &BOXES);
        assert_eq!(show(&f.render(&plain("oo"))), "/\\/\\\n\\/\\/");
    }

    #[test]
    fn smushing_merges_by_rules() {
        // Equal characters merge: `|` + `|`.
        let f = font(1, None, &BOXES);
        assert_eq!(show(&f.render(&plain("xx"))), "|..|..|\n|__|__|");
        // Without the rule they only touch.
        let f = font(0, None, &BOXES);
        assert_eq!(show(&f.render(&plain("xx"))), "|..||..|\n|__||__|");
    }

    #[test]
    fn universal_smushing_prefers_the_later_character() {
        let f = font(0, Some(128), &[('a', ["ab", "cd"]), ('b', ["XY", "ZW"])]);
        assert_eq!(show(&f.render(&plain("ab"))), "aXY\ncZW");
    }

    #[test]
    fn hardblanks_draw_spaces_and_block_kerning() {
        let f = font(0, None, &BOXES);
        let art = f.render(&plain("hh"));
        assert_eq!(show(&art), " | |\n | |");
        assert_eq!(
            art.rows[0][2],
            Some((' ', ())),
            "a hardblank is a visible space"
        );
    }

    #[test]
    fn smush_rules() {
        let f = font(0, Some(128 | 63), &[]);
        let s = |l, r| f.smush(l, r, 2, 2);
        assert_eq!(s('_', '/'), Some('/'));
        assert_eq!(s('|', '['), Some('['));
        assert_eq!(s('{', ')'), Some(')'));
        assert_eq!(s('[', ']'), Some('|'));
        assert_eq!(s('/', '\\'), Some('|'));
        assert_eq!(s('\\', '/'), Some('Y'));
        assert_eq!(s('>', '<'), Some('X'));
        assert_eq!(s('$', '$'), Some('$'));
        assert_eq!(s('$', 'x'), None);
        assert_eq!(s('a', 'b'), None);
        assert_eq!(f.smush('|', '|', 1, 2), None, "narrow glyphs never smush");
    }

    #[test]
    fn tags_follow_the_character_drawn() {
        let f = font(-1, Some(0), &BOXES);
        let art = f.render(&[('o', 1), ('o', 2)]);
        assert_eq!(art.rows[0][0], Some(('/', 1)));
        assert_eq!(art.rows[0][3], Some(('/', 2)));
    }

    #[test]
    fn wraps_at_spaces_and_breaks() {
        let f = font(-1, Some(0), &[('a', ["aa", "aa"]), ('b', ["b", "b"])]);
        let lines = f.render_wrapped(&plain("aa b aa\nb\n"), 6);
        let shown: Vec<String> = lines.iter().map(show).collect();
        assert_eq!(shown, ["aaaa.b\naaaa.b", "aaaa\naaaa", "b\nb"]);
    }

    #[test]
    fn non_breaking_spaces_keep_words_together() {
        let f = font(-1, Some(0), &[('a', ["aa", "aa"]), (' ', ["$", "$"])]);
        let lines = f.render_wrapped(&plain("a\u{a0}a a"), 6);
        assert_eq!(lines.iter().map(Art::width).collect::<Vec<_>>(), [5, 2]);
    }

    #[test]
    fn long_words_split() {
        let f = font(-1, Some(0), &[('a', ["aa", "aa"])]);
        let lines = f.render_wrapped(&plain("aaa"), 5);
        assert_eq!(lines.iter().map(Art::width).collect::<Vec<_>>(), [4, 2]);
    }

    #[test]
    fn parses_code_tags_and_rejects_bad_files() {
        let mut text = String::from("flf2a$ 1 1 4 -1 0\n");
        for _ in 32..=126 {
            text.push_str("x@@\n");
        }
        let ascii_only = Font::parse("t", &text).unwrap();
        assert!(!ascii_only.has('ä'), "German characters may be left out");

        for _ in DEUTSCH {
            text.push_str("d@@\n");
        }
        text.push_str("0x2022  BULLET\no@@\n-5 unofficial\nz@@\n");
        let f = Font::parse("t", &text).unwrap();
        assert!(f.has('ä'));
        assert!(f.has('•'), "hex code tag");
        assert_eq!(f.glyphs.len(), 95 + 7 + 1, "negative codes are skipped");
        assert_eq!(parse_code("0101"), Some(65));

        assert!(Font::parse("t", "hello").is_err());
        assert!(Font::parse("t", "flf2a$ 1 1 4 -1 0\nx@@\n").is_err());
    }

    #[test]
    fn endmarks_are_stripped() {
        assert_eq!(strip_endmarks("  /\\ @@  "), "  /\\ ");
        assert_eq!(strip_endmarks("##"), "");
        assert_eq!(strip_endmarks(""), "");
    }
}

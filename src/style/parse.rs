//! A small CSS tokenizer: stylesheets into rules, rule bodies into declarations.
//!
//! Selectors are kept as text here and compiled by `simplecss` later.
//! Anything malformed is skipped with a warning instead of failing.

/// One `selectors { declarations }` block.
#[derive(Debug, PartialEq)]
pub(super) struct RawRule {
    pub selectors: Vec<String>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Declaration {
    /// Lowercase, except custom properties (`--name`), which are case-sensitive.
    pub name: String,
    pub value: String,
    pub important: bool,
}

pub(super) fn stylesheet(text: &str, warnings: &mut Vec<String>) -> Vec<RawRule> {
    let text = strip_comments(text);
    let mut rules = Vec::new();
    let mut rest = text.as_str();

    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }

        // `@import …;` and other statement at-rules end at a semicolon.
        if rest.starts_with('@') {
            let semicolon = find_top_level(rest, ';');
            let brace = find_top_level(rest, '{');
            let name: String = rest
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '{' && *c != ';')
                .collect();
            warnings.push(format!("unsupported CSS rule `{name}` ignored"));
            match (semicolon, brace) {
                (Some(s), Some(b)) if s < b => rest = &rest[s + 1..],
                (Some(s), None) => rest = &rest[s + 1..],
                (_, Some(b)) => rest = skip_block(&rest[b..]),
                (None, None) => break,
            }
            continue;
        }

        let Some(open) = find_top_level(rest, '{') else {
            warnings.push(format!(
                "CSS text without a {{ block ignored: `{}`",
                rest.trim()
            ));
            break;
        };
        let prelude = &rest[..open];
        let after_open = &rest[open + 1..];
        let (body, next) = match find_top_level(after_open, '}') {
            Some(close) => (&after_open[..close], &after_open[close + 1..]),
            None => {
                warnings.push(format!(
                    "CSS rule `{}` is missing its closing }}",
                    prelude.trim()
                ));
                (after_open, "")
            }
        };
        rest = next;

        let selectors: Vec<String> = split_top_level(prelude, ',')
            .into_iter()
            .map(str::to_string)
            .collect();
        if selectors.is_empty() {
            warnings.push(format!(
                "CSS rule without a selector ignored: `{{{}}}`",
                body.trim()
            ));
            continue;
        }
        rules.push(RawRule {
            selectors,
            declarations: declarations(body, warnings),
        });
    }
    rules
}

/// Parses `name: value; name: value !important` (a rule body or a `style` attribute).
pub(super) fn declarations(text: &str, warnings: &mut Vec<String>) -> Vec<Declaration> {
    let mut out = Vec::new();
    for part in split_top_level(text, ';') {
        let Some((name, value)) = part.split_once(':') else {
            warnings.push(format!("malformed CSS declaration `{part}` ignored"));
            continue;
        };
        let name = name.trim();
        let mut value = value.trim();
        let mut important = false;
        if let Some(bang) = value.rfind('!')
            && value[bang + 1..].trim().eq_ignore_ascii_case("important")
        {
            important = true;
            value = value[..bang].trim_end();
        }
        if name.is_empty() || value.is_empty() {
            warnings.push(format!("malformed CSS declaration `{part}` ignored"));
            continue;
        }
        let name = if name.starts_with("--") {
            name.to_string()
        } else {
            name.to_ascii_lowercase()
        };
        out.push(Declaration {
            name,
            value: value.to_string(),
            important,
        });
    }
    out
}

/// Splits on `separator` outside quotes, parentheses and brackets; drops empty parts.
pub(super) fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    scan(text, |i, c| {
        if c == separator {
            parts.push(&text[start..i]);
            start = i + c.len_utf8();
        }
        false
    });
    parts.push(&text[start..]);
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Byte index of the first `target` outside quotes, parentheses and brackets.
fn find_top_level(text: &str, target: char) -> Option<usize> {
    let mut found = None;
    scan(text, |i, c| {
        if c == target {
            found = Some(i);
            true
        } else {
            false
        }
    });
    found
}

/// Calls `visit` for every character at nesting depth 0 and outside
/// quotes, until it returns `true`.
fn scan(text: &str, mut visit: impl FnMut(usize, char) -> bool) {
    let mut quote = None;
    let mut depth = 0u32;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth = depth.saturating_sub(1),
            (None, c) if depth == 0 && visit(i, c) => return,
            _ => {}
        }
    }
}

/// `text` starts at a `{`; returns what follows its matching `}`.
fn skip_block(text: &str) -> &str {
    let mut depth = 0u32;
    for (i, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return &text[i + 1..];
                }
            }
            _ => {}
        }
    }
    ""
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(name: &str, value: &str, important: bool) -> Declaration {
        Declaration {
            name: name.into(),
            value: value.into(),
            important,
        }
    }

    #[test]
    fn parses_rules_and_selector_lists() {
        let mut warnings = Vec::new();
        let rules = stylesheet(
            "/* comment */ h1, h2 { color: Yellow; MARGIN: 1 }\n.box{background:blue}",
            &mut warnings,
        );
        assert_eq!(
            rules,
            [
                RawRule {
                    selectors: vec!["h1".into(), "h2".into()],
                    declarations: vec![decl("color", "Yellow", false), decl("margin", "1", false)],
                },
                RawRule {
                    selectors: vec![".box".into()],
                    declarations: vec![decl("background", "blue", false)],
                },
            ]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn declarations_handle_important_quotes_and_custom_properties() {
        let mut warnings = Vec::new();
        assert_eq!(
            declarations(
                "color: red !important; --Fill: \"a;b\"; list-style-type: '► '",
                &mut warnings
            ),
            [
                decl("color", "red", true),
                decl("--Fill", "\"a;b\"", false),
                decl("list-style-type", "'► '", false),
            ]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn skips_at_rules_and_malformed_parts_with_warnings() {
        let mut warnings = Vec::new();
        let rules = stylesheet(
            "@import url(x.css); @media print { p { color: red } } p { color; : x; width: 2 } trailing",
            &mut warnings,
        );
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].declarations, [decl("width", "2", false)]);
        assert_eq!(warnings.len(), 5, "{warnings:?}");
    }

    #[test]
    fn unclosed_rule_still_applies() {
        let mut warnings = Vec::new();
        let rules = stylesheet("p { color: red", &mut warnings);
        assert_eq!(rules[0].declarations, [decl("color", "red", false)]);
        assert_eq!(warnings.len(), 1);
    }
}

//! The cascade: matching rules to elements and building the styled tree.
//!
//! Order, lowest to highest priority: the default stylesheet (`ua.css`), the
//! deck's `style.css`, the slide's `<style>` blocks, then `style=""`
//! attributes. Within one level, more specific selectors win, then later
//! rules. `!important` declarations beat all normal ones.

use scraper::{ElementRef, Html};
use simplecss::{AttributeOperator, PseudoClass, Selector};

use super::parse::{self, Declaration, RawRule};
use super::{ComputedStyle, Display, Rejected};

const DEFAULT_CSS: &str = include_str!("ua.css");

/// An element or text node with its computed style.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledNode {
    pub style: ComputedStyle,
    pub content: Content,
    pub children: Vec<StyledNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
    },
    Text(String),
}

impl StyledNode {
    pub fn tag(&self) -> Option<&str> {
        match &self.content {
            Content::Element { tag, .. } => Some(tag),
            Content::Text(_) => None,
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        match &self.content {
            Content::Element { attrs, .. } => attrs
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str()),
            Content::Text(_) => None,
        }
    }

    /// The first element with this tag, depth first (including `self`).
    pub fn find(&self, tag: &str) -> Option<&StyledNode> {
        if self.tag() == Some(tag) {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(tag))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    Default,
    /// HTML attributes that stand for CSS, like `<img width="20">`.
    Hint,
    Deck,
    Slide,
    Inline,
}

/// Cascade priority: (important, origin, specificity, order). Sorts lowest first.
type Priority = (bool, Origin, [u8; 3], usize);

struct Rule<'a> {
    selector: Selector<'a>,
    specificity: [u8; 3],
    origin: Origin,
    /// Position among all rules, for "later wins".
    order: usize,
    declarations: &'a [Declaration],
}

/// Styles the whole document, starting at `<html>`. Problems (unknown
/// properties, bad values, unsupported selectors) are added to `warnings`.
pub fn build(doc: &Html, deck_css: Option<&str>, warnings: &mut Vec<String>) -> StyledNode {
    let mut problems = Vec::new();

    let mut ignored = Vec::new();
    let mut sheets = vec![(
        Origin::Default,
        parse::stylesheet(DEFAULT_CSS, &mut ignored),
    )];
    debug_assert!(
        ignored.is_empty(),
        "default stylesheet has problems: {ignored:?}"
    );
    if let Some(css) = deck_css {
        sheets.push((Origin::Deck, parse::stylesheet(css, &mut problems)));
    }
    let style_selector = scraper::Selector::parse("style").expect("static selector");
    for style in doc.select(&style_selector) {
        let css: String = style.text().collect();
        sheets.push((Origin::Slide, parse::stylesheet(&css, &mut problems)));
    }

    let rules = compile(&sheets, &mut problems);
    let root = style_element(
        doc.root_element(),
        &ComputedStyle::root(),
        &rules,
        &mut problems,
    )
    .unwrap_or_else(|| StyledNode {
        style: ComputedStyle::root(),
        content: Content::Element {
            tag: "html".into(),
            attrs: Vec::new(),
        },
        children: Vec::new(),
    });

    for problem in problems {
        if !warnings.contains(&problem) {
            warnings.push(problem);
        }
    }
    root
}

fn compile<'a>(sheets: &'a [(Origin, Vec<RawRule>)], problems: &mut Vec<String>) -> Vec<Rule<'a>> {
    let mut rules = Vec::new();
    for (origin, raw_rules) in sheets {
        for raw in raw_rules {
            for text in &raw.selectors {
                match Selector::parse(text) {
                    Some(selector) => rules.push(Rule {
                        specificity: selector.specificity(),
                        selector,
                        origin: *origin,
                        order: rules.len(),
                        declarations: &raw.declarations,
                    }),
                    None => problems.push(format!("unsupported CSS selector `{text}` ignored")),
                }
            }
        }
    }
    rules
}

fn style_element(
    element: ElementRef<'_>,
    parent: &ComputedStyle,
    rules: &[Rule<'_>],
    problems: &mut Vec<String>,
) -> Option<StyledNode> {
    let inline = element
        .value()
        .attr("style")
        .map(|css| parse::declarations(css, problems))
        .unwrap_or_default();
    // `data-font="small"` works like `style="--font: small"`, which still beats it.
    let font_hint = element.value().attr("data-font").map(|font| Declaration {
        name: "--font".into(),
        value: font.into(),
        important: false,
    });
    // `width` and `height` attributes on pictures and tables, in cells.
    // Like in browsers, any CSS rule overrides them.
    let size_hints: Vec<Declaration> = if matches!(element.value().name(), "img" | "table") {
        ["width", "height"]
            .into_iter()
            .filter_map(|name| {
                let value = element.value().attr(name)?;
                Some(Declaration {
                    name: name.into(),
                    value: value.into(),
                    important: false,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut matched: Vec<(Priority, &Declaration)> = size_hints
        .iter()
        .map(|hint| ((false, Origin::Hint, [0; 3], 0), hint))
        .collect();
    let target = El(element);
    for rule in rules {
        if rule.selector.matches(&target) {
            for decl in rule.declarations {
                matched.push((
                    (decl.important, rule.origin, rule.specificity, rule.order),
                    decl,
                ));
            }
        }
    }
    if let Some(hint) = &font_hint {
        matched.push(((false, Origin::Inline, [0; 3], usize::MAX - 1), hint));
    }
    for decl in &inline {
        matched.push(((decl.important, Origin::Inline, [0; 3], usize::MAX), decl));
    }
    matched.sort_by_key(|(key, _)| *key);

    let mut style = parent.inherit();
    for (_, decl) in matched {
        match style.apply(&decl.name, &decl.value) {
            Ok(()) => {}
            Err(Rejected::UnknownProperty) => {
                problems.push(format!("unknown CSS property `{}` ignored", decl.name));
            }
            Err(Rejected::InvalidValue) => problems.push(format!(
                "invalid value `{}` for CSS property `{}` ignored",
                decl.value, decl.name
            )),
        }
    }
    if style.display == Display::None {
        return None;
    }

    let mut children = Vec::new();
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            children.push(StyledNode {
                style: style.inherit(),
                content: Content::Text(text.to_string()),
                children: Vec::new(),
            });
        } else if let Some(child) = ElementRef::wrap(child)
            && let Some(node) = style_element(child, &style, rules, problems)
        {
            children.push(node);
        }
    }

    let value = element.value();
    Some(StyledNode {
        style,
        content: Content::Element {
            tag: value.name().to_string(),
            attrs: value
                .attrs()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
        },
        children,
    })
}

/// Lets `simplecss` selectors match `scraper` elements.
#[derive(Clone, Copy)]
struct El<'a>(ElementRef<'a>);

impl simplecss::Element for El<'_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent().and_then(ElementRef::wrap).map(El)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_siblings().find_map(ElementRef::wrap).map(El)
    }

    fn has_local_name(&self, name: &str) -> bool {
        self.0.value().name().eq_ignore_ascii_case(name)
    }

    fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> bool {
        self.0
            .value()
            .attr(local_name)
            .is_some_and(|value| operator.matches(value))
    }

    fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> bool {
        match class {
            PseudoClass::FirstChild => self.prev_sibling_element().is_none(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Ega;
    use crate::style::{Length, ListStyleType, WhiteSpace};

    fn styled(html: &str, deck_css: Option<&str>) -> (StyledNode, Vec<String>) {
        let mut warnings = Vec::new();
        let root = build(&Html::parse_document(html), deck_css, &mut warnings);
        (root, warnings)
    }

    fn color_of(root: &StyledNode, tag: &str) -> Ega {
        root.find(tag).unwrap().style.color
    }

    #[test]
    fn size_attributes_are_hints() {
        let (root, warnings) = styled(
            "<style>.wide { width: 30 }</style>
             <img src='a.png' width='12' height='5'>
             <img class='wide' src='b.png' width='12'>
             <p width='9'>not a picture</p>",
            None,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let body = root.find("body").unwrap();
        let images: Vec<&StyledNode> = body
            .children
            .iter()
            .filter(|c| c.tag() == Some("img"))
            .collect();
        assert_eq!(images[0].style.width, Some(Length::Cells(12)));
        assert_eq!(images[0].style.height, Some(5));
        assert_eq!(
            images[1].style.width,
            Some(Length::Cells(30)),
            "CSS beats the attribute"
        );
        assert_eq!(body.find("p").unwrap().style.width, None);
    }

    #[test]
    fn default_stylesheet_is_clean() {
        let mut warnings = Vec::new();
        let sheets = [(
            Origin::Default,
            parse::stylesheet(DEFAULT_CSS, &mut warnings),
        )];
        let rules = compile(&sheets, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(rules.len() > 50);

        let (root, warnings) = styled("<h1>x</h1><ul><li>a</li></ul><pre>p</pre>", None);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            root.find("body").unwrap().style.background,
            Some(Ega::Black)
        );
        assert_eq!(root.find("li").unwrap().style.display, Display::ListItem);
        assert_eq!(root.find("pre").unwrap().style.white_space, WhiteSpace::Pre);
    }

    #[test]
    fn head_and_scripts_are_dropped() {
        let (root, _) = styled("<title>t</title><script>x</script><p>y</p>", None);
        assert!(root.find("head").is_none());
        assert!(root.find("script").is_none());
        assert!(root.find("p").is_some());
    }

    #[test]
    fn specificity_then_order() {
        let (root, _) = styled(
            "<style>
               #x { color: red }
               p.a { color: green }
               p { color: blue }
               .a { color: cyan }
               p { color: magenta }
             </style>
             <p id=x class=a>1</p><p class=a>2</p><p>3</p>",
            None,
        );
        let body = root.find("body").unwrap();
        let colors: Vec<Ega> = body
            .children
            .iter()
            .filter(|c| c.tag() == Some("p"))
            .map(|p| p.style.color)
            .collect();
        assert_eq!(colors, [Ega::Red, Ega::Green, Ega::Magenta]);
    }

    #[test]
    fn origins_inline_and_important() {
        let deck = "p { color: red } h2 { color: green !important }";
        let (root, _) = styled(
            "<style>p { color: blue } h2 { color: white }</style>
             <p>a</p><h2 style='color: cyan'>b</h2><h3 style='color: brown'>c</h3>",
            Some(deck),
        );
        assert_eq!(color_of(&root, "p"), Ega::Blue, "slide beats deck");
        assert_eq!(color_of(&root, "h2"), Ega::Green, "!important beats inline");
        assert_eq!(color_of(&root, "h3"), Ega::Brown, "inline beats default");
    }

    #[test]
    fn deck_css_overrides_defaults() {
        let (root, _) = styled("<h2>x</h2>", Some("h2 { color: lightred }"));
        assert_eq!(color_of(&root, "h2"), Ega::LightRed);
    }

    #[test]
    fn inheritance_and_descendant_selectors() {
        let (root, _) = styled(
            "<style>.box { color: white } .box em { color: red } li:first-child { color: green }</style>
             <div class='box'><p>a <em>b</em></p></div>
             <ul><li>one</li><li>two</li></ul>",
            None,
        );
        assert_eq!(color_of(&root, "p"), Ega::White);
        assert_eq!(color_of(&root, "em"), Ega::Red);
        let ul = root.find("ul").unwrap();
        assert_eq!(ul.children[0].style.color, Ega::Green);
        assert_eq!(ul.children[1].style.color, Ega::LightGray);
        assert_eq!(ul.children[1].style.list_style_type, ListStyleType::Disc);
    }

    #[test]
    fn problems_become_warnings_once() {
        let (_, warnings) = styled(
            "<style>p { font-size: 20px; color: nope } p::marker { color: red }</style><p>a</p><p>b</p>",
            None,
        );
        assert_eq!(
            warnings,
            [
                "unsupported CSS selector `p::marker` ignored",
                "unknown CSS property `font-size` ignored",
                "invalid value `nope` for CSS property `color` ignored",
            ]
        );
    }
}

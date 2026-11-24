//! Animations: build steps, text effects and frame animation, plus slide
//! transitions in [`transition`].
//!
//! A slide is laid out once. Every display item carries the build step it
//! appears at and the innermost animation it belongs to; painting then
//! depends only on a [`Clock`]: the current step and how long the slide
//! and the step have been showing.
//!
//! - `data-step` shows an element (and what is inside) from that step on.
//!   A number picks the step; an empty value means the step after the
//!   highest one so far, so steps can simply follow document order.
//! - `data-build` gives each child element its own next step (rows, for
//!   a table). `PgDn` shows the next step before moving on.
//! - `data-effect="typewriter | marquee | rainbow"` animates an element's
//!   text; `text-decoration: blink` blinks it.
//! - `data-frames` shows its child elements one at a time, `data-fps`
//!   (default 4) times a second, all in the same place.

pub mod transition;

use std::time::Duration;

use crate::style::StyledNode;

/// Characters a typewriter types per second.
pub const TYPEWRITER_CPS: f64 = 30.0;
/// Cells a marquee scrolls per second.
pub const MARQUEE_SPEED: f64 = 10.0;
/// Columns the rainbow moves per second.
pub const RAINBOW_SPEED: f64 = 8.0;
/// How long blinking text stays on, then off: the EGA rate.
pub const BLINK_HALF_PERIOD: Duration = Duration::from_millis(267);
/// Frames per second of `data-frames` without `data-fps`.
pub const DEFAULT_FPS: f64 = 4.0;

/// The build steps and animations of one slide.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    /// The last build step; 0 when the slide shows all at once.
    pub steps: u32,
    pub animations: Vec<Animation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub effect: Effect,
    /// The step its element appears at; a typewriter starts then.
    pub step: u32,
    /// The animation of the enclosing element, which applies too.
    pub parent: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// Text appears a character at a time.
    Typewriter,
    /// Text scrolls left through the element's box, round and round.
    Marquee,
    /// Text cycles through the bright colors.
    Rainbow,
    /// One of the children of a `data-frames` element.
    Frame { index: u32, count: u32, fps: f64 },
}

impl Effect {
    pub const NAMES: [&str; 3] = ["typewriter", "marquee", "rainbow"];

    pub fn parse(name: &str) -> Option<Effect> {
        match name.trim().to_ascii_lowercase().as_str() {
            "typewriter" => Some(Effect::Typewriter),
            "marquee" => Some(Effect::Marquee),
            "rainbow" => Some(Effect::Rainbow),
            _ => None,
        }
    }
}

impl Timeline {
    /// The animation `id` and the ones enclosing it, innermost first.
    pub fn chain(&self, id: Option<u32>) -> impl Iterator<Item = (u32, &Animation)> {
        let mut next = id;
        std::iter::from_fn(move || {
            let id = next?;
            let animation = self.animations.get(usize::try_from(id).ok()?)?;
            next = animation.parent;
            Some((id, animation))
        })
    }

    /// How often something visible at `clock` changes, ignoring
    /// typewriters (their length is in the display list) and blinking.
    pub fn tick(&self, clock: &Clock) -> Option<Duration> {
        self.animations
            .iter()
            .filter(|animation| animation.step <= clock.step)
            .filter_map(|animation| match animation.effect {
                Effect::Typewriter => None,
                Effect::Marquee => Some(1.0 / MARQUEE_SPEED),
                Effect::Rainbow => Some(1.0 / RAINBOW_SPEED),
                Effect::Frame { fps, count, .. } => (count > 1).then_some(1.0 / fps),
            })
            .map(Duration::from_secs_f64)
            .min()
    }
}

/// What painting depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    /// Elements with a higher step are hidden.
    pub step: u32,
    /// Time since the slide appeared: drives marquees, rainbows, frames, blinking.
    pub time: Duration,
    /// Time since the current step began: drives its typewriters.
    pub step_time: Duration,
    /// `false` after going back: typewriters show their whole text.
    pub forward: bool,
}

impl Clock {
    /// Every step shown and every effect at rest: how a slide prints.
    pub fn still(steps: u32) -> Self {
        Self {
            step: steps,
            time: Duration::ZERO,
            step_time: Duration::ZERO,
            forward: false,
        }
    }

    /// Characters a typewriter that starts with `step` has typed so far.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn typed(&self, step: u32) -> usize {
        if step == self.step && self.forward {
            (self.step_time.as_secs_f64() * TYPEWRITER_CPS) as usize
        } else if step < self.step || !self.forward {
            usize::MAX
        } else {
            0
        }
    }

    /// The frame showing among `count`, at `fps` frames per second.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn frame(&self, count: u32, fps: f64) -> u32 {
        let frames = (self.time.as_secs_f64() * fps) as u64;
        u32::try_from(frames % u64::from(count.max(1))).unwrap_or(0)
    }

    /// Cells a marquee has scrolled.
    #[allow(clippy::cast_possible_truncation)]
    pub fn scrolled(&self) -> i32 {
        (self.time.as_secs_f64() * MARQUEE_SPEED) as i32
    }

    /// Columns the rainbow has moved.
    #[allow(clippy::cast_possible_truncation)]
    pub fn rainbow_phase(&self) -> i32 {
        (self.time.as_secs_f64() * RAINBOW_SPEED) as i32
    }

    /// Whether blinking text shows now.
    pub fn blink_on(&self) -> bool {
        (self.time.as_millis() / BLINK_HALF_PERIOD.as_millis()).is_multiple_of(2)
    }
}

/// Sets the build step and animations of every node under `root` and
/// returns the slide's timeline. Bad values become `warnings`.
pub fn prepare(root: &mut StyledNode, warnings: &mut Vec<String>) -> Timeline {
    let mut walker = Walker {
        timeline: Timeline::default(),
        warnings,
    };
    walker.visit(
        root,
        Inherited {
            step: 0,
            anim: None,
            build_item: false,
            build_rows: false,
        },
    );
    walker.timeline
}

#[derive(Clone, Copy)]
struct Inherited {
    step: u32,
    anim: Option<u32>,
    /// The parent has `data-build`: this element gets the next step.
    build_item: bool,
    /// A `<thead>`, `<tbody>` or `<tfoot>` of a building table: its rows
    /// are build items.
    build_rows: bool,
}

struct Walker<'w> {
    timeline: Timeline,
    warnings: &'w mut Vec<String>,
}

impl Walker<'_> {
    fn warn(&mut self, warning: String) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    fn add(&mut self, effect: Effect, step: u32, parent: Option<u32>) -> u32 {
        let id = u32::try_from(self.timeline.animations.len()).unwrap_or(u32::MAX);
        self.timeline.animations.push(Animation {
            effect,
            step,
            parent,
        });
        id
    }

    fn visit(&mut self, node: &mut StyledNode, inherited: Inherited) {
        let is_element = node.tag().is_some();
        let next = self.timeline.steps + 1;
        let own_step = match node.attr("data-step").map(str::trim) {
            Some("") => Some(next),
            Some(value) => match value.parse::<u32>() {
                Ok(step) => Some(step),
                Err(_) => {
                    let warning = format!("data-step `{value}` is not a step number");
                    self.warn(warning);
                    None
                }
            },
            None => (is_element && inherited.build_item).then_some(next),
        };
        let step = own_step.map_or(inherited.step, |own| own.max(inherited.step));
        self.timeline.steps = self.timeline.steps.max(step);

        let box_anim = inherited.anim;
        let mut anim = box_anim;
        if let Some(name) = node.attr("data-effect").map(str::to_string) {
            match Effect::parse(&name) {
                Some(effect) => anim = Some(self.add(effect, step, anim)),
                None => self.warn(format!(
                    "unknown data-effect `{}` ignored (effects: {})",
                    name.trim(),
                    Effect::NAMES.join(", ")
                )),
            }
        }

        let tag = node.tag().map(str::to_string);
        let build = node.attr("data-build").is_some();
        let build_rows = build && tag.as_deref() == Some("table");
        let fps = match node.attr("data-frames").map(|_| node.attr("data-fps")) {
            None => None,
            Some(None) => Some(DEFAULT_FPS),
            Some(Some(value)) => match value.trim().parse::<f64>() {
                Ok(fps) if fps > 0.0 && fps.is_finite() => Some(fps),
                _ => {
                    self.warn(format!(
                        "data-fps `{}` is not a positive number",
                        value.trim()
                    ));
                    Some(DEFAULT_FPS)
                }
            },
        };

        node.style.step = step;
        node.style.anim = anim;
        node.style.box_anim = box_anim;

        let count = node.children.iter().filter(|c| c.tag().is_some()).count();
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let mut index = 0;
        for child in &mut node.children {
            let child_tag = child.tag();
            let is_section = matches!(child_tag, Some("thead" | "tbody" | "tfoot"));
            // In a building table the rows are the items, in or out of sections.
            let build_item = if build_rows || inherited.build_rows {
                child_tag == Some("tr")
            } else {
                build && child_tag.is_some_and(|tag| tag != "caption")
            };
            let mut child_anim = anim;
            if let (Some(fps), true) = (fps, child_tag.is_some()) {
                let frame = Effect::Frame { index, count, fps };
                child_anim = Some(self.add(frame, step, anim));
                index += 1;
            }
            self.visit(
                child,
                Inherited {
                    step,
                    anim: child_anim,
                    build_item,
                    build_rows: build_rows && is_section,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    fn find<'a>(node: &'a StyledNode, id: &str) -> &'a StyledNode {
        fn walk<'a>(node: &'a StyledNode, id: &str) -> Option<&'a StyledNode> {
            if node.attr("id") == Some(id) {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, id))
        }
        walk(node, id).unwrap_or_else(|| panic!("no #{id}"))
    }

    #[test]
    fn steps_follow_document_order() {
        let slide = html::parse(
            "<p id=a>always</p>
             <p id=b data-step>first</p>
             <ul id=list data-build><li id=c>one</li><li id=d>two <b id=e>bold</b></li></ul>
             <p id=f data-step='2'>with one</p>
             <p id=g data-step>last</p>",
            None,
        );
        assert!(slide.warnings.is_empty(), "{:?}", slide.warnings);
        let step = |id| find(&slide.root, id).style.step;
        assert_eq!(
            [
                step("a"),
                step("b"),
                step("list"),
                step("c"),
                step("d"),
                step("e")
            ],
            [0, 1, 0, 2, 3, 3]
        );
        assert_eq!((step("f"), step("g")), (2, 4));
        assert_eq!(slide.timeline.steps, 4);
    }

    #[test]
    fn nested_steps_never_come_earlier() {
        let slide = html::parse(
            "<div id=a data-step='3'><p id=b data-step='1'>x</p></div>",
            None,
        );
        assert_eq!(find(&slide.root, "b").style.step, 3);
    }

    #[test]
    fn building_tables_reveal_rows() {
        let slide = html::parse(
            "<table data-build><caption id=cap>c</caption>
               <thead><tr id=h><th>h</th></tr></thead>
               <tbody><tr id=r1><td>1</td></tr><tr id=r2><td>2</td></tr></tbody></table>",
            None,
        );
        let step = |id| find(&slide.root, id).style.step;
        assert_eq!(
            [step("cap"), step("h"), step("r1"), step("r2")],
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn effects_chain_and_frames_take_turns() {
        let slide = html::parse(
            "<div id=box data-effect='rainbow'><p id=p data-effect='typewriter'>hi</p></div>
             <div id=frames data-frames data-fps='2'><pre id=f0>a</pre><pre id=f1>b</pre></div>",
            None,
        );
        assert!(slide.warnings.is_empty(), "{:?}", slide.warnings);
        let timeline = &slide.timeline;
        let p = &find(&slide.root, "p").style;
        let effects: Vec<Effect> = timeline.chain(p.anim).map(|(_, a)| a.effect).collect();
        assert_eq!(effects, [Effect::Typewriter, Effect::Rainbow]);
        assert_eq!(p.box_anim, find(&slide.root, "box").style.anim);

        let f1 = &find(&slide.root, "f1").style;
        let (_, frame) = timeline.chain(f1.box_anim).next().unwrap();
        assert_eq!(
            frame.effect,
            Effect::Frame {
                index: 1,
                count: 2,
                fps: 2.0
            }
        );
        assert_eq!(find(&slide.root, "frames").style.anim, None);
    }

    #[test]
    fn bad_values_warn() {
        let slide = html::parse(
            "<p data-step='soon'>a</p><p data-effect='sparkle'>b</p>
             <div data-frames data-fps='0'><p>c</p></div>",
            None,
        );
        assert_eq!(
            slide.warnings,
            [
                "data-step `soon` is not a step number",
                "unknown data-effect `sparkle` ignored (effects: typewriter, marquee, rainbow)",
                "data-fps `0` is not a positive number",
            ]
        );
    }

    #[test]
    fn clock() {
        let at = |step, ms, forward| Clock {
            step,
            time: Duration::from_millis(ms),
            step_time: Duration::from_millis(ms),
            forward,
        };
        assert_eq!(at(1, 100, true).typed(1), 3);
        assert_eq!(
            at(1, 100, true).typed(0),
            usize::MAX,
            "earlier steps are done"
        );
        assert_eq!(
            at(1, 100, false).typed(1),
            usize::MAX,
            "going back shows all"
        );
        assert_eq!(at(0, 100, true).typed(1), 0);
        assert_eq!(at(0, 1250, true).frame(3, 4.0), 2);
        assert!(at(0, 0, true).blink_on());
        assert!(!at(0, 300, true).blink_on());
        assert_eq!(at(0, 1000, true).scrolled(), 10);
    }
}

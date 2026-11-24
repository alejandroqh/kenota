---
name: kenota
description: Create, edit and check kenota presentations — DOS text-mode slide decks in the look of the 1988 Microsoft Works tutorials (EGA 16 colors, ░▒▓ dithered fields, box-drawing frames, black drop shadows, a PgUp/PgDn footer), written as one HTML file per slide in a deck folder. Use this whenever the user wants slides, a deck, a talk, a lesson or a demo made with kenota, asks for a retro, DOS, ANSI, BBS, EGA or terminal-style presentation, or works in a folder holding a deck.toml or numbered .html slides, even if they never say "kenota". It covers the Works visual style, every supported component, layout balance on the 80×25 grid, builds, transitions and text effects, and a script to see each slide as text.
---

# kenota decks

kenota plays a folder of HTML files as a full-screen DOS presentation in
the terminal. Each slide is drawn on a fixed grid of character cells (80×25 by
default) with the 16 EGA colors. Nothing is in pixels: every size, gap and
position is a number of cells, so a slide is closer to a drawing on graph
paper than a web page. Thinking in cells is the key to good slides here.

The look to aim for is the Microsoft Works for DOS tutorial: a black band of
narration, a dithered blue field, light grey panels with black shadows, bold
white labels, and pictures built from blocks. `references/works-style.md`
describes it in detail.

## Workflow

1. **Plan the story.** One idea per slide, the way the Works tutorials do it:
   a short narration (2–5 lines) and one visual that shows it. Write the
   slide list first (file name, title, the one thing it shows).
2. **Set up the deck folder.** Copy `assets/starter-deck/style.css` (the Works
   kit: `.field`, `.band`, `.panel`, `.dialog`, `.grid`/`.card`, `.sky`) and
   `deck.toml` into the new folder. Name slides `01-intro.html`,
   `02-....html`; they play in natural filename order. The starter slides are
   worked examples of the four main layouts: open them when building a
   similar slide.
3. **Sketch each slide in cells** before writing HTML: decide where the band,
   field and boxes go, and add up the rows and columns (see *Layout and
   balance*). Put the budget in a CSS comment, like the starter slides do, so
   later edits keep it.
4. **Write the HTML**, using `references/components.md` for exact syntax.
   Only the documented subset works; the viewer ignores anything else
   silently.
5. **Check, then look.**
   - Run `scripts/check.sh <deck>` and fix every problem it lists: ignored
     CSS, unsupported tags, missing pictures and fonts, bad transitions and
     chart values, content off the slide area.
   - Then preview every slide with `scripts/preview.sh` and fix what you see
     (next section). `check` can't see a line that wrapped where you didn't
     want it, or lopsided margins, so never hand over a deck you haven't
     looked at.
6. **Hand over** with the command to run it: `kenota <deck>` (or
   `cargo run -- <deck>` inside the kenota repository). Keys: PgDn/→/Space
   next step or slide, PgUp/← back, Home/End, q quits. `--slide N` starts at
   slide N.

## Seeing a slide

You can't watch the interactive viewer, so `scripts/preview.sh` prints a
slide through `kenota render`:

```bash
scripts/preview.sh <deck> 3                 # slide 3, all steps showing, as a numbered text grid
scripts/preview.sh <deck> 3 --map           # + background and foreground colors of every cell
scripts/preview.sh <deck> 3 --grid bg --rows 5-12   # only the background colors of rows 5-12
scripts/preview.sh <deck> 3 --step 0        # how the slide first appears (before any PgDn)
scripts/preview.sh <deck> 3 --step 2 --time 0.5     # half a second after step 2 appeared
```

- **Numbers:** rows and columns are canvas cells, the same numbers as CSS
  `top`/`left`. The footer is rows 23–24.
- **Colors:** `--map` and `--grid bg|fg` print each cell's color as an EGA
  number in hex (1 blue, 7 lightgray, f white…). In the text grid, a grey
  panel, a black screen and a brown desk all look like blank space, so use
  `--grid bg` on every picture scene.
- **Moments:**
  - without `--step`, every step shows;
  - without `--time`, typing has finished and the other effects are at rest:
    blinking text on, marquees at the start, the first frame;
  - so the same command always gives the same picture.

To see a slide as a picture, exactly as it looks on screen, save a PNG and
open it (your file-reading tool shows images):
`scripts/preview.sh <deck> 3 --png /tmp/slide-3.png`. Do this for picture
scenes and anything where colors and shapes matter more than text.

The engine also runs directly: `kenota render <deck> [--slide N]
[--step N] [--time S] [--colors none|truecolor|ansi16]` prints slides (every
slide when `--slide` is left out), and `kenota check <deck>` lists
problems. Check each slide for:

- text that wrapped where you didn't want it (a box narrower than you thought);
- anything cut off at the bottom: the slide area ends at row 22 when the footer is on;
- margins that differ left and right once shadows are counted;
- each build step (`--step 0`, `1`, …) as well as the finished slide; a
  `--step` past the last one shows the finished slide;
- typewriters part-way (`--time`), to see that the order of appearance makes
  sense.

The scripts find the engine with `scripts/engine.sh`: `$KENOTA`, then
the repository this skill lives in (building it with cargo), then `PATH`.

## The Works look in brief

- **Base colors do the heavy lifting**:
  - black narration band;
  - blue field with `--fill: medium lightblue` (▒);
  - lightgray panels with black text;
  - white bold labels;
  - black `box-shadow: 2 1`.
- **Accents are few and flat:** blue, magenta and cyan for data and pictures,
  yellow or white for signs and highlights, red sparingly. A slide with more
  than three accent colors starts to look like a ransom note.
- **Narration:**
  - plain second person, present tense, task-first ("You run the bakery.
    Every Friday you send the order. In this lesson you'll…");
  - 40–50 characters per line, no more than five lines;
  - goes in the top band, or in a panel at the top right of a picture.
- **Big FIGlet letters only for titles.** Body text stays plain, and there
  are no italics in DOS: `em` is yellow and bold is the bright version of the
  color.
- **Pictures are built from boxes:**
  - absolutely positioned `div`s;
  - each filled with a pattern character (`▌` stripes, `♦` trim, `▀` waves,
    ░▒▓ shading);
  - or `<img>` PNGs reduced to 16 colors.
- **The footer:** the engine draws it, bottom right, with the slide `<title>`
  in cyan and `◄◄◄ PgUp • n/N • PgDn ►►►`. Give every slide a short title
  (2–3 words); it is the "chapter name" of the screen.

## Layout and balance

The numbers that matter:

| | Cells |
|---|---|
| Screen | 80×25 (`size = "80x43"` or `"80x50"` in deck.toml for taller decks) |
| Slide area with the footer | 80×23, rows 0–22 (`footer = false` gives all 25) |
| Default body margin | `1 2 0 2`, so 76 columns from row 1; the starter kit sets `body { margin: 0 }` for full-bleed bands and fields |
| A box's `width`/`height` | include its border and padding: text width = width − 2 (border) − left/right padding |
| `box-shadow: 2 1` | 2 extra columns on the right and 1 extra row below, outside the box |
| Big text | `block` 4 rows × 6 cols/letter (`<h1>`), `small` 3 × 4 (capitals only), `big` 7 × 12 |
| Chart `height` | the whole chart: caption + plot + axis row + labels row + legend (a gap and one row per line of entries); without it the plot alone is 10 rows |
| Chart plot width | the chart's width − (widest value label + 2); it is split evenly between the categories |

Balance rules, and why:

- **Count shadows as part of the box.** A shadowed box looks centered only
  when the right margin is the left margin + 2. For a row of shadowed cards,
  make the gap at least 3 (2 for the shadow, 1 or more of field showing).
- **Pick sizes that divide evenly.** With n cards: inner width = n × card + (n−1) × gap.
  - 4 × 15 + 3 × 4 = 72 inside `padding: 2 5 2 3`: the field shows 3 columns
    on both sides, counting the shadow.
  - Uneven splits make cards differ by a column, which shows in a grid.
- **Share edges.** Start the band text, the panels and the field content on
  the same column (3 is a good default). Eyes notice a 1-cell misalignment
  on a coarse grid.
- **Vertical rhythm:** budget rows top-down.
  - The rows: band (4) + field padding (2) + boxes + shadow (1) + bottom
    padding (2) ≤ 23.
  - Leave at least one free row above the footer so the slide doesn't touch it.
  - Boxes in a flex row stretch to the tallest one; they don't fill the field
    down to the bottom. The tallest item (often a chart, sized by its
    `height`) sets the row's height, so budget from it.
- **Breathing room inside boxes.** Use `padding: 1 2` (one row, two
  columns): a cell is twice as tall as wide, so this looks even.
- **Weight:**
  - large areas in calm colors (blue, black, grey), small areas bright
    (white labels, yellow signs);
  - one focal element per slide: a big chart, a picture or a dialog, not three
    competing ones.
- **Text on solid ground.** Text placed straight on a ▒ fill gets a solid
  strip behind each line, which looks ragged. Put it in a band or panel, or
  give it a padded `background`, so the edge looks deliberate.
- **Fill the field, don't crowd it.** Works screens are about one-third
  empty field. If a slide needs more than ~12 lines of text, split it.

## Animation in brief

Full guide: `references/animation.md`. The short version:

- **Builds:** `data-step` on any element (empty = the next step), `data-build`
  on a list or table to reveal its items or rows. PgDn shows the next step
  before moving on. Hidden steps keep their space, so design the final state
  first and then choose the reveal order.
- **Transitions:**
  - set one quiet default in `deck.toml` (`transition = "dissolve"`, or a wipe);
  - override per slide with `<meta name="transition" content="push">` for a
    change of section or scene;
  - the choices are `cut`, `wipe-right|left|down|up`, `dissolve`, `blinds`,
    `push`, with an optional duration (`"push 0.8s"`).
- **Text effects:** `data-effect` takes one of the following, one per element
  (nest elements to combine):
  - `typewriter` for narration (30 characters a second);
  - `marquee` for a one-line ticker;
  - `rainbow` for a celebration.
  - Blinking is not a `data-effect`: use `text-decoration: blink`, for the one
    thing the viewer must not miss ("Press PgDn").
- **Frame animation:** `<div data-frames data-fps="6">` shows its child
  elements in turn, in the same place: spinners, progress bars, a walking
  figure.
- **Restraint is the style:**
  - at most one moving thing on a static slide;
  - builds on slides where order matters;
  - typewriter only on narration of two or three lines (each line takes 1.5 s).

## Gotchas

These came up in practice:

- **`<pre>` inherits `text-align`.** In a centered box, each line is centered
  on its own and trailing spaces are dropped, which shears block art. Put
  `text-align: left` on art inside centered boxes, and give the `pre` a
  `width` with `margin: 0 auto` to center the art as a whole.
- **Selectors:** tag, `.class`, `#id`, descendant, `>`, `+`, `[attr]` and
  `:first-child` work. `:last-child`, `:nth-child` and `:hover` don't. To
  space siblings, use `p + p { margin-top: 1 }`.
- **Not supported:** overflow clipping, flex wrapping, `colspan` and
  `rowspan`. Content taller than a box's `height` spills out, so keep it
  short. Lay grids out as several flex rows.
- **`data-build` on a table** hides the header row too. Put `data-step` on
  each body row instead to keep the header visible.
- **Units:** plain numbers are cells; `ch` and `em` work as cells, and `%` of
  the container works for widths and insets. `px` is rejected.
- **Charts need room:** the value axis takes the widest value label + 2
  columns, and the axis and labels take 2 rows. Below ~30×10 cells, draw a
  small `<pre>` picture instead (see the gallery slide).
  - For equal bars, make the plot width a multiple of the number of
    categories: 3 bars in 31 columns come out 11/10/10.
  - Steps work on a whole chart (`data-step` on the `<table>`) but not on its
    rows.
- **Bold brightens.** `font-weight: bold` turns a color into its bright
  version (blue → lightblue, black → darkgray). Dark accent text on grey
  should stay `font-weight: normal`; the kit does this for `b` in panels.
- **Spaces collapse** outside `<pre>`, as in HTML. File listings, screen
  mocks and aligned columns need `<pre>` (the kit's `pre.screen`) or
  `white-space: pre`.
- **Steps are numbered in source order.** An empty `data-step` or a
  `data-build` item takes the step after the highest one so far in the HTML,
  even if that number came from an element further up the screen. A list
  placed after an explicit `data-step="3"` starts at step 4. To sync a
  picture with a list, number every element with `data-step="N"` instead
  of `data-build`.
- **Silent in the viewer, loud in `check`:** the viewer ignores unknown CSS,
  unknown fonts and bad values without a word; `scripts/check.sh` lists them.
  Run it after every round of edits.
- **Colors:** use the 16 EGA names: black, blue, green, cyan, red, magenta,
  brown, lightgray, darkgray, lightblue, lightgreen, lightcyan, lightred,
  lightmagenta, yellow, white. Other CSS colors snap to the nearest one,
  sometimes surprisingly.

## Files in this skill

- `references/works-style.md`: the Works tutorial look, analyzed from two
  reference screens, with palette roles, composition, writing tone, and how
  to build picture scenes from pattern characters. Read it when designing a
  deck's look or a picture.
- `references/components.md`: everything the engine supports (deck.toml,
  elements, CSS properties, boxes, flex, positioning, tables, charts,
  pictures, big text) plus copy-paste components. Read it while writing HTML.
- `references/animation.md`: builds, transitions, effects and frames, with
  timings and pacing advice. Read it before adding motion.
- `assets/starter-deck/`: the Works kit `style.css` (`.field`, `.sky`,
  `.band`, `.panel`, `.dialog`, `.grid`/`.card`, `pre.screen`, and the color
  classes `.k .b .c .m .r .y .w` for block art), a `deck.toml`, and four
  example slides:
  - title dialog;
  - a picture scene with narration and typewriter;
  - a gallery of 8 cards;
  - a chart with a build list.
- `scripts/preview.sh`: shows a slide as a numbered text grid or as color
  grids; `scripts/check.sh` lists a deck's problems; `scripts/engine.sh`
  finds or builds the engine; `scripts/colormap.py` turns colored output
  into grids.

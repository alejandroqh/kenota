# dos-kenota — TODO

A text-only presentation engine in the style of the MS Works DOS tutorials:
EGA 16 colors, box drawing, ░▒▓ fills, drop shadows, and a footer bar
reading `◄◄◄ PgUp • n/N • PgDn ►►►`.

## Decisions

- One presentation = one folder. One slide = one HTML file, using standard tags.
- Fixed 80×25 canvas, centered in the terminal with a border when there is room.
  A deck can choose 80×43 or 80×50 instead.
- Layout: normal flow by default, plus `position:absolute` in character cells (overlap allowed).
- Animations: slide transitions, step-by-step builds, text effects, frame animation (milestone 6).
- Pictures: `<img>` PNG/GIF reduced to 16 colors and drawn with ▀▄ blocks; `<pre>` art too.
- No JavaScript.

## Deck format

```
my-talk/
├── deck.toml          # optional
├── 01-intro.html      # slides play in natural filename order (01, 02 … 10)
├── 02-charts.html
├── style.css          # optional, applies to every slide
└── img/storefront.png
```

```toml
# deck.toml (every key is optional)
title      = "Works Tutorial"
size       = "80x25"        # or 80x43 / 80x50
transition = "dissolve"     # default for all slides
footer     = true           # bottom 2 rows: slide title + PgUp/PgDn bar
```

- Plain numbers in CSS mean character cells (`left: 34`, `gap: 2`, `box-shadow: 2 1 black`).
- `<title>` is the slide title shown in the footer. Without it, the file name is used.

## Feature map

| Feature | Authoring | Result |
|---|---|---|
| Large text | `<h1>`, `data-font` to pick a font | FIGlet block letters; extra `.flf` fonts can live in the deck |
| Text | `p`, `span`, `br`, `pre`, `code`, `hr` | wrapped text |
| Emphasis | `b`/`strong` bright, `u` underline, `em` alternate color | no italics in DOS |
| Highlight | `<mark>` or `background:` | colored bar behind the text |
| Bullets | `ul`/`ol`/`li`, `list-style-type` | • ► ■ ♦ |
| Boxes | `border: single \| double \| thick`, `padding`, `box-shadow` | ┌─┐ ╔═╗ frames and black drop shadows |
| Fills | `--fill: "░" \| "▒" \| "▓"` | dithered backgrounds |
| Placement | flow, `display:flex` rows, `position:absolute` + `z-index` | overlapping boxes |
| Charts | `<table data-chart="bar\|stacked\|stacked100\|line\|area\|hilo\|pie\|xy">` | header row = series, first column = categories |
| Pictures | `<img src width>` | 16 colors, ▀▄ blocks, optional ordered dithering |
| Colors | the 16 EGA names; other CSS colors snap to the nearest | exact EGA RGB, ANSI-16 fallback |

## Animations

- Transitions: `<meta name="transition" content="wipe-right">` — cut, wipe ×4, dissolve, blinds, push.
  Each is a mask function `(x, y, t) -> old | new`.
- Builds: `data-step="N"` on any element, `data-build` on a list. PgDn advances steps first.
- Text effects: `data-effect="typewriter | marquee | rainbow"`, blinking via `text-decoration: blink`
  (drawn by the engine, not the terminal).
- Frame animation: `<div data-frames data-fps="4">` holding several `<pre>` frames.

## Architecture

```
folder ─load─► Deck ─parse─► DOM ─style─► styled tree ─layout─► display list ─┐
                   time + build step ─────────────────────────────────────────┤
                                                                              ▼
                     ratatui buffer ◄─ letterbox + footer ◄─ Canvas 80×25 ◄─ paint
```

- A slide compiles once into a fixed display list; painting is a pure function of
  (display list, step, time). The canvas is a terminal-independent `ratatui::Buffer`,
  so slides can be snapshot-tested.
- Charts and pictures share a half-block pixel canvas (80×50 pixels at 16 colors).
- Event loop: no async runtime. Input thread + file watcher feed one channel; the loop waits
  until the next animation deadline, or forever when nothing animates.

```
src/
├── lib.rs         engine library
├── main.rs        command line + terminal setup + event loop
├── app.rs         current slide/step, keys, drawing
├── deck.rs        folder loading, deck.toml, slide order
├── html.rs        parsing
├── palette.rs     EGA colors, ANSI-16 fallback
├── style/         CSS subset, cascade, computed style
├── layout/        flow, wrapping, flex rows, absolute positioning, lists, tables
├── render.rs      slide → canvas (grows into render/: boxes, big text, charts, images)
├── anim/          transitions, effects, frames
├── chrome.rs      footer bar, letterbox, "terminal too small" screen
└── watch.rs       reload when files change
```

Crates: `ratatui`, `scraper`, `simplecss`, `image` (png+gif), `toml` + `serde`, `notify`,
`clap`, `unicode-width`, `color-eyre`; dev: `insta`, `tempfile`.

## Milestones

### 1. Shell ✅
- [x] Library + binary split, `clap` command line: `dos-kenota <deck> [--slide N] [--colors auto|truecolor|ansi16]`
- [x] Load a deck folder: optional `deck.toml`, `.html` slides in natural filename order
- [x] Slide title from `<title>`, falling back to the file name
- [x] Plain-text extraction of the slide body (temporary until milestone 2)
- [x] 80×25 canvas with EGA colors, ANSI-16 fallback when the terminal lacks truecolor
- [x] Footer bar: slide title, `◄◄◄ PgUp • n/N • PgDn ►►►`, dimmed arrows at the ends
- [x] Centered canvas with a border when there is room; "terminal too small" screen
- [x] Keys: PgDn/→/↓/Space/Enter next, PgUp/←/↑/Backspace back, Home/End, q/Esc/Ctrl+C quit
- [x] Demo deck in `decks/demo`
- [x] Tests: deck loading, text extraction, navigation, chrome, canvas snapshot

### 2. HTML and CSS core ✅
- [x] DOM → styled tree; CSS subset from `<style>`, `style=""`, deck `style.css`
- [x] Selectors: tag, `.class`, `#id`, compound, descendant (also `>`, `+`, `[attr]`, `:first-child`)
- [x] EGA color names + nearest-color snapping
- [x] Block/inline flow, word wrapping, margins, padding
- [x] Headings, `b`/`strong`/`u`/`em`/`mark`, `pre`, `code`, `hr`, `br`
- [x] Lists with DOS bullets and numbering
- [x] Snapshot tests per feature

Notes:
- All default looks live in `src/style/ua.css`; decks override them with normal CSS.
- Cascade: defaults → deck `style.css` → slide `<style>` → `style=""`; specificity, then order; `!important` wins.
- Supported properties: `color`, `background(-color)`, `font-weight`, `text-decoration`, `display`
  (`block|inline|list-item|none`), `margin*`, `padding*`, `width`, `text-align`, `white-space`
  (`normal|nowrap|pre|pre-wrap`), `text-transform`, `list-style(-type)`. `--custom` properties are accepted.
- Bold = the bright version of the color (DOS high intensity). Vertical margins collapse.
- CSS problems are collected per slide in `Slide::warnings`, ready for `check` (milestone 7).
- selector matching uses `simplecss`; rules and declarations use our own tokenizer (custom properties, warnings).

### 3. Boxes ✅
- [x] Borders (single, double, thick), drop shadows, `--fill` dither patterns
- [x] `display:flex` rows with gap and fixed/percent/auto widths
- [x] `position:absolute` in cells, `z-index`, overlap
- [x] Plain tables with box-drawing grid

Notes:
- `border: single | double | thick [color]` (CSS `solid` = single), plus `border-top` …,
  `border-style`, `border-color` and the per-side longhands. Mixed corners pick ╒ ╓ etc.
  The default border color is the text color.
- `width` and `height` include padding and border (like `box-sizing: border-box`).
  `margin: 0 auto` centers a box that has a width.
- `box-shadow: x y [color]` is solid (black by default) and only shows outside the box.
- `--fill: "░" | light | medium | dark [color]` repeats a character over the background.
- The body's background and fill cover the whole slide (CSS background propagation),
  so `z-index: -1` boxes show behind the text.
- Flex: one row. `flex` (`flex: 1` grows from a zero basis), `flex-grow`, `flex-basis`, `gap`,
  `justify-content`, `align-items` (stretch by default). Items without a width shrink to
  their longest word; items with a width keep it.
- `position: relative | absolute`, `top/right/bottom/left/inset` (cells or %), `z-index`.
  Absolute boxes go in the nearest positioned ancestor's padding box, else the slide
  area; without a width they shrink to fit.
- Tables: frame from the table's `border` (single by default), inner lines single, `rules`
  attribute (`groups` default = column lines + lines between thead/tbody/tfoot), a first
  row of `<th>` counts as a header, `<caption>`, row backgrounds. Columns fit content.
- Not yet: `colspan`/`rowspan`, flex wrapping and columns, `overflow` clipping.

### 4. Large text ✅
- [x] FIGlet `.flf` loader, 2–3 bundled fonts, `<h1>` + `data-font`

Notes:
- `src/figlet.rs` reads `flf2a` fonts (code-tagged characters too) and lays them out like
  `figlet`: full width, kerning, or smushing with the six rules; hardblanks draw as spaces.
- Bundled fonts, drawn for dos-kenota (`src/fonts/*.flf`): `block` (5×8 pixels in half
  blocks, 4 rows, the `<h1>` default), `small` (3×5 capitals, 3 rows), `big` (full blocks,
  8 rows). A deck adds fonts with `.flf` files in its folder or `fonts/`, named by file.
- `--font: <name> | none` is inherited; `data-font="name"` on an element works like an inline
  `--font`. Unknown names become slide warnings and draw plain text.
- Big text wraps between words, follows `text-align`, takes colors from inline elements,
  and leaves blank cells untouched so fills show between letters.

### 5. Pixel canvas ✅
- [x] Half-block pixel canvas shared by images and charts
- [x] `<img>`: decode, scale, quantize to EGA, ordered dithering
- [x] Charts: bar, stacked, stacked100, line, area, hilo, pie, xy

Notes:
- `src/pixels.rs`: two pixels per cell (`▀▄█`), transparent pixels keep what is behind.
- `src/images.rs`: PNG and GIF (first frame) via the `image` crate, loaded once per deck and
  shrunk to at most 200 pixels a side. Drawing averages source pixels, snaps to the nearest
  EGA color, and with `--dither: ordered` (inherited) uses a 4×4 Bayer matrix.
- `<img>` is a block. `width`/`height` attributes (and CSS) are cells; with one of them the
  picture keeps its shape, with neither it is one pixel per pixel, shrunk to fit.
  `margin: 0 auto` centers it. Missing pictures show `[alt]` and a slide warning.
- Fonts and pictures travel together as `Assets` (`deck.assets`), passed to layout.
- Charts: `src/chart.rs` reads the table (first row = series names, first column = categories,
  or X values for `xy`; empty cells are missing; `1,250`, `$12`, `45%` are numbers), and
  `src/layout/plot.rs` draws it: caption, value axis with round ticks (`k`/`M` for big numbers),
  category labels, and a centered legend when there are several series. A pie uses the first
  series and puts its legend with percentages beside it.
- A chart fills its container's width like a block; plot height is 10 rows unless the table
  has a `height`. No frame by default (`table[data-chart] { border: none }`); `border` adds one.
- Series look comes from the header cell: `background` sets the color, `--fill` draws `░▒▓`
  (bars then snap to whole cells); otherwise a palette. Pie slices use their first cell.
- `bar` includes zero and grows down for negatives; `line`/`xy` include zero unless the values
  bunch far from it; `hilo` and the X axis of `xy` span just their data. `hilo`: a line from the
  lowest to the highest value, the third series ticks right (close), the fourth left (open).
- Unknown `data-chart` types and non-numeric values are slide warnings; unknown types draw as
  a (frameless) table.

### 6. Animations ✅
- [x] Builds (`data-step`, `data-build`), PgDn/PgUp through steps
- [x] Timer-driven event loop (channel + deadlines)
- [x] Transitions: cut, wipe ×4, dissolve, blinds, push
- [x] Text effects: typewriter, blink, marquee, rainbow
- [x] Frame animation (`data-frames`); animated GIFs still to do

Notes:
- `src/anim/mod.rs`: after styling, one pass gives every node its build step and innermost
  animation (`ComputedStyle::step`, `anim`, and `box_anim` for its own background and border).
  Layout copies them onto each display item as a `Tag`; a slide is laid out once and drawn
  from a `Clock` (step, time on the slide, time in the step, forward or back).
- `data-step` = from step N; empty = the step after the highest so far. `data-build` gives each
  child element the next step (table rows for a table). Nested steps never come earlier than
  their parent's. Hidden steps keep their space, so nothing reflows.
- PgDn shows the next step, then the next slide; PgUp goes back a step, or to the previous slide
  fully built. Going back shows typewritten text complete.
- `data-effect="typewriter | marquee | rainbow"` (one per element; nest elements to combine).
  Typewriter: 30 characters/s from when its step appears. Marquee: one line (`white-space:
  nowrap`), 10 cells/s, clipped to the content box. Rainbow: six bright colors by column,
  moving 8 columns/s. `text-decoration: blink` toggles every 267 ms; highlighted text keeps its
  background. Effects change text only, not the element's border or background.
- `data-frames` lays out its child elements in the same place and shows one at a time,
  `data-fps` (default 4) per second.
- Transitions: `<meta name="transition" content="name [0.5s|500ms]">` or `transition =` in
  `deck.toml` (a bad deck value stops loading; a bad slide value is a warning). Default
  durations: wipes and push 0.4 s, blinds 0.5 s (8-column slats), dissolve 0.6 s. Going back
  plays the transition of the slide being left, mirrored. Home and End cut.
- Event loop (`main.rs`): an input thread feeds a channel; the loop draws, then waits for input
  or the next deadline from `App::next_deadline` (transition frames at 30 fps, the next
  typewriter letter, the next blink, frame, marquee or rainbow step), or forever when still.

### 7. Authoring tools
- [ ] Reload on save (`notify`)
- [ ] `check <deck>`: unsupported tags/CSS, missing images, content past the canvas
- [ ] `render <deck> --slide N`: print a slide as ANSI to stdout
- [ ] Jump to slide by typing its number + Enter
- [ ] Example deck recreating both MS Works screenshots

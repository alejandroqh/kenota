# dos-kenota — TODO

A text-only presentation engine in the style of the MS Works DOS tutorials:
EGA 16 colors, box drawing, ░▒▓ fills, drop shadows, and a footer bar
reading `◄◄◄PgUp • n/N • PgDn►►►`.

## Decisions

- One presentation = one folder. One slide = one HTML file, using standard tags.
- Fixed 80×25 canvas, centered in the terminal with a border when there is room.
  A deck can choose 80×43 or 80×50 instead.
- Layout: normal flow by default, plus `position:absolute` in character cells (overlap allowed).
- Animations: slide transitions, step-by-step builds, text effects, frame animation.
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
- [x] Footer bar: slide title, `◄◄◄PgUp • n/N • PgDn►►►`, dimmed arrows at the ends
- [x] Centered canvas with a border when there is room; "terminal too small" screen
- [x] Keys: PgDn/→/↓/Space/Enter next, PgUp/←/↑/Backspace back, Home/End, q/Esc/Ctrl+C quit
- [x] Demo deck in `decks/demo`
- [x] Tests: deck loading, text extraction, navigation, chrome, canvas snapshot

### 2. HTML and CSS core
- [ ] DOM → styled tree; CSS subset from `<style>`, `style=""`, deck `style.css`
- [ ] Selectors: tag, `.class`, `#id`, compound, descendant
- [ ] EGA color names + nearest-color snapping
- [ ] Block/inline flow, word wrapping, margins, padding
- [ ] Headings, `b`/`strong`/`u`/`em`/`mark`, `pre`, `code`, `hr`, `br`
- [ ] Lists with DOS bullets and numbering
- [ ] Snapshot tests per feature

### 3. Boxes
- [ ] Borders (single, double, thick), drop shadows, `--fill` dither patterns
- [ ] `display:flex` rows with gap and fixed/percent/auto widths
- [ ] `position:absolute` in cells, `z-index`, overlap
- [ ] Plain tables with box-drawing grid

### 4. Large text
- [ ] FIGlet `.flf` loader, 2–3 bundled fonts, `<h1>` + `data-font`

### 5. Pixel canvas
- [ ] Half-block pixel canvas shared by images and charts
- [ ] `<img>`: decode, scale, quantize to EGA, ordered dithering
- [ ] Charts: bar, stacked, stacked100, line, area, hilo, pie, xy

### 6. Animations
- [ ] Builds (`data-step`, `data-build`), PgDn/PgUp through steps
- [ ] Timer-driven event loop (channel + deadlines)
- [ ] Transitions: cut, wipe ×4, dissolve, blinds, push
- [ ] Text effects: typewriter, blink, marquee, rainbow
- [ ] Frame animation (`data-frames`), animated GIFs later

### 7. Authoring tools
- [ ] Reload on save (`notify`)
- [ ] `check <deck>`: unsupported tags/CSS, missing images, content past the canvas
- [ ] `render <deck> --slide N`: print a slide as ANSI to stdout
- [ ] Jump to slide by typing its number + Enter
- [ ] Example deck recreating both MS Works screenshots

# dos-kenota components and syntax

Everything the engine understands, and ready-made pieces in the Works style.
Anything not listed here is ignored without an error, so stick to this list.

Contents:
1. Deck folder and deck.toml
2. Slide skeleton
3. Elements and their default look
4. CSS: units, selectors, cascade
5. CSS properties
6. Boxes: borders, shadows, fills
7. Placement: flow, flex rows, absolute positioning
8. Tables
9. Charts
10. Pictures
11. Big text
12. Works components (copy-paste)

## 1. Deck folder and deck.toml

```
my-talk/
├── deck.toml        # optional
├── style.css        # optional: applies to every slide
├── 01-intro.html    # slides, in natural filename order (2 before 10)
├── 02-story.html
├── img/shop.png     # pictures, referenced relative to the deck
└── fonts/logo.flf   # extra FIGlet fonts, named by file (also allowed at the top level)
```

```toml
# every key is optional
title      = "Works Tutorial"   # shown at the bottom left of the footer
size       = "80x25"            # or "80x43" / "80x50"
transition = "dissolve"         # default transition for every slide
footer     = true               # false gives the slide all 25 rows
```

A bad `transition` in deck.toml stops the deck from loading, with an error
message naming the valid choices.

## 2. Slide skeleton

```html
<!doctype html>
<html>
<head>
  <title>Short Title</title>                       <!-- footer title; else the file name -->
  <meta name="transition" content="wipe-right">    <!-- optional, this slide only -->
  <style>
    /* slide CSS; plain numbers are cells */
  </style>
</head>
<body class="field">
  ...
</body>
</html>
```

There is no JavaScript. `head`, `script`, `style`, `title`, `meta` and
`template` are never drawn.

## 3. Elements and their default look

These come from the engine's built-in stylesheet; override any of them with CSS.

| Element | Looks like |
|---|---|
| `body` | lightgray on black, margin `1 2 0 2` (the starter kit uses `margin: 0` on blue) |
| `h1` | big white FIGlet letters (`block` font), margin-bottom 1 |
| `h2` | yellow, `h3` lightcyan, `h4`–`h6` white; plain text size, margin-bottom 1 |
| `p`, `pre`, `blockquote`, `figure`, `table`, `dl` | margin-bottom 1 |
| `b`, `strong`, `th` | bold = the bright version of the color (lightgray → white, blue → lightblue) |
| `em`, `i`, `cite`, `var` | yellow (DOS has no italics) |
| `u`, `ins` | underline; `s`, `del` line-through |
| `mark` | black on cyan bar |
| `code`, `samp`, `tt` | lightgreen |
| `kbd` | black on lightgray key cap |
| `hr` | darkgray line |
| `ul` | bullets • then ■ then ○ for nested lists; padding-left 2 |
| `ol` | 1. 2. 3., nested a. b. c.; `start` attribute works |
| `blockquote` | padding-left 2; `dd` padding-left 4 |
| `center` | centered block |
| `img` | block picture (see Pictures) |
| `table` | single-line frame, `td`/`th` padding `0 1`, `th` centered |
| `br` | line break |

Other block containers (`div`, `section`, `header`, `footer`, `main`, `nav`,
`aside`, `article`, `figure`, `figcaption`) are plain blocks. Unknown inline
tags act like `span`.

## 4. CSS: units, selectors, cascade

- **Units:** a plain number is character cells (`left: 34`, `gap: 2`); `ch`,
  `em` and `rem` also count as cells; `%` of the container works for `width`
  and the insets. `px` is rejected. Negative numbers work for margins and
  insets.
- **Where CSS comes from:** the defaults, then the deck's `style.css`, then
  the slide's `<style>`, then `style=""`. Specificity, then order;
  `!important` wins.
- **Selectors:** `tag`, `.class`, `#id`, compound (`p.note`), descendant
  (`.panel b`), child (`>`), adjacent (`+`), `[attr]` and `[attr=value]`,
  `:first-child`. Not available: `:last-child`, `:nth-child`, `:hover`,
  `::before`/`::after`.
- **Custom properties:** `--fill`, `--font` and `--dither` are engine
  properties; other `--names` are accepted and ignored.

## 5. CSS properties

| Property | Values |
|---|---|
| `color` | one of the 16 EGA names (below), or any CSS color, which snaps to the nearest |
| `background`, `background-color` | color, `transparent`, `none` |
| `font-weight` | `bold` (bright color) / `normal` |
| `text-decoration` | `underline`, `line-through`, `blink`, `none` (combinable) |
| `text-align` | `left`, `center`, `right` (also `start`/`end`) |
| `text-transform` | `uppercase`, `lowercase`, `none` |
| `white-space` | `normal`, `nowrap`, `pre`, `pre-wrap` |
| `display` | `block`, `inline`, `list-item`, `flex`, `table`, `none` |
| `margin`, `margin-*` | cells, `auto` (`margin: 0 auto` centers a box that has a width) |
| `padding`, `padding-*` | cells |
| `width` | cells or % (includes padding and border) |
| `height` | cells (includes padding and border; content taller than it is not clipped) |
| `border`, `border-top`… | `single`, `double`, `thick`, `none` + optional color (`solid` = single) |
| `border-style`, `border-color` | per-side longhands work too |
| `box-shadow` | `x y [color]`, e.g. `2 1` (black) |
| `--fill` | `light` ░, `medium` ▒, `dark` ▓, or any one character (`"▌"`, `"♦"`) + optional color |
| `--font` | FIGlet font name (`block`, `small`, `big`, or a deck font), `none` |
| `--dither` | `ordered` / `none` for pictures (inherited) |
| `position` | `static`, `relative`, `absolute` |
| `top`, `right`, `bottom`, `left`, `inset` | cells or % |
| `z-index` | integer; higher is drawn on top |
| `flex`, `flex-grow`, `flex-basis` | `flex: 1` grows from zero; widths in cells or % |
| `gap`, `column-gap` | cells between flex items |
| `justify-content` | `start`, `center`, `end`, `space-between`, `space-around` |
| `align-items` | `stretch` (default), `start`, `center`, `end` |
| `list-style-type`, `list-style` | `disc`, `circle`, `square`, `decimal`, `lower-alpha`, `upper-alpha`, `lower-roman`, `upper-roman`, `none`, or a string like `'► '` |

**The 16 colors**:

| Dark | Bright |
|---|---|
| `black` | `darkgray` (`gray`) |
| `blue` (`navy`) | `lightblue` |
| `green` | `lightgreen` (`lime`) |
| `cyan` (`teal`) | `lightcyan` (`aqua`) |
| `red` (`maroon`) | `lightred` |
| `magenta` (`purple`) | `lightmagenta` (`fuchsia`, `pink`) |
| `brown` (`olive`, `orange`) | `yellow` |
| `lightgray` (`silver`) | `white` |

**Attributes the engine reads:**
- `data-font` (on any text element);
- `data-step`, `data-build`, `data-effect`, `data-frames`, `data-fps` (see
  animation.md);
- `data-chart` (tables);
- `src`, `width`, `height` and `alt` on `img`;
- `rules` on `table`;
- `start` on `ol`.

## 6. Boxes: borders, shadows, fills

```css
.box {
  width: 40;                 /* includes border + padding: text gets 40-2-4 = 34 */
  padding: 1 2;              /* 1 row, 2 columns: looks even, since cells are tall */
  border: double yellow;     /* ╔═╗; single ┌─┐; thick █▀▄ */
  box-shadow: 2 1;           /* solid black, 2 right and 1 down, outside the box */
  background: lightgray;
  color: black;
}
.dither { background: blue; --fill: medium lightblue; }   /* ▒ lightblue on blue */
.stripes { background: red; --fill: "▌" lightred; }       /* vertical stripes */
```

- **Border color:** the text color unless given.
- **Mixed borders:** a box can mix borders per side; corners join correctly
  (╒ ╓ …).
- **Fill colors:** in `--fill`, the background color sits behind the
  character and the fill color draws the character. Text written on a filled
  box replaces the pattern in those cells.
- **Background propagation:** the `body` background and fill cover the whole
  slide, as in CSS.
- **Shadows** draw only outside the box, so they never hide the box's own
  content.

## 7. Placement: flow, flex rows, absolute positioning

- **Flow:** blocks stack top to bottom. Vertical margins collapse (the
  larger of two adjacent margins wins). Text wraps between words.
- **Flex rows:** `display: flex` puts children side by side, on one row only;
  there is no wrapping and no columns.
  - Items stretch to the tallest by default.
  - `flex: 1` shares the space equally. `flex: 3` beside `flex: 2` is a
    60/40 split.
  - An item with a `width` keeps it; an item with neither `width` nor `flex`
    shrinks to its longest word.
  - For a grid, stack several flex rows (`.grid + .grid { margin-top: 2 }`).
- **Absolute positioning:** `position: absolute` places a box in cells
  relative to the nearest `position: relative|absolute` ancestor's padding
  box, else the slide area (0,0 = top-left).
  - Without a width, a box shrinks to fit its content.
  - Boxes may overlap. Later boxes and higher `z-index` draw on top, and
    `z-index: -1` goes behind the flow content.
  - Use it for scenes, stamps, callouts and anything that must not move
    when text changes.

## 8. Tables

```html
<table rules="groups">           <!-- groups (default) | rows | cols | all | none -->
  <caption>Quarterly sales</caption>
  <thead><tr><th>Region</th><th>Q1</th><th>Q2</th></tr></thead>
  <tbody>
    <tr><td>North</td><td>120</td><td>135</td></tr>
    <tr style="background: cyan; color: black"><td>South</td><td>95</td><td>110</td></tr>
  </tbody>
  <tfoot><tr><th>Total</th><th>215</th><th>245</th></tr></tfoot>
</table>
```

- **Frame:** from the table's `border` (single by default, `border: double`
  for a dialog look). Inner lines are single.
- **Header:** a first row of `<th>` counts as one.
- **Columns** fit their content. Align numbers with
  `td + td { text-align: right }`.
- **Not supported:** `colspan` and `rowspan`.

## 9. Charts

A table with `data-chart` is drawn as a chart instead:

```html
<table data-chart="bar" style="height: 12">
  <caption>Loaves ordered</caption>
  <tr><th></th><th style="background: blue">Bread</th><th style="--fill: medium magenta">Rolls</th></tr>
  <tr><td>Mon</td><td>40</td><td>25</td></tr>
  <tr><td>Tue</td><td>35</td><td>30</td></tr>
</table>
```

- **Types:** `bar`, `stacked`, `stacked100`, `line`, `area`, `hilo`, `pie`, `xy`.
- **Data layout:**
  - the first row names the series; the first column holds the categories
    (for `xy`, the X values);
  - empty cells are missing values;
  - `1,250`, `$12` and `45%` all count as numbers.
- **Series look:**
  - a header cell's `background` sets the series color, and `--fill` draws
    it with ░▒▓;
  - otherwise a bright palette, which looks bad on lightgray panels, so set
    colors there;
  - pie slices take their color from their row's first cell.
- **Height:** a chart's `height` is the whole chart.
  - It counts the caption, the plot, the axis row, the category labels row,
    and, with two or more series, a legend: one blank row plus one row per
    line of entries.
  - So `height: 12` with a caption and a one-line legend leaves
    12 − 1 − 2 − 2 = 7 plot rows.
  - Without a `height`, the plot is 10 rows and the rest comes on top.
- **Width:**
  - a chart fills its container's width;
  - the plot width is that width minus the widest value label (`60`,
    `1500`, `75%`) minus 2 for the axis;
  - it is split evenly between the categories. Make it a multiple of the
    category count for equal bars: 3 categories in 31 columns give bars
    11/10/10 wide.
  - Keep charts at least 30 columns wide and 10 rows tall.
- **Builds:** `data-step` on the `<table data-chart>` reveals the whole chart
  at a step. Steps on its rows or cells are ignored, so a chart can't grow
  bar by bar; reveal the explanation beside it instead.
- **Chart types:**
  - `bar` includes zero and grows down for negatives;
  - `pie` draws only the first series, with a legend and percentages beside it;
  - `hilo` draws a line from each row's lowest to its highest value; series
    3 adds a tick on the right (close) and series 4 one on the left (open).
    It is usually given as high, low, close, open.
- **Errors:** an unknown type draws as a frameless table; numbers that don't
  parse are warnings.

## 10. Pictures

```html
<img src="img/shop.png" width="32" alt="The shop">          <!-- width in cells -->
<img src="img/logo.gif" height="10" style="--dither: ordered; margin: 0 auto">
```

- **Formats:** PNG and GIF (first frame).
- **Drawing:** pictures are reduced to the 16 EGA colors and drawn with ▀▄,
  two pixels per cell (80×50 pixels on a full screen). Transparent pixels
  show the slide behind.
- **Size:**
  - with one of `width`/`height`, the picture keeps its shape;
  - with neither, one pixel per pixel, shrunk to fit;
  - `margin: 0 auto` centers it.
- **Style:** pixel art with flat colors looks best. Photos need
  `--dither: ordered` and still look rough, which can be the charm.
- **Missing file:** shows `[alt]`.

## 11. Big text

```html
<h1>Bakery</h1>                               <!-- block font, 4 rows -->
<p data-font="small">SECTION TWO</p>          <!-- 3 rows, capitals only -->
<p data-font="big">GO</p>                     <!-- 7 rows -->
<h2 style="--font: block">Also big</h2>
```

| Font | Rows | Columns per letter | Letters across 76 columns |
|---|---|---|---|
| `small` | 3 | 4 | ~19 |
| `block` | 4 | 6 | ~12 |
| `big` | 7 | 12 | ~6 |

- Big text wraps between words and follows `text-align`.
- It takes colors from inline elements (`<h1>Big <span style="color:yellow">text</span></h1>`).
- Blank cells between letters show the fill behind.
- `--font: none` turns an `h1` back into plain text.

## 12. Works components (copy-paste)

These assume the starter kit `style.css` (`assets/starter-deck/style.css`).

**Narration band + field** (rows 0–3 are the band):
```html
<body class="field">
  <div class="band">
    <p>Once you've entered your numbers,</p>
    <p>one command turns them into a chart.</p>
  </div>
  ...
</body>
```

**Panel** (grey, black text, shadow):
```html
<div class="panel" style="width: 44; margin: 2 auto">
  <p>Text in a panel reads best at 40–50 characters per line.</p>
  <p>Press <kbd>Esc</kbd> to cancel.</p>
</div>
```

**Dialog** (double frame, `width: 50`; `position: relative; left: -1` centers box + shadow):
```html
<div class="dialog">
  <p>Save changes to SALES.WKS?</p>
  <p>&nbsp;</p>
  <p><kbd> Yes </kbd>  <kbd> No </kbd>  <kbd> Cancel </kbd></p>
</div>
```

**Card gallery** (4 across, 2 rows, inside the field):
```html
<style> .gallery { padding: 2 5 2 3; } .card pre { width: 13; } </style>
<div class="gallery">
  <div class="grid">
    <div class="card"><pre>...13-wide art, 4 rows...</pre><p class="label">Bar</p></div>
    <!-- 3 more cards -->
  </div>
  <div class="grid"> <!-- 4 more cards --> </div>
</div>
```

**Two columns** (60/40, both shadowed):
```html
<div style="display: flex; gap: 4; padding: 2 5 2 3">
  <div class="panel" style="flex: 3">chart or picture</div>
  <div class="panel" style="flex: 2">steps</div>
</div>
```

**Callout stamp** (overlaps whatever is under it):
```html
<div style="position: absolute; right: 6; top: 10; z-index: 2;
            padding: 0 1; background: red; color: white; font-weight: bold">NEW!</div>
```

**Screen mock with a typed command** (spaces kept by `<pre>`; the span types
out when its step comes):
```html
<pre class="screen" style="width: 42">C:\&gt;<span data-step data-effect="typewriter">COPY C:\LETTERS\*.* A:</span>
<span data-step>C:\LETTERS\MOM.TXT
C:\LETTERS\BANK.TXT
        2 file(s) copied</span>
C:\&gt;<span style="text-decoration: blink">_</span></pre>
```

**A light that starts flickering at a step** (frames + step + position on
one element):
```html
<div data-step="3" data-frames data-fps="6"
     style="position: absolute; left: 30; top: 18">
  <span style="color: lightred">■</span>
  <span style="color: black">■</span>
</div>
```

**Checklist** (any string can be a list marker):
```html
<ul style="list-style-type: '√ '; padding-left: 0">
  <li>Write today's date on the label.</li>
  <li>Keep it away from magnets.</li>
</ul>
```

**Key list** (keyboard reference in a panel):
```html
<div class="panel">
<pre><kbd>F1</kbd>  Help         <kbd>Alt</kbd>  Menu
<kbd>F10</kbd> Exit         <kbd>Esc</kbd>  Cancel</pre>
</div>
```

**Scene pieces** (see works-style.md for building whole pictures):
```css
.scene div { position: absolute; }
.awning { background: red; --fill: "▌" lightred; }      /* stripes */
.trim   { background: red; --fill: "♦" yellow; }        /* ornament row */
.brick  { background: red; --fill: "▒" brown; }
.glass  { background: black; }                           /* windows */
.water  { background: blue; --fill: "▀" lightblue; }     /* waves / street */
.city   { background: lightgray; --fill: dark blue; }    /* distant buildings */
```
```html
<div class="awning" style="left: 0; top: 10; width: 22; height: 3"></div>
```

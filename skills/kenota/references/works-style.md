# The Works tutorial look

The Microsoft Works for DOS (1988) tutorials taught software with full-screen
EGA pictures: a scenario, a bit of narration, and one clear visual per
screen, paged with PgUp/PgDn. kenota exists to make slides that feel like
those screens. This file breaks the look down so you can reproduce it on
purpose instead of by accident.

## The two reference screens

Both are 640×350 EGA screens: 80 columns × 25 rows of 8×14-pixel cells.
Positions below are in cells.

### "Spreadsheet Charting": a gallery of chart formats

```
row 0-3   ████████████████████████████████████████  black band
            Once you've created a chart,             white text, col 4
            you can quickly and easily change ...
row 4-21  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒  blue field, ▒ lightblue on blue
          ▒ ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐ ▒  4 grey cards per row,
          ▒ │ art  │█ │ art  │█ │ art  │█ │ art  │█▒  ~17 cols × 7 rows,
          ▒ │ Bar  │█ │Stack │█ │ 100% │█ │ Line │█▒  black shadow right + below,
          ▒ └──────┘█ └──────┘█ └──────┘█ └──────┘█▒  label bold white, bottom centre
          ▒  ███████   ███████   ███████   ███████  ▒
          ▒ (second row of 4 cards)                  ▒
row 23-24 black, footer at the right: ──Spreadsheet Charting── / ◄◄◄PgUp • Controls: Ctrl • PgDn►►►
```

What it teaches:
- **Narration on top, in a black band:** it reads first, and the black
  separates it from the busy field.
- **One repeated unit (the card), many times:**
  - equal sizes and gaps;
  - the same shadow;
  - the same label position.
  - Repetition makes a screen calm even with eight pictures on it.
- **Inside the cards, only three data colors:** blue, magenta and cyan, with
  darkgray axes, on lightgray. The pictures are tiny and abstract: an icon
  of each chart, not a real chart.
- **Labels are the only bright text** (white, bold) in the field, so they
  read as captions.

### "Copying Between Tools": a story scene

```
row 0-10  grey dithered sky; dark blue dithered skyline on the left
          ┌──────────────────────────────────┐   narration panel, cols 32-75,
          │ As sales manager of the ... it's │   rows 1-8: lightgray, black text,
          │ your job to assemble the monthly │   padding 1 row / 2 cols,
          │ sales memo. In the following ... │   no frame, no shadow
          └──────────────────────────────────┘
row 8-21  a street of storefronts, side by side, each 18-22 columns wide:
          red/lightred stripes, a ♦ trim row, a blue front with black windows,
          a cyan sign with ═ rules ("Friends of the Rivers / since 1953"),
          a magenta sign with yellow capitals, a flower stall of colored checks
row 21-24 blue street; footer on blue at the right: ──Copying Between Tools──
```

What it teaches:
- **A scenario first:** "As sales manager of … it's your job to … In the
  following practices you'll use Works to …". The picture sets the scene;
  it doesn't explain the software.
- **The picture fills the screen, and the narration sits on top** at the
  top right, where the sky is empty. Text never runs over the busy
  storefronts.
- **Texture comes from patterns, not detail:** stripes, checks, dithering,
  solid black windows. Each building is a stack of a few rectangles.
- **Signs are where the bright colors and capitals go** (yellow on magenta,
  lightcyan on cyan). They are the "headlines" of the picture.

## Palette roles

| Role | Colors | CSS (starter kit) |
|---|---|---|
| Field / backdrop | blue with lightblue ▒ | `.field` (`background: blue; --fill: medium lightblue`) |
| Narration band | black, white text | `.band` |
| Panels, cards, dialogs | lightgray, black text | `.panel`, `.card`, `.dialog` |
| Shadow | black, 2 columns right, 1 row down | `box-shadow: 2 1` |
| Labels, captions | white bold | `.card .label` |
| Data / picture accents | blue, magenta, cyan (+ darkgray lines) | `.b .m .c .k` |
| Signs, highlights | yellow on magenta/blue, white on red, lightcyan on cyan | |
| Key names | white on black inside grey panels | `kbd` in `.panel` |
| Sky | lightgray with darkgray ▒ | `.sky` |
| Footer | drawn by the engine: cyan title, white arrows | |

Rules of thumb:
- **Hierarchy:**
  - dark, calm colors cover the large areas;
  - mid-tone panels sit on them;
  - the brightest color is kept for the smallest, most important things
    (labels, the one key to press).
- **Accents:** at most three per slide, plus the base colors (blue, black,
  lightgray, white). This is for text, panels and diagrams. A picture scene
  may use more (the reference street has red, lightred, yellow, magenta and
  cyan) as long as the narration panel and band around it stay plain.
- **Bright on bright vibrates:** yellow on lightgray or white on lightcyan
  is hard to read. Put bright text on dark grounds and black text on grey.
- **Brown is the EGA "dark yellow"** and does well for wood, earth and brick.
- **Check contrast against the neighbor, not the page:** a darkgray detail
  vanishes on a darkgray bezel, and blue on blue disappears. An "off" light
  on a grey box is black, not darkgray.

## Composition patterns

Each pattern has a starter slide you can copy.

1. **Title dialog** (`01-title.html`):
   - full field, big block-letter title near the top;
   - a centered double-framed dialog with two lines and a blinking "Press
     PgDn".
2. **Narrated scene** (`02-story.html`):
   - a picture across the whole slide area;
   - a grey narration panel at the top right, `z-index: 1`.
3. **Narrated gallery** (`03-gallery.html`):
   - band (rows 0–3), field below with a grid of equal cards (2 rows × 4);
   - use it for "here are the options", "the parts of the screen", or "the
     tools in this course".
4. **Narrated work** (`04-chart.html`):
   - band, then two panels side by side: the visual (chart, table, screen
     mock) at 60%, numbered steps at 40%, built one by one.
5. **Screen mock** (for software lessons): draw the program instead of a
   screenshot.
   - A lightgray menu bar on row 0 with black text, and the active menu as
     a framed lightgray box dropping down with a shadow.
   - The work area in blue with white text, and a status line in black on
     cyan at the bottom.
   - Then point at things with a stamp (`position: absolute`, red or yellow
     background) or a build step.
6. **Summary**: a single panel or dialog listing 3–4 points, with the band
   saying what comes next.

To vary a deck, alternate patterns. Two galleries in a row feel like a
catalogue, and a scene between them resets the eye.

## Writing the narration

- **Speak to the viewer:** "you", present tense, concrete. Name a role and a
  goal before a feature: "You run the bakery. Every Friday you send the
  order." beats "This lesson covers spreadsheets."
- **Keep it short:**
  - 2–5 lines;
  - 40–50 characters a line in panels, up to about 70 in a band;
  - break lines by hand at phrase boundaries (one `<p>` per line in a band
    or dialog gives exact control).
- **One idea per screen.** If you need "and also", make another slide.
- **Name keys and menu items exactly:** `<kbd>Esc</kbd>`, `<b>New Chart</b>`.
  In a grey panel the kit turns `b` blue and `kbd` white on black.
- **Plain words:** the tutorials never say "leverage" or "seamlessly"; they
  say "quickly and easily change its format".

## Building pictures from blocks

A scene is a stack of rectangles placed with `position: absolute`, drawn back
to front, each with a background and maybe a pattern. Work like this:

1. **Plan on the grid.** Sketch it in a comment or on paper at 80×23:
   - the horizon row;
   - where each building starts and ends (column ranges);
   - where the text panel goes (keep that area simple: sky, not buildings).
2. **Back to front:** sky (the `body` background and fill), far skyline,
   buildings, details (windows, doors, signs), foreground (street), then
   the narration panel with `z-index: 1`.
3. **Two to four rectangles per object.** A shop is an awning band, a trim
   row, a front, a sign and two windows. Resist drawing details smaller than
   2 cells.
4. **Use patterns for material.** The character shows in the fill color over
   the background color:

| Material | CSS |
|---|---|
| Awning stripes | `background: red; --fill: "▌" lightred` |
| Ornament / trim | `background: red; --fill: "♦" yellow` |
| Brick | `background: red; --fill: "▒" brown` |
| Glass front | `background: blue; --fill: light lightblue` |
| Concrete / stone | `background: lightgray; --fill: light darkgray` |
| Distant city | `background: lightgray; --fill: dark blue` |
| Water / street | `background: blue; --fill: "▀" lightblue` |
| Grass | `background: green; --fill: "▒" lightgreen` |
| Wood | `background: brown; --fill: "║" red` |
| Flowers / confetti | `background: cyan; --fill: "▄" lightred` |
| Window, door | solid `black` or `darkgray` |

Indoors (desks, offices, a computer):

| Material | CSS |
|---|---|
| Wall | the blue `.field`, or `background: cyan; --fill: light lightcyan` |
| Wallpaper | `background: brown; --fill: "│" red` |
| Desk / wood | `background: brown; --fill: "═" red` (grain runs along the desk) |
| Floor, carpet | `background: red; --fill: light magenta` or `background: darkgray; --fill: "▒" black` |
| Plastic case (computer, keyboard) | solid `lightgray`; darkgray `▬`/`▄` for slots and edges |
| Screen | `pre.screen`: black, lightgray or lightgreen text |
| Paper, labels | `white` with black or blue text |
| Mug, lamp | `white` with a `brown` top row (coffee) or `yellow` (light) |

5. **Signs carry text.** A sign is a box with a solid background, bright
   text and `text-align: center`; `═` or `─` runs make a ruled frame
   (`══ Ocean Books ══`).
6. **Depth:** farther things are darker or more dithered (`dark blue`
   skyline), nearer things are solid and bright. Overlap buildings slightly
   to show which is in front.
7. **Pictures instead:** an `<img>` with a small, flat-colored PNG (pixel
   art at 2 pixels per cell) is the other way to make a scene, good for
   objects with curves (a computer, a face, a logo).

## Checklist for a Works-looking slide

- [ ] Narration in a black band or a grey panel; 2–5 short lines; second person.
- [ ] One focal visual; the field shows around it.
- [ ] Every raised box has the same `box-shadow: 2 1`.
- [ ] Margins look equal with shadows counted (right = left + 2).
- [ ] At most three accent colors; bright text only on labels, signs and keys.
- [ ] A 2–3 word `<title>` for the footer.
- [ ] Previewed: no stray wraps, nothing below row 22.

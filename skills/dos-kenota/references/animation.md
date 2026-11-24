# Motion in dos-kenota

Four kinds of motion: builds (PgDn reveals the next part), transitions
(how one slide replaces the next), text effects (typewriter, marquee,
rainbow, blink) and frame animation. All are declared in HTML; there is no
script.

The Works tutorials moved very little: screens appeared, a few things
flashed, the narration changed. Motion is strongest when it is rare, so
every animation should answer "what should the viewer look at now?".

## Builds

```html
<p>Always visible.</p>
<p data-step>Appears on the first PgDn.</p>
<p data-step>Appears on the second PgDn.</p>
<p data-step="1">Also step 1: appears together with the first one.</p>

<ol data-build>            <!-- each item gets the next step -->
  <li>Select the numbers.</li>
  <li>Choose New Chart.</li>
</ol>

<table data-build>          <!-- every row gets the next step, header row included -->
  <tr><th>Item</th><th>Qty</th></tr>
  <tr><td>Flour</td><td>40</td></tr>
</table>

<table>                     <!-- header always visible: step the body rows yourself -->
  <tr><th>Item</th><th>Qty</th></tr>
  <tr data-step><td>Flour</td><td>40</td></tr>
  <tr data-step><td>Sugar</td><td>12</td></tr>
</table>
```

In a table, the caption and the grid lines are always visible; hidden rows
show as empty cells until their step.

- **Numbering:** `data-step="N"` shows the element from step N on. An empty
  `data-step` means "the step after the highest one so far", so plain
  `data-step` attributes number themselves in document order.
- **Builds:** `data-build` gives each child element the next step: list
  items, or table rows (the header row too; the caption stays).
- **Source order, not screen order:** empty `data-step` and `data-build`
  items count on from the highest step seen so far *in the HTML*. In
  the example below, the list comes after an explicit step 3, so its items
  are steps 4 and 5, not 1 and 2:

  ```html
  <div class="picture">… <span data-step="3">Drive A: ►</span> …</div>
  <ol data-build><li>Insert the disk</li><li>Type COPY</li></ol>  <!-- steps 4, 5 -->
  ```

  To keep a picture and a list in sync, number every element explicitly
  (`<li data-step="1">`, `<span data-step="1">`) and skip `data-build`, or
  put the list first in the source.
- **Nesting:** a step inside a stepped element never shows before its parent.
- **Charts** step as a whole: `data-step` on the `<table data-chart>` works,
  but steps on its rows are ignored.
- **Navigation:**
  - PgDn shows the next step, then goes to the next slide;
  - PgUp hides the last step, then goes to the previous slide, shown fully
    built;
  - Home and End jump straight to the first or last slide.
- **Space:** hidden elements keep their space, so nothing moves when they
  appear. Design the finished slide first, then choose what to hold back.

What to build:
- Numbered steps of a procedure, one per PgDn: the viewer reads along with you.
- The answer after the question; the result after the input.
- Table rows when the order tells a story (week 1, week 2, …).
- A callout or stamp pointing at something that is already on screen.

What not to build:
- Titles, narration that must be read first, or every bullet on every slide.
  A deck where each slide needs five presses feels slow.
- Things whose position depends on what comes before: everything is laid out
  in advance, so builds can't reflow.

## Transitions

```toml
# deck.toml: the default for every slide
transition = "dissolve"
```
```html
<!-- one slide: how this slide comes in -->
<meta name="transition" content="push 0.6s">
```

| Name | Default length | Use it for |
|---|---|---|
| `cut` | 0 | instant changes, rapid sequences, screen mocks that "update" |
| `dissolve` | 0.6 s | a calm deck-wide default; cells switch in random order |
| `wipe-right` | 0.4 s | moving on to the next step of a story |
| `wipe-left` | 0.4 s | going back to a previous idea |
| `wipe-down` | 0.4 s | revealing a picture or scene, like a blind going up |
| `wipe-up` | 0.4 s | a summary or results slide |
| `blinds` | 0.5 s | a change of section (8-column slats) |
| `push` | 0.4 s | "next chapter": the new slide pushes the old one out to the left |

- **Durations:** add one after the name: `"push 0.8s"`, `"blinds 300ms"`.
  Anything over a second feels slow.
- **Going back** (PgUp) plays the transition of the slide being left,
  mirrored: wipes reverse direction, push moves the other way.
- **Pick a family:** one quiet default for the deck, plus one or two special
  transitions for section changes. Mixing many kinds looks like a demo reel.

## Text effects

One `data-effect` per element; nest elements to combine (a `span` with
`rainbow` inside a `p` with `typewriter`, say).

### Typewriter

```html
<p data-effect="typewriter">You run the Harbor Street Bakery.</p>
<p data-step data-effect="typewriter">In this lesson you'll plan the order.</p>
```

- **Speed:** 30 characters a second, from when the element's step appears
  (slide entry for step 0).
  - A 45-character line takes 1.5 s; a 4-line panel (~180 characters) takes 6 s.
  - Keep typewritten text to 2–3 lines, or split it across steps as above.
- **Going back:** PgUp shows typewritten text complete, not retyped.
- **Use it for** narration, a "terminal output" moment, or one line that
  should be read word by word. A typed command is a `span` inside a `<pre>`,
  so the prompt is there first and the command types after it:

  ```html
  <pre class="screen">C:\&gt;<span data-step data-effect="typewriter">COPY *.* A:</span>
  <span data-step>        3 file(s) copied</span></pre>
  ```
- **Avoid it** on long paragraphs, titles, or several elements at the same
  step: they type at the same time.

### Marquee

```html
<div style="background: cyan; color: black" data-effect="marquee">
  Weekly specials: rye 2 for 1 • croissants after 10am • closed Sunday
</div>
```

- One line (the engine makes the text `nowrap`) scrolls right to left at
  10 cells a second, inside the element's content box, and loops with a
  gap.
- Give the box a background so the strip reads as a ticker.
- **Use it for** news tickers, a scrolling status line, a shop sign. One per
  slide at most.

### Rainbow

```html
<p>All orders are in. <span data-effect="rainbow">Well done!</span></p>
```

- Six bright colors by column, moving 8 columns a second.
- It's loud: use it for a celebration or a joke, on a few words. It works on
  big text too (`<h1 data-effect="rainbow">`).

### Blink

```html
<p style="text-decoration: blink">Press <kbd>PgDn</kbd> to begin</p>
```

- On and off every 267 ms, drawn by the engine, so every terminal shows it.
  Highlighted text (with a background) keeps its background while the text
  blinks.
- **Use it for** the one thing the viewer must not miss: a prompt, a warning,
  a cursor. Never more than one blinking thing per slide.

Effects change text only; the element's border, background and fill stay
still. So a marquee box or a blinking prompt in a panel keeps its frame.

## Frame animation

```html
<div data-frames data-fps="4" style="width: 10; border: single; padding: 0 1">
  <pre>[■   ]</pre>
  <pre>[■■  ]</pre>
  <pre>[■■■ ]</pre>
  <pre>[■■■■]</pre>
</div>
```

- **How it works:** `data-frames` lays out its child elements in the same
  place and shows one at a time, looping, at `data-fps` frames a second
  (default 4).
- **Size:** the container is as big as the largest frame. Make frames the
  same size (pad lines with spaces) so the box doesn't seem to jump.
- **Speed:**
  - 2–4 fps for blinking cursors and slow cycles;
  - 6–10 fps for motion (spinners, a walking figure, the ░▒▓█ sweep in the
    engine's demo deck).
- **Frames:** any element can be a frame: a `pre` of block art, a styled
  `div`, an `img`. Animated GIFs don't play yet, so split them into frames.
- **Uses:** a spinner (`| / - \`), a progress bar, a typing cursor (`█` and
  a blank), a sign that flashes between two colors, a small character
  walking.
- **Steps and placement:** `data-frames` combines with `data-step` and
  `position: absolute` on the same element. That gives a drive light that
  starts flickering at step 3:

  ```html
  <div data-step="3" data-frames data-fps="6" style="position: absolute; left: 30; top: 18">
    <span style="color: lightred">■</span>
    <span style="color: black">■</span>
  </div>
  ```

## Pacing and restraint

- **Per slide:** at most one thing moving all the time (marquee, frames,
  blink). Builds and a typewriter come and finish.
- **Per deck:** one default transition, a few special ones, some build
  slides, and one or two playful moments (rainbow, frames).
- **Time:** a presenter talks for 20–60 s per slide. Typewriter and builds
  should finish well within that; nobody should wait on an animation.
- **Order of attention:** narration first, then the visual, then details. If
  the narration is typewritten, show the visual at once or on step 1, not
  before the reader is done.

## Checking motion with the preview script

```bash
scripts/preview.sh <deck> 4 --steps 0     # the slide as it first appears
scripts/preview.sh <deck> 4 --steps 2     # after two PgDn presses
scripts/preview.sh <deck> 2 --wait 0.5    # typewriter mid-way
scripts/preview.sh <deck> 2 --wait 5      # typewriter finished
```

- Pressing PgDn past the last step goes to the next slide, which shows the
  `n/N` in the footer change: a quick way to count a slide's steps.
- A capture right after a slide change can catch the transition halfway; the
  default 1 s wait is longer than every default transition.
- Blinking text appears in some captures and not in others; marquee and
  frames show a different moment each time.

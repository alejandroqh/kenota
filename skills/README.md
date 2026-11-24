# Claude Code skill

`kenota/` is a Claude Code skill that teaches Claude to write kenota
decks in the MS Works tutorial style: the look, every supported component,
layout on the 80×25 grid, and animation. Its scripts wrap `kenota check` and
`kenota render`, so Claude can check its own work and see each slide as text.

```
kenota/
├── SKILL.md                    workflow, style summary, layout rules, gotchas
├── references/
│   ├── works-style.md          the Works look: palette, compositions, narration, block pictures
│   ├── components.md           every element, CSS property and attribute, plus copy-paste pieces
│   └── animation.md            builds, transitions, text effects, frames, pacing
├── assets/starter-deck/        Works kit style.css, deck.toml and four example slides
└── scripts/
    ├── preview.sh              a slide as a numbered text grid or color grids (via `render`)
    ├── sheet.sh                every slide of a deck in one picture
    ├── pixelart.py             a grid of color letters into a PNG picture
    ├── check.sh                list a deck's problems (via `check`)
    ├── engine.sh               find the engine, building it from this repository
    └── colormap.py             turn colored output into numbered grids
```

The repository is also a Claude Code plugin marketplace
(`.claude-plugin/`), so the simplest install is one command, after
`cargo install kenota`:

```bash
claude plugin install kenota --marketplace alejandroqh/kenota
```

Then ask Claude for a deck ("make a kenota deck about…"); the skill loads by
itself. To work on the skill itself, link this folder into your personal
skills folder instead, so edits take effect right away:

```bash
ln -s "$PWD/skills/kenota" ~/.claude/skills/kenota
```

The scripts find the engine with `engine.sh`: `$KENOTA`, then this
repository built with cargo (when the skill is here or linked from here),
then `kenota` on `PATH`, then the repository copy that a plugin install
brings along, built with cargo. `preview.sh`, `sheet.sh` and `pixelart.py`
need `python3` (nothing beyond its standard library).

The starter deck is checked by `tests/demo_deck.rs`, so engine changes that
break it fail the tests.

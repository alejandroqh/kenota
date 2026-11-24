# Claude Code skill

`dos-kenota/` is a Claude Code skill that teaches Claude to write dos-kenota
decks in the MS Works tutorial style: the look, every supported component,
layout on the 80×25 grid, and animation. Its scripts wrap `dos-kenota check` and
`dos-kenota render`, so Claude can check its own work and see each slide as text.

```
dos-kenota/
├── SKILL.md                    workflow, style summary, layout rules, gotchas
├── references/
│   ├── works-style.md          the Works look: palette, compositions, narration, block pictures
│   ├── components.md           every element, CSS property and attribute, plus copy-paste pieces
│   └── animation.md            builds, transitions, text effects, frames, pacing
├── assets/starter-deck/        Works kit style.css, deck.toml and four example slides
└── scripts/
    ├── preview.sh              a slide as a numbered text grid or color grids (via `render`)
    ├── check.sh                list a deck's problems (via `check`)
    ├── engine.sh               find the engine, building it from this repository
    └── colormap.py             turn colored output into numbered grids
```

Install it for every project by linking it into your personal skills folder:

```bash
ln -s "$PWD/skills/dos-kenota" ~/.claude/skills/dos-kenota
```

Or, for this repository only, link it into `.claude/skills/` instead. Then ask
Claude for a deck ("make a dos-kenota deck about…"); the skill loads by
itself. The scripts build the engine from this repository when run from here,
or use `$DOS_KENOTA` or a `dos-kenota` on `PATH`; `preview.sh` needs `python3`.

The starter deck is checked by `tests/demo_deck.rs`, so engine changes that
break it fail the tests.

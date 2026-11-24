# Claude Code skill

`dos-kenota/` is a Claude Code skill that teaches Claude to write dos-kenota
decks in the MS Works tutorial style: the look, every supported component,
layout on the 80×25 grid, and animation. It includes a script that shows each
slide as text, so Claude can check its own work.

```
dos-kenota/
├── SKILL.md                    workflow, style summary, layout rules, gotchas
├── references/
│   ├── works-style.md          the Works look: palette, compositions, narration, block pictures
│   ├── components.md           every element, CSS property and attribute, plus copy-paste pieces
│   └── animation.md            builds, transitions, text effects, frames, pacing
├── assets/starter-deck/        Works kit style.css, deck.toml and four example slides
└── scripts/
    ├── preview.sh              run a slide in a hidden tmux window and print it
    └── colormap.py             turn a colored capture into numbered color grids
```

Install it for every project by linking it into your personal skills folder:

```bash
ln -s "$PWD/skills/dos-kenota" ~/.claude/skills/dos-kenota
```

Or, for this repository only, link it into `.claude/skills/` instead. Then ask
Claude for a deck ("make a dos-kenota deck about…"); the skill loads by
itself. `preview.sh` needs `tmux`; it builds the engine from this repository
when run from here, or uses `$DOS_KENOTA` or a `dos-kenota` on `PATH`.

The starter deck is checked by `tests/demo_deck.rs`, so engine changes that
break it fail the tests.

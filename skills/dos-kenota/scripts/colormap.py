#!/usr/bin/env python3
"""Reads a colored screen capture of a dos-kenota slide (tmux capture-pane -e)
on stdin and prints grids with row and column numbers: the text, the
background color of every cell, and the foreground color of every cell.

    colormap.py [--grid text,bg,fg] [--rows FIRST-LAST]

--grid picks which grids to print (default all three); --rows limits them to
some canvas rows, e.g. `--rows 5-12` or `--rows 7`.

Colors are EGA numbers in hex, so each cell stays one character wide:

    0 black   4 red       8 darkgray    c lightred
    1 blue    5 magenta   9 lightblue   d lightmagenta
    2 green   6 brown     a lightgreen  e yellow
    3 cyan    7 lightgray b lightcyan   f white

Blinking text is missing from a capture taken in its "off" phase; capture
again to see it.

The border tmux draws around the 80-column canvas is cropped away, so row and
column numbers are canvas cells: the same numbers as CSS `top` and `left`
(row 0 is the first row, and the footer is rows 23 and 24 on an 80×25 deck).
"""

import argparse
import re
import sys

EGA = [
    (0x00, 0x00, 0x00), (0x00, 0x00, 0xAA), (0x00, 0xAA, 0x00), (0x00, 0xAA, 0xAA),
    (0xAA, 0x00, 0x00), (0xAA, 0x00, 0xAA), (0xAA, 0x55, 0x00), (0xAA, 0xAA, 0xAA),
    (0x55, 0x55, 0x55), (0x55, 0x55, 0xFF), (0x55, 0xFF, 0x55), (0x55, 0xFF, 0xFF),
    (0xFF, 0x55, 0x55), (0xFF, 0x55, 0xFF), (0xFF, 0xFF, 0x55), (0xFF, 0xFF, 0xFF),
]
# ANSI color order (black red green yellow blue magenta cyan white) → EGA.
ANSI = [0, 4, 2, 6, 1, 5, 3, 7]
SGR = re.compile(r"\x1b\[([0-9;]*)m")


def nearest(r, g, b):
    return min(range(16), key=lambda i: sum((x - y) ** 2 for x, y in zip(EGA[i], (r, g, b))))


def xterm(n):
    """An xterm 256-color index as an EGA number."""
    if n < 8:
        return ANSI[n]
    if n < 16:
        return ANSI[n - 8] + 8
    if n < 232:
        n -= 16
        levels = [0, 95, 135, 175, 215, 255]
        return nearest(levels[n // 36], levels[n // 6 % 6], levels[n % 6])
    gray = 8 + (n - 232) * 10
    return nearest(gray, gray, gray)


def apply(params, state):
    codes = [int(p) if p else 0 for p in params.split(";")] if params else [0]
    i = 0
    while i < len(codes):
        c = codes[i]
        if c == 0:
            state.update(fg=None, bg=None)
        elif c in (38, 48) and i + 1 < len(codes):
            key = "fg" if c == 38 else "bg"
            if codes[i + 1] == 2 and i + 4 < len(codes):
                state[key] = nearest(*codes[i + 2 : i + 5])
                i += 4
            elif codes[i + 1] == 5 and i + 2 < len(codes):
                state[key] = xterm(codes[i + 2])
                i += 2
        elif c == 39:
            state["fg"] = None
        elif c == 49:
            state["bg"] = None
        elif 30 <= c <= 37:
            state["fg"] = ANSI[c - 30]
        elif 90 <= c <= 97:
            state["fg"] = ANSI[c - 90] + 8
        elif 40 <= c <= 47:
            state["bg"] = ANSI[c - 40]
        elif 100 <= c <= 107:
            state["bg"] = ANSI[c - 100] + 8
        i += 1


def cells(lines):
    """Each captured line as a list of (char, fg, bg)."""
    state = {"fg": None, "bg": None}
    rows = []
    for line in lines:
        row = []
        pos = 0
        for m in SGR.finditer(line):
            row.extend((ch, state["fg"], state["bg"]) for ch in line[pos : m.start()])
            apply(m.group(1), state)
            pos = m.end()
        row.extend((ch, state["fg"], state["bg"]) for ch in line[pos:])
        rows.append(row)
    return rows


def crop(rows):
    """Drops the border around the canvas, when there is one."""
    text = ["".join(c[0] for c in r) for r in rows]
    if text and text[0].startswith("┌"):
        last = max(i for i, t in enumerate(text) if t.startswith("└"))
        return [r[1:-1] if len(r) >= 2 else r for r in rows[1:last]]
    return rows


def color(value):
    return "." if value is None else "0123456789abcdef"[value]


def show(title, rows, pick, first, last):
    width = max((len(r) for r in rows), default=0)
    print(title)
    print("    " + "".join(str(x // 10) if x % 10 == 0 else " " for x in range(width)))
    print("    " + "".join(str(x % 10) for x in range(width)))
    for y, row in enumerate(rows):
        if first <= y <= last:
            print(f"{y:>2}  " + "".join(pick(c) for c in row))
    print()


def row_range(text):
    first, _, last = text.partition("-")
    return int(first), int(last or first)


GRIDS = {
    "text": ("text", lambda c: c[0]),
    "bg": ("background", lambda c: color(c[2])),
    "fg": ("foreground (only cells with text)",
           lambda c: color(c[1]) if c[0].strip() else " "),
}


def main():
    parser = argparse.ArgumentParser(description="Color grids of a dos-kenota capture.")
    parser.add_argument("--grid", default="text,bg,fg",
                        help="comma-separated: text, bg, fg (default: all)")
    parser.add_argument("--rows", type=row_range, default=(0, 999),
                        help="canvas rows to show: FIRST-LAST or one row")
    args = parser.parse_args()
    names = [name.strip() for name in args.grid.split(",") if name.strip()]
    unknown = [name for name in names if name not in GRIDS]
    if unknown:
        parser.error(f"unknown grid {', '.join(unknown)} (grids: text, bg, fg)")

    rows = crop(cells(sys.stdin.read().splitlines()))
    first, last = args.rows
    for name in names:
        title, pick = GRIDS[name]
        show(title, rows, pick, first, last)
    if any(name != "text" for name in names):
        print("0 black  1 blue  2 green  3 cyan  4 red  5 magenta  6 brown  7 lightgray")
        print("8 darkgray  9 lightblue  a lightgreen  b lightcyan  c lightred  d lightmagenta  e yellow  f white")


if __name__ == "__main__":
    main()

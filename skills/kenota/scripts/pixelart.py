#!/usr/bin/env python3
"""Turns a drawing made of letters into a PNG picture for kenota, in the 16
EGA colors.

    pixelart.py DRAWING.txt [OUT.png]

The drawing has one letter per pixel. kenota draws pictures at two pixels
per cell, so a 14 × 14 drawing takes 14 columns and 7 rows. Lower case is
the dark color and upper case its bright version, as bold does to text:

    k black       K darkgray        .  transparent (a space works too)
    b blue        B lightblue
    g green       G lightgreen
    c cyan        C lightcyan
    r red         R lightred
    m magenta     M lightmagenta
    y brown       Y yellow
    w lightgray   W white

Lines starting with # are comments, and blank lines at the start and end
are dropped (use a row of dots for a transparent row there). Short rows are
padded with transparent pixels, since editors drop trailing spaces. An odd
height gets a transparent row at the bottom: it would be half a row, and
kenota would stretch the picture and blend its colors.

OUT defaults to the drawing's name with .png. The script prints the size in
cells and the <img> tag to use: its width is the width in pixels, so the
picture is never scaled.
"""

import struct
import sys
import zlib
from pathlib import Path

COLORS = {
    "k": (0x00, 0x00, 0x00), "K": (0x55, 0x55, 0x55),
    "b": (0x00, 0x00, 0xAA), "B": (0x55, 0x55, 0xFF),
    "g": (0x00, 0xAA, 0x00), "G": (0x55, 0xFF, 0x55),
    "c": (0x00, 0xAA, 0xAA), "C": (0x55, 0xFF, 0xFF),
    "r": (0xAA, 0x00, 0x00), "R": (0xFF, 0x55, 0x55),
    "m": (0xAA, 0x00, 0xAA), "M": (0xFF, 0x55, 0xFF),
    "y": (0xAA, 0x55, 0x00), "Y": (0xFF, 0xFF, 0x55),
    "w": (0xAA, 0xAA, 0xAA), "W": (0xFF, 0xFF, 0xFF),
}
CLEAR = ".  "


def read_drawing(path):
    lines = [line.rstrip("\n") for line in path.read_text(encoding="utf-8").splitlines()]
    lines = [line for line in lines if not line.startswith("#")]
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    if not lines:
        sys.exit(f"{path}: the drawing is empty")
    for y, line in enumerate(lines):
        for x, ch in enumerate(line):
            if ch not in COLORS and ch not in CLEAR:
                sys.exit(f"{path}: row {y + 1}, column {x + 1}: '{ch}' is not a color "
                         "letter (kbgcrmyw, upper case for bright, . for transparent)")
    width = max(len(line) for line in lines)
    rows = [line.ljust(width, ".") for line in lines]
    if len(rows) % 2:
        rows.append("." * width)
        print(f"{path}: odd height, added a transparent row at the bottom", file=sys.stderr)
    return rows


def write_png(path, rows):
    width, height = len(rows[0]), len(rows)
    raw = bytearray()
    for row in rows:
        raw.append(0)  # no filter
        for ch in row:
            raw.extend(COLORS[ch] + (255,) if ch in COLORS else (0, 0, 0, 0))

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)  # 8-bit RGBA
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
                     + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))
    return width, height


def main():
    if len(sys.argv) not in (2, 3) or sys.argv[1] in ("-h", "--help"):
        print(__doc__.strip())
        sys.exit(2)
    drawing = Path(sys.argv[1])
    out = Path(sys.argv[2]) if len(sys.argv) == 3 else drawing.with_suffix(".png")
    width, height = write_png(out, read_drawing(drawing))
    src = f"img/{out.name}" if out.resolve().parent.name == "img" else out.name
    print(f"{out}: {width} × {height} pixels, {width} columns × {height // 2} rows")
    print(f'<img src="{src}" width="{width}" alt="">')


if __name__ == "__main__":
    main()

#!/usr/bin/env bash
# Draws every slide of a deck into one PNG, to look at the whole deck at
# once: the rhythm of layouts, and slides whose margins or colors drift
# from the rest.
#
#   sheet.sh <deck> <FILE.png> [--cols N] [--step N]
#
#   --cols N   slides per row (default 3)
#   --step N   build step for every slide (default: the last, all showing;
#              0 shows how each slide first appears)
#
# Slides keep their real size, 640×400 for an 80×25 deck, with a dark grey
# gap between them. To look closely at one slide, use preview.sh --png.
# The engine is found by engine.sh next to this script.
set -euo pipefail

usage() {
  sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
deck=""
out=""
cols=3
step_args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --cols) cols="${2:?--cols needs a number}"; shift 2 ;;
    --step) step_args=(--step "${2:?--step needs a number}"); shift 2 ;;
    -h|--help) usage ;;
    -*) echo "unknown option $1" >&2; usage ;;
    *)
      if [ -z "$deck" ]; then deck="$1"
      elif [ -z "$out" ]; then out="$1"
      else usage
      fi
      shift ;;
  esac
done
[ -n "$deck" ] && [ -n "$out" ] || usage

engine="$("$here/engine.sh")"
# `check` ends with "<deck>: N slides, ..." even when it finds problems.
count="$("$engine" check "$deck" 2>&1 | grep -oE '[0-9]+ slides?' | tail -1 | grep -oE '[0-9]+' || true)"
[ -n "$count" ] || { echo "can't read the number of slides of $deck" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
files=()
for i in $(seq 1 "$count"); do
  "$engine" render "$deck" --slide "$i" ${step_args[@]+"${step_args[@]}"} --png "$tmp/$i.png"
  files+=("$tmp/$i.png")
done

python3 - "$out" "$cols" "${files[@]}" <<'PY'
# Tiles kenota's PNGs (4-bit palette, all with the EGA palette) into one.
import struct
import sys
import zlib

def read_png(path):
    data = open(path, "rb").read()
    pos, idat, palette = 8, b"", None
    while pos < len(data):
        size, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + size]
        if kind == b"IHDR":
            width, height, depth, color = struct.unpack(">IIBB", body[:10])
            if (depth, color) != (4, 3):
                sys.exit(f"{path}: not a kenota picture (4-bit palette)")
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat += body
        pos += 12 + size
    raw = zlib.decompress(idat)
    stride = (width + 1) // 2
    rows, prev = [], bytearray(stride)
    for y in range(height):
        kind = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for x in range(stride):
            a = line[x - 1] if x else 0
            b = prev[x]
            c = prev[x - 1] if x else 0
            if kind == 1:
                line[x] = (line[x] + a) & 255
            elif kind == 2:
                line[x] = (line[x] + b) & 255
            elif kind == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        prev = line
        pixels = bytearray()
        for byte in line:
            pixels += bytes((byte >> 4, byte & 15))
        rows.append(pixels[:width])
    return width, height, palette, rows

out, cols, paths = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
slides = [read_png(p) for p in paths]
w, h, palette = slides[0][0], slides[0][1], slides[0][2]
colors = [palette[i:i + 3] for i in range(0, len(palette), 3)]
gap_color = colors.index(b"\x55\x55\x55") if b"\x55\x55\x55" in colors else 0
gap = 16
cols = max(1, min(cols, len(slides)))
lines = (len(slides) + cols - 1) // cols
width, height = cols * w + (cols + 1) * gap, lines * h + (lines + 1) * gap
canvas = [bytearray([gap_color]) * width for _ in range(height)]
for n, (_, _, _, rows) in enumerate(slides):
    left = gap + (n % cols) * (w + gap)
    top = gap + (n // cols) * (h + gap)
    for y, row in enumerate(rows):
        canvas[top + y][left:left + w] = row

raw = bytearray()
for row in canvas:
    raw.append(0)
    padded = row + bytearray(len(row) % 2)
    raw += bytes((padded[i] << 4) | padded[i + 1] for i in range(0, len(padded), 2))

def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

with open(out, "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 4, 3, 0, 0, 0))
            + chunk(b"PLTE", palette)
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
            + chunk(b"IEND", b""))
print(f"{out}: {len(slides)} slide{'s' if len(slides) != 1 else ''}, {width}×{height}")
PY

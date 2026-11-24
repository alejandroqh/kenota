#!/usr/bin/env bash
# Shows a kenota slide as text you can read: by default a grid of the
# characters with row and column numbers that match CSS `top`/`left`.
#
#   preview.sh <deck> [slide] [--step N] [--time SECONDS]
#              [--map | --grid text,bg,fg] [--rows FIRST-LAST] [--color | --plain]
#              [--png FILE]
#
#   slide       slide number, from 1 (default 1)
#   --step N    build step to show, from 0 (default: the last, all showing)
#   --time S    seconds since that step appeared, to catch a typewriter or
#               another effect mid-way (default: typing done, effects at rest)
#   --map       all three grids: text, background colors, foreground colors
#               (colors are EGA numbers in hex: 1 blue, 7 lightgray, f white)
#   --grid G    the grids to print, comma-separated: text, bg, fg
#   --rows R    only canvas rows R (`5-12` or `7`)
#   --color     the raw output of `kenota render`, with color codes
#   --plain     plain text without numbers or colors
#   --png FILE  save the slide as a picture (1280×800) and print its path
#
# The engine is found by engine.sh next to this script.
set -euo pipefail

usage() {
  sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
deck=""
slide=1
render_args=()
grid="text"
rows="0-999"
raw=""
png=""
while [ $# -gt 0 ]; do
  case "$1" in
    --step) render_args+=(--step "${2:?--step needs a number}"); shift 2 ;;
    --time) render_args+=(--time "${2:?--time needs seconds}"); shift 2 ;;
    --map) grid="text,bg,fg"; shift ;;
    --grid) grid="${2:?--grid needs text, bg or fg}"; shift 2 ;;
    --rows) rows="${2:?--rows needs FIRST-LAST}"; shift 2 ;;
    --color) raw="truecolor"; shift ;;
    --plain) raw="none"; shift ;;
    --png) png="${2:?--png needs a file}"; shift 2 ;;
    -h|--help) usage ;;
    -*) echo "unknown option $1" >&2; usage ;;
    *)
      if [ -z "$deck" ]; then deck="$1"
      else slide="$1"
      fi
      shift ;;
  esac
done
[ -n "$deck" ] || usage

engine="$("$here/engine.sh")"
render() {
  "$engine" render "$deck" --slide "$slide" ${render_args[@]+"${render_args[@]}"} --colors "$1"
}
if [ -n "$png" ]; then
  "$engine" render "$deck" --slide "$slide" ${render_args[@]+"${render_args[@]}"} \
    --png "$png" --scale 2
  echo "$png"
elif [ -n "$raw" ]; then
  render "$raw"
else
  render truecolor | python3 "$here/colormap.py" --grid "$grid" --rows "$rows"
fi

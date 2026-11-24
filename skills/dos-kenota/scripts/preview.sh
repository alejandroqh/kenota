#!/usr/bin/env bash
# Shows a dos-kenota slide as text, so you can check a layout without a
# human looking at the screen. The deck runs in a detached tmux window the
# size of the canvas plus its border (82×27), the screen is captured, and the
# window is closed again.
#
#   preview.sh <deck> [slide] [--steps N] [--wait SECONDS]
#              [--map] [--grid text,bg,fg] [--rows FIRST-LAST] [--color]
#
#   slide       slide number, from 1 (default 1)
#   --steps N   press PgDn N times before capturing, to see build steps
#   --wait S    seconds to wait before capturing (default 1); give a
#               typewriter or a transition time to finish
#   --map       print numbered grids of the text, background colors and
#               foreground colors (EGA numbers in hex), cropped to the canvas:
#               row/column numbers match CSS top/left
#   --grid G    like --map, printing only some grids: text, bg, fg
#               (comma-separated)
#   --rows R    with --map or --grid, only canvas rows R (`5-12` or `7`)
#   --color     print the raw capture with its color escape codes
#
# The engine is found in this order: $DOS_KENOTA, the dos-kenota repository
# this skill lives in (built with cargo when needed), `dos-kenota` on PATH.
set -euo pipefail

usage() {
  sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

deck=""
slide=1
steps=0
wait=1
color=""
map=""
grid="text,bg,fg"
rows="0-999"
while [ $# -gt 0 ]; do
  case "$1" in
    --steps) steps="${2:?--steps needs a number}"; shift 2 ;;
    --wait) wait="${2:?--wait needs seconds}"; shift 2 ;;
    --color) color="-e"; shift ;;
    --map) map=1; shift ;;
    --grid) grid="${2:?--grid needs text, bg or fg}"; map=1; shift 2 ;;
    --rows) rows="${2:?--rows needs FIRST-LAST}"; shift 2 ;;
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
[ -d "$deck" ] || { echo "no deck folder at $deck" >&2; exit 1; }
command -v tmux >/dev/null || { echo "preview needs tmux" >&2; exit 1; }

find_engine() {
  if [ -n "${DOS_KENOTA:-}" ]; then
    echo "$DOS_KENOTA"
    return
  fi
  local here root
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  root="$(cd "$here/../../.." && pwd)"
  if grep -qs '^name = "dos-kenota"' "$root/Cargo.toml"; then
    (cd "$root" && cargo build --quiet < /dev/null) >&2
    echo "$root/target/debug/dos-kenota"
    return
  fi
  if command -v dos-kenota >/dev/null; then
    command -v dos-kenota
    return
  fi
  echo "dos-kenota not found: set DOS_KENOTA or put it on PATH" >&2
  exit 1
}

engine="$(find_engine)"
deck="$(cd "$deck" && pwd)"
session="dk-preview-$$"
trap 'tmux kill-session -t "$session" 2>/dev/null || true' EXIT

# `sleep` keeps the window open if the engine stops with an error, so the
# error message is what gets captured.
tmux new-session -d -s "$session" -x 82 -y 27 \
  "COLORTERM=truecolor '$engine' '$deck' --slide $slide; sleep 30"
sleep 0.5
pressed=0
while [ "$pressed" -lt "$steps" ]; do
  tmux send-keys -t "$session" PageDown
  pressed=$((pressed + 1))
  sleep 0.1
done
sleep "$wait"
if [ -n "$map" ]; then
  tmux capture-pane -p -e -t "$session" |
    python3 "$(dirname "${BASH_SOURCE[0]}")/colormap.py" --grid "$grid" --rows "$rows"
else
  tmux capture-pane -p $color -t "$session"
fi

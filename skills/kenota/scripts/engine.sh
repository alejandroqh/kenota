#!/usr/bin/env bash
# Prints the path of the kenota engine, building it first when this
# skill sits inside the kenota repository. Order: $KENOTA, the
# repository around this skill (cargo build; a linked skill counts),
# `kenota` on PATH.
set -euo pipefail

if [ -n "${KENOTA:-}" ]; then
  echo "$KENOTA"
  exit
fi
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root="$(cd "$here/../../.." && pwd)"
if grep -qs '^name = "kenota"' "$root/Cargo.toml"; then
  (cd "$root" && cargo build --quiet < /dev/null) >&2
  echo "$root/target/debug/kenota"
  exit
fi
if command -v kenota >/dev/null; then
  command -v kenota
  exit
fi
echo "kenota not found: set KENOTA or put it on PATH" >&2
exit 1

#!/usr/bin/env bash
# Prints the path of the dos-kenota engine, building it first when this
# skill sits inside the dos-kenota repository. Order: $DOS_KENOTA, the
# repository around this skill (cargo build), `dos-kenota` on PATH.
set -euo pipefail

if [ -n "${DOS_KENOTA:-}" ]; then
  echo "$DOS_KENOTA"
  exit
fi
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
if grep -qs '^name = "dos-kenota"' "$root/Cargo.toml"; then
  (cd "$root" && cargo build --quiet < /dev/null) >&2
  echo "$root/target/debug/dos-kenota"
  exit
fi
if command -v dos-kenota >/dev/null; then
  command -v dos-kenota
  exit
fi
echo "dos-kenota not found: set DOS_KENOTA or put it on PATH" >&2
exit 1

#!/usr/bin/env bash
# Prints the path of the kenota engine. Order:
#   1. $KENOTA;
#   2. the kenota repository around this skill, built with cargo, when the
#      skill is a working copy (inside the repository or linked from it);
#   3. `kenota` on PATH (cargo install kenota);
#   4. the repository copy that a plugin install brings along, built with
#      cargo, when kenota isn't installed.
set -euo pipefail

if [ -n "${KENOTA:-}" ]; then
  echo "$KENOTA"
  exit
fi
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root="$(cd "$here/../../.." && pwd)"
build() {
  (cd "$root" && cargo build --quiet < /dev/null) >&2
  echo "$root/target/debug/kenota"
  exit
}
repo=""
if grep -qs '^name = "kenota"' "$root/Cargo.toml"; then
  repo=yes
fi
# A plugin install is a copy under ~/.claude/plugins/cache.
case "$root" in
  */plugins/cache/*) plugin=yes ;;
  *) plugin="" ;;
esac
if [ -n "$repo" ] && [ -z "$plugin" ]; then
  build
fi
if command -v kenota >/dev/null; then
  command -v kenota
  exit
fi
if [ -n "$repo" ] && command -v cargo >/dev/null; then
  build
fi
echo "kenota not found: install it with 'cargo install kenota', or set KENOTA" >&2
exit 1

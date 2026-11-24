#!/usr/bin/env bash
# Lists every problem in a deck: CSS the engine ignored, unsupported tags,
# missing pictures and fonts, bad transitions and chart values, and content
# that runs off the slide area. Exits with 1 when there is any.
#
#   check.sh <deck>
set -euo pipefail
engine="$("$(dirname "${BASH_SOURCE[0]}")/engine.sh")"
exec "$engine" check "$@"

#!/bin/bash
# Run witness N times; print exit codes.
set -euo pipefail
WITNESS="${1:?}"
N="${2:-3}"
export BISECT_WITNESS="$WITNESS"
SCRIPT="$(dirname "$0")/bisect_witness.sh"
for i in $(seq 1 "$N"); do
  "$SCRIPT"
  echo "run $i exit=$?"
done

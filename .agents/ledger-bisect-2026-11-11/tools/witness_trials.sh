#!/bin/bash
# Run witness script N times at current HEAD; print each exit code.
set -euo pipefail
N="${1:-3}"
SCRIPT="$(dirname "$0")/bisect_witness_full.sh"
for i in $(seq 1 "$N"); do
  "$SCRIPT" || true
  echo "trial=$i exit=$?"
done

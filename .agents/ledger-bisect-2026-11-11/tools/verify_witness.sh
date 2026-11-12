#!/bin/bash
# Run witness N times at current HEAD; print exit codes (0=ledger-ok, 1=regressed, 125=skip).
set -euo pipefail
WITNESS="${1:?witness}"
N="${2:-3}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLS="$(dirname "$0")"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
for i in $(seq 1 "$N"); do
  bash "$TOOLS/bisect_witness.sh" || ec=$?
  ec="${ec:-0}"
  echo "run=$i exit=$ec"
  ec=0
done

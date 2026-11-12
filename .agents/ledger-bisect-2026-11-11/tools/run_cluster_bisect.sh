#!/bin/bash
# Bisect one cluster witness: find good by walking from PIN, then git bisect run.
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
if [[ -d /tmp/ledger-bisect-tools ]]; then
  DIR=/tmp/ledger-bisect-tools
fi
LOG="${3:-$ROOT/.agents/ledger-bisect-2026-11-11/work/bisect_${WITNESS//\//_}.log}"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
"$DIR/prepare_bisect_pin.sh" "$WITNESS" "$PIN" "$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_observed.txt"
BRANCH="$(git symbolic-ref -q --short HEAD || echo work/ledger-bisect-part-9)"
GOOD=$("$DIR/find_nearest_good.sh" "$WITNESS" "$PIN" "${LOG}.find_good" | tail -1)
if [[ -z "$GOOD" || "$GOOD" == OLDEST* ]]; then
  echo "no good commit" | tee -a "$LOG"
  git checkout --quiet "$BRANCH"
  exit 1
fi
echo "GOOD=$GOOD" | tee -a "$LOG"
git bisect start --first-parent "$PIN" "$GOOD" 2>&1 | tee -a "$LOG"
git bisect run "$DIR/bisect_witness.sh" 2>&1 | tee -a "$LOG"
git bisect log 2>&1 | tee -a "$LOG"
FIRST=$(grep -m1 'first bad commit' "$LOG" || true)
git bisect reset "$PIN" 2>&1 | tee -a "$LOG"
git checkout --quiet "$BRANCH"
echo "$FIRST"

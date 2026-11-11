#!/bin/bash
# Run one corpus batch (≤200 filters). Args: batch_id
set -euo pipefail
BATCH_ID="${1:?batch id}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
DIR="$ROOT/wiki/drafts/audits/corpus/batches"
FILTER_FILE="$DIR/batch_${BATCH_ID}.filter"
OUT="$ROOT/wiki/drafts/audits/corpus/batch_${BATCH_ID}.txt"
[[ -f "$FILTER_FILE" ]] || { echo "missing $FILTER_FILE"; exit 1; }
FILTER="$(tr '\n' ',' <"$FILTER_FILE" | sed 's/,$//')"
export OMEGA_CORPUS_JOBS=1
MEASURED="$(git rev-parse origin/main)"
echo "batch $BATCH_ID measured=$MEASURED"
nice -n 19 mbx xtask corpus --filter "$FILTER" < /dev/null | tee "$OUT"

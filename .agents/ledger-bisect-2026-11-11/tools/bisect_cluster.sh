#!/bin/bash
# Full cluster bisect: prepare meta, find good, bisect run, write .result
set -euo pipefail
CLUSTER_ID="${1:?}"
WITNESS="${2:?}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
LEDGER_PIN="/tmp/ledger-bisect-part27-corpus_outcomes.txt"
OBS="$ROOT/.agents/ledger-bisect-2026-11-11/part-27-premeasure.txt"
SLUG="$(echo "$WITNESS" | tr '/' '_')"
META="$ROOT/.agents/ledger-bisect-2026-11-11/pin-witness/${SLUG}.json"
mkdir -p "$(dirname "$META")"
cp "$ROOT/tests/omega/corpus_outcomes.txt" "$LEDGER_PIN"
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/prepare_pin_witness.py" \
  "$WITNESS" "$LEDGER_PIN" "$OBS" "$META"
export BISECT_WITNESS_PIN="$META"
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/bisect-${CLUSTER_ID}.log"
GOOD=$(bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/find_good_commit.sh" "$PIN" "$WITNESS" | tail -1)
[[ "$GOOD" =~ ^[0-9a-f]{40}$ ]] || { echo "invalid good=$GOOD" >&2; exit 1; }
echo "good=$GOOD" | tee "$LOG"
export BISECT_WITNESS_PIN="$META"
git checkout -f work/ledger-bisect-part-27 2>/dev/null || git checkout -f -B work/ledger-bisect-part-27 "$PIN"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD" | tee -a "$LOG"
git bisect run bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" 2>&1 | tee -a "$LOG"
FIRST_BAD=$(rg -o '^[0-9a-f]{40} is the first bad commit' "$LOG" | awk '{print $1}' || true)
git bisect reset >/dev/null
git checkout -f work/ledger-bisect-part-27
if [[ -z "$FIRST_BAD" ]]; then
  echo "bisect failed for $CLUSTER_ID" >&2
  exit 1
fi
{
  echo "$FIRST_BAD"
  git show -s --format='%s' "$FIRST_BAD"
  echo "good=$GOOD"
} >"$ROOT/.agents/ledger-bisect-2026-11-11/bisect-${CLUSTER_ID}.result"
echo "first_bad=$FIRST_BAD"

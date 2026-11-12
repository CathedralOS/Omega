#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_ID="${2:?id}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$ROOT"
git checkout -q work/ledger-bisect-part-31
git add -A .agents/ledger-bisect-2026-11-11/ 2>/dev/null || true
git diff --cached --quiet || git commit -m "repo: ledger bisect part 31 autosave before bisect" >/dev/null || true
export BISECT_WITNESS="$WITNESS"
GOOD_FILE="$ROOT/.agents/ledger-bisect-2026-11-11/cluster${CLUSTER_ID}-good.sha"
if [[ -f "$GOOD_FILE" ]]; then
  GOOD="$(tr -d ' \n' <"$GOOD_FILE")"
else
  GOOD="$(python3 "$ROOT/.agents/ledger-bisect-2026-11-11/find_good_commit.py" "$WITNESS" "$PIN")"
  echo "$GOOD" >"$GOOD_FILE"
fi
echo "cluster=$CLUSTER_ID good=$GOOD witness=$WITNESS"
git bisect reset 2>/dev/null || true
git checkout -q work/ledger-bisect-part-31
LOG="/tmp/ledger-bisect-c${CLUSTER_ID}.log"
export BISECT_WITNESS="$WITNESS"
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run bash /tmp/ledger-bisect-pin/tools/bisect_witness.sh 2>&1 | tee "$LOG"
FIRST=$(grep -m1 'first bad commit' "$LOG" | sed -n 's/.*\[\([^]]*\)\].*/\1/p')
git bisect reset
git checkout -q work/ledger-bisect-part-31
if [[ -z "$FIRST" ]]; then
  echo "bisect failed for cluster $CLUSTER_ID" >&2
  exit 1
fi
git log -1 --format='%H %s' "$FIRST"
echo "$FIRST" > "$ROOT/.agents/ledger-bisect-2026-11-11/bisect-c${CLUSTER_ID}-first-bad.txt"

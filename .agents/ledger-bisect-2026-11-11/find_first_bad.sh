#!/bin/bash
# Walk first-parent from PIN^1 toward GOOD; first rc==1 is latest breaking commit.
set -euo pipefail
WITNESS="${1:?}"
GOOD="${2:?good}"
CLUSTER_TAG="${3:?tag}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
LOG="/tmp/bisect_${CLUSTER_TAG}.log"
export BISECT_WITNESS="$WITNESS"
git checkout -q work/ledger-bisect-part-20
bash "$ROOT/.agents/ledger-bisect-2026-11-11/prepare_witness.sh" >>"$LOG"
cp "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_test.py" /tmp/bisect_tools_test.py
cp "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_witness.sh" /tmp/bisect_tools_witness.sh
cp /tmp/bisect_tools_test.py /tmp/bisect_test.py
cp /tmp/bisect_tools_witness.sh /tmp/bisect_witness.sh
chmod +x /tmp/bisect_witness.sh

cursor="$(git rev-parse "$PIN^1")"
FIRST=""
steps=0
while [[ "$cursor" != "$GOOD" ]]; do
  steps=$((steps + 1))
  git checkout -q "$cursor"
  set +e
  /tmp/bisect_witness.sh
  rc=$?
  set -e
  echo "walk step=$steps ${cursor:0:12} rc=$rc" | tee -a "$LOG"
  if [[ "$rc" -eq 1 ]]; then
    FIRST="$cursor"
    echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
    break
  fi
  if [[ "$rc" -ne 0 && "$rc" -ne 125 ]]; then
    echo "unexpected rc=$rc at $cursor" | tee -a "$LOG"
    exit 1
  fi
  cursor="$(git rev-parse "$cursor^1")"
done
[[ -n "$FIRST" ]] || { echo "no first-bad between PIN and GOOD" | tee -a "$LOG"; exit 1; }
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
git checkout -q work/ledger-bisect-part-20
echo "SUBJECT=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-none}" | tee -a "$LOG"

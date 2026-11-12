#!/bin/bash
# Continue bisect when GOOD is already known (skip find_good/verify).
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_TAG="${2:?tag}"
GOOD="${3:?good sha}"
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
git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD" 2>&1 | tee -a "$LOG"
git bisect run /tmp/bisect_witness.sh 2>&1 | tee -a "$LOG"
FIRST=$(git rev-parse HEAD)
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
git bisect reset "$PIN" 2>/dev/null || git bisect reset
git checkout -q work/ledger-bisect-part-20
echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
echo "SUBJECT=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-none}" | tee -a "$LOG"

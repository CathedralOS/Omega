#!/bin/bash
# find good + git bisect run for one witness; prints BISECT_COMMIT and good commit.
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_ID="${2:?cluster id}"
PIN="${PIN_COMMIT:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git rev-parse --show-toplevel)"
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
rm -rf /tmp/ledger-bisect-tools
cp -a "$TOOLS" /tmp/ledger-bisect-tools
cd "$ROOT"
git checkout work/ledger-bisect-part-22
export BISECT_WITNESS="$WITNESS" PIN_COMMIT="$PIN"
python3 "$TOOLS/find_good_commit.py" | tee ".agents/ledger-bisect-2026-11-11/find_good_${CLUSTER_ID}.log"
GOOD=$(cat /tmp/ledger_bisect_good.txt)
export BISECT_WITNESS="$WITNESS"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run /tmp/ledger-bisect-tools/bisect_witness.sh 2>&1 | tee ".agents/ledger-bisect-2026-11-11/bisect_${CLUSTER_ID}.log"
FIRST=$(grep -oE '[0-9a-f]{40} is the first bad commit' ".agents/ledger-bisect-2026-11-11/bisect_${CLUSTER_ID}.log" | awk '{print $1}')
git bisect reset "$PIN"
echo "BREAKING=$FIRST"
echo "GOOD=$GOOD"

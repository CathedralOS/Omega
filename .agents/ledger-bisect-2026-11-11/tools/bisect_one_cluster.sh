#!/bin/bash
# Full bisect for one witness cluster: snapshot, find good, git bisect run.
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_ID="${2:?cluster_id}"
OUT_DIR="${3:-/agent/repos/omega/.agents/ledger-bisect-2026-11-11}"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SRC_TOOLS="$(dirname "$0")"
TOOLS=/tmp/ledger_bisect_tools_2026_11_11
BRANCH_TIP="$(git -C "$ROOT" rev-parse work/ledger-bisect-part-10)"

rm -rf "$TOOLS"
cp -a "$SRC_TOOLS" "$TOOLS"
chmod +x "$TOOLS"/*.sh "$TOOLS"/*.py

cd "$ROOT"
git checkout -f work/ledger-bisect-part-10

export BISECT_WITNESS="$WITNESS"
export BISECT_PIN="$PIN"

cp "$ROOT/tests/omega/corpus_outcomes.txt" /tmp/pin_corpus_outcomes.txt 2>/dev/null || true
python3 "$TOOLS/pin_snapshot.py" "$WITNESS" /tmp/part10_pin_measure.txt >/tmp/pin_snap_${CLUSTER_ID}.json

echo "Finding good commit for $WITNESS ..." >&2
GOOD="$(python3 "$TOOLS/find_good_commit.py" "$WITNESS")"
echo "good=$GOOD" >&2

git bisect reset 2>/dev/null || true
git checkout -f "$PIN"
git bisect start --first-parent "$PIN" "$GOOD"
set +e
git bisect run bash "$TOOLS/bisect_witness.sh" 2>&1 | tee "$OUT_DIR/bisect_${CLUSTER_ID}.log"
RC=$?
set -e
FIRST_BAD=""
if [[ $RC -eq 0 ]]; then
  FIRST_BAD=$(git rev-parse HEAD^{commit})
fi
git bisect log > "$OUT_DIR/bisect_${CLUSTER_ID}_log.txt" 2>/dev/null || true
git bisect reset
git checkout -f work/ledger-bisect-part-10
git reset --hard "$BRANCH_TIP"

PR=""
MECH=""
if [[ -n "$FIRST_BAD" ]]; then
  SUBJ=$(git log -1 --format='%s' "$FIRST_BAD")
  PR=$(sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' <<<"$SUBJ" | head -1)
  MECH="$SUBJ"
fi

python3 <<PY
import json
out = {
  "cluster_id": "$CLUSTER_ID",
  "witness": "$WITNESS",
  "good": "$GOOD",
  "breaking_commit": "$FIRST_BAD",
  "pr": "$PR" or None,
  "mechanism": """$MECH"""[:300],
  "status": "bisected" if "$FIRST_BAD" and "$FIRST_BAD" != "$PIN" else "propagated",
  "bisect_exit": int("$RC"),
}
open("$OUT_DIR/results/${CLUSTER_ID}.json", "w").write(json.dumps(out, indent=2)+"\n")
print(json.dumps(out))
PY

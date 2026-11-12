#!/bin/bash
set -euo pipefail
WITNESS=$1 GOOD=$2 BAD=$3 OUT=$4
ROOT=/agent/repos/omega
cd "$ROOT"
INSTALL=/tmp/ledger-bisect-p6
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/prepare_pin_ref.py" "$WITNESS" >/dev/null
git checkout -q work/ledger-bisect-part-6
git bisect reset >/dev/null 2>&1 || true
git bisect start "$BAD" "$GOOD" --first-parent
set +e
git bisect run "$INSTALL/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit})
git bisect reset >/dev/null 2>&1 || true
git checkout -q work/ledger-bisect-part-6
PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p')
MECH=$(git log -1 --format='%s' "$BREAK")
python3 - "$OUT" <<PY
import json,sys
json.dump({"witness":"$WITNESS","breaking_commit":"$BREAK","pr":"$PR","mechanism":"$MECH","status":"bisected","good_commit":"$GOOD","bad_range_end":"$BAD","method":"pin_frozen_narrow"},open(sys.argv[1]),indent=2)
PY
echo "break=$BREAK pr=$PR"

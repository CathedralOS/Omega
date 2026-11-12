#!/bin/bash
# Bisect one witness: BAD=PIN, GOOD from ledger stamp. Writes JSON path in $2.
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
PIN="${BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLS="$(dirname "$0")"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
GOOD="${BISECT_GOOD:-}"
if [[ -z "$GOOD" ]]; then
  GOOD="$("$TOOLS/ledger_good_commit.py" "$WITNESS" || true)"
fi
if [[ -z "$GOOD" ]]; then
  echo "no good commit for $WITNESS" >&2
  exit 2
fi
echo "witness=$WITNESS good=$GOOD bad=$PIN" >&2
export GIT_PAGER=cat
git bisect reset 2>/dev/null || true
git checkout -f work/ledger-bisect-part-10 2>/dev/null || true
if ! git bisect start --first-parent "$PIN" "$GOOD"; then
  git bisect reset 2>/dev/null || true
  git bisect start
  git bisect bad "$PIN"
  git bisect good "$GOOD"
fi
set +e
git bisect run bash "$TOOLS/bisect_witness.sh" 2>&1 | tee "${OUT_JSON}.log"
RC=$?
set -e
BREAK=""
if [[ $RC -eq 0 ]]; then
  BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || true)
fi
git bisect log > "${OUT_JSON}.bisect_log" 2>/dev/null || true
git bisect reset
git checkout -f work/ledger-bisect-part-10
PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH=$(git log -1 --format='%s' "$BREAK" 2>/dev/null)
else
  MECH=""
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$MECH" "$RC" "$GOOD" "$PIN" <<'PY'
import json, sys
out, witness, brk, pr, mech, rc, good, pin = sys.argv[1:9]
status = "bisected" if rc in (0, 1) and brk and brk != pin else ("propagated" if not brk else "bisected")
open(out, "w").write(json.dumps({
    "witness": witness,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:300],
    "status": status,
    "bisect_exit": int(rc),
    "good": good,
    "bad": pin,
}, indent=2))
print(json.dumps({"witness": witness, "breaking_commit": brk, "status": status}))
PY

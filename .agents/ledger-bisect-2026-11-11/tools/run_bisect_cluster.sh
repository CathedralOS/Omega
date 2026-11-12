#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness fixture}"
OUT_JSON="${2:?output json}"
OBS_RECORDS="${3:-.agents/ledger-bisect-2026-11-11/part17_pin_records.txt}"
PIN="${PIN_COMMIT:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLDIR="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
"$TOOLDIR/prepare_witness_pin.sh" "$WITNESS" "$ROOT/$OBS_RECORDS"
GOOD=$("$TOOLDIR/find_bisect_good.sh")
git checkout -q "$PIN"
export GIT_PAGER=cat
git bisect reset 2>/dev/null || true
git bisect start "$PIN" "$GOOD" --first-parent
set +e
git bisect run "$TOOLDIR/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || echo "")
git bisect log > "${OUT_JSON}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
git checkout -q work/ledger-bisect-part-17 2>/dev/null || git checkout -q "$PIN"
PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$RC" "$GOOD" "$PIN" <<'PY'
import json, subprocess, sys
out, witness, brk, pr, rc, good, pin = sys.argv[1:8]
mech = ""
if brk:
    subj = subprocess.run(
        ["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True
    ).stdout.strip()
    stat = subprocess.run(
        ["git", "show", brk, "--stat", "--format="], capture_output=True, text=True
    ).stdout.strip().replace("\n", "; ")[:240]
    mech = f"{subj}" + (f" — touched: {stat}" if stat else "")
status = "bisected" if rc in (0, 1) and brk and brk != pin else "propagated"
if brk == pin:
    status = "propagated"
open(out, "w").write(json.dumps({
    "witness": witness,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:400],
    "status": status,
    "bisect_exit": int(rc),
    "good_commit": good,
    "bad_commit": pin,
}, indent=2))
print(status, brk[:12] if brk else "?", "good", good[:12], "rc", rc)
PY

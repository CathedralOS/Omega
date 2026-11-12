#!/bin/bash
# Bisect one witness: PIN observation vs fixed PIN ledger; find good by walking first-parent back.
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${BISECT_BAD:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
OUT_DIR="$ROOT/.agents/ledger-bisect-2026-11-11/bisect_results"
MEASURE="${BISECT_MEASURE:-$ROOT/.agents/ledger-bisect-2026-11-11/slice_2_pin_measure.txt}"
mkdir -p "$OUT_DIR"
SAFE="${WITNESS//\//_}"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1 || true
START_SHA="$(git rev-parse HEAD)"

python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_snapshot.py" "$WITNESS" "$MEASURE"

run_test_at() {
  local rev="$1"
  git -c submodule.recurse=false checkout --detach "$rev" >/dev/null 2>&1 || return 125
  nice -n 19 "$SCRIPT" 2>/dev/null
}

echo "find_good from $PIN" >&2
GOOD=""
cur="$(git rev-parse "${PIN}^1")"
while [[ -n "$cur" ]]; do
  run_test_at "$cur" || true
  rc=$?
  echo "  $cur -> $rc" >&2
  if [[ "$rc" -eq 0 ]]; then
    GOOD="$cur"
    break
  fi
  cur="$(git rev-parse "${cur}^1" 2>/dev/null)" || break
done
git checkout --detach "$START_SHA" >/dev/null 2>&1
[[ -n "$GOOD" ]] || {
  echo "ERROR: no good first-parent ancestor for $WITNESS" >&2
  exit 2
}

run_test_at "$PIN" || true
pin_rc=$?
run_test_at "$GOOD" || true
good_rc=$?
git checkout --detach "$START_SHA" >/dev/null 2>&1
[[ "$good_rc" -eq 0 && "$pin_rc" -eq 1 ]] || {
  echo "ERROR: endpoints good=$good_rc($GOOD) pin=$pin_rc($PIN)" >&2
  exit 2
}

STABLE=1
for _ in 1 2; do
  run_test_at "$GOOD" || true
  g=$?
  run_test_at "$PIN" || true
  p=$?
  [[ "$g" -eq 0 && "$p" -eq 1 ]] || STABLE=0
done
git checkout --detach "$START_SHA" >/dev/null 2>&1

git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD"
set +e
nice -n 19 git bisect run "$SCRIPT" >"$OUT_DIR/${SAFE}.log" 2>&1
RC=$?
set -e
BREAK=$(git rev-parse HEAD 2>/dev/null || true)
git bisect reset 2>/dev/null || true
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1

PR=""
MECH=""
STATUS=propagated
if [[ "$RC" -le 1 && -n "$BREAK" && "$BREAK" != "$PIN" && "$BREAK" != "$GOOD" ]]; then
  STATUS=bisected
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
stat = subprocess.run(["git", "show", "--stat", "--format=", brk], capture_output=True, text=True).stdout
lines = [l.strip() for l in stat.splitlines() if l.strip() and "|" in l][:4]
subj = subprocess.run(["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True).stdout.strip()
print(subj + (f" — {'; '.join(lines)}" if lines else ""))
PY
)
fi
[[ "$STABLE" -eq 0 ]] && MECH="${MECH}; endpoint stability: flaky across replays"

python3 - "$OUT_DIR/${SAFE}.json" "$WITNESS" "$GOOD" "$PIN" "$BREAK" "$PR" "$MECH" "$STATUS" "$STABLE" "$RC" <<'PY'
import json, sys
_, out, witness, good, pin, brk, pr, mech, status, stable, rc = sys.argv
json.dump(
    {
        "witness": witness,
        "good": good,
        "bad_pin": pin,
        "breaking_commit": brk,
        "pr": pr or None,
        "mechanism": mech,
        "status": status,
        "stable": stable == "1",
        "bisect_rc": int(rc),
        "method": "pin-fixed-ledger",
    },
    open(out, "w"),
    indent=2,
)
PY
echo "done witness=$WITNESS good=${GOOD:0:12} break=${BREAK:0:12} pr=$PR status=$STATUS"

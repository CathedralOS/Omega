#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
CLUSTER_ID="${2:?}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
chmod -R u+w "$ROOT/target" 2>/dev/null || true

if ! GOOD=$(python3 "$ROOT/.agents/ledger-bisect-2026-11-11/find_good_commit.py" "$WITNESS" 2>"$ROOT/.agents/ledger-bisect-2026-11-11/find-good-${CLUSTER_ID}.log"); then
  echo "cluster=$CLUSTER_ID witness=$WITNESS status=propagated reason=no_good_commit"
  exit 0
fi

git checkout -f -q "$PIN"
set +e
nice -n 19 "$SCRIPT"; pin_ec=$?
git checkout -f -q "$GOOD"
nice -n 19 "$SCRIPT"; good_ec=$?
git checkout -f -q "$PIN"
set -e

if [[ "$good_ec" -ne 0 ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD status=propagated reason=good_exit_${good_ec}"
  exit 0
fi
if [[ "$pin_ec" -ne 1 ]]; then
  echo "cluster=$CLUSTER_ID witness=$WITNESS status=propagated reason=pin_exit_${pin_ec}"
  exit 0
fi

git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD"
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || echo "")
PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
git bisect reset
git checkout -f -q "$PIN"
MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
show = subprocess.run(["git", "show", "--no-renames", "--format=", brk], capture_output=True, text=True).stdout
code = [l for l in show.splitlines() if l.startswith(("diff --git a/omega-rust", "diff --git a/source/"))]
subj = subprocess.check_output(["git", "log", "-1", "--format=%s", brk], text=True).strip()
first = ""
for l in code[:3]:
    parts = l.split()
    if len(parts) >= 4:
        first += parts[3].replace("b/", "") + " "
print(f"{subj}; touches {first}".strip()[:200])
PY
)
echo "cluster=$CLUSTER_ID witness=$WITNESS good=$GOOD break=$BREAK pr=$PR rc=$RC mech=$MECH status=bisected"

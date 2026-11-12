#!/bin/bash
# Nearest first-parent ancestor of REF where witness returns 0 (exponential walk, not binary search).
set -euo pipefail
WITNESS="${1:?}"
REF="${2:?}" # usually PIN
ROOT="/agent/repos/omega"
LB="/tmp/ledger-bisect-2026-11-11"
[[ -d "$LB/tools" ]] || cp -a "$ROOT/.agents/ledger-bisect-2026-11-11" "$LB"
export BISECT_WITNESS="$WITNESS"
STEP="$LB/tools/bisect_step_worktree.sh"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-5
mapfile -t commits < <(git rev-list --first-parent "$REF")
test_rc() {
  local c=$1
  git checkout -q "$c"
  set +e
  "$STEP" >/dev/null 2>&1
  local r=$?
  set -e
  echo "exp_good probe $c rc=$r" >&2
  echo "$r"
}
n=${#commits[@]}
far_good=-1
for idx in $((n - 1)) $((n / 2)) $((n / 4)) $((n / 8)) 8000 4000 2000 1000 500 200 100 50; do
  [[ $idx -lt 1 ]] && continue
  [[ $idx -ge $n ]] && continue
  rc=$(test_rc "${commits[$idx]}")
  if [[ "$rc" == "0" ]]; then
    far_good=$idx
    break
  fi
done
if [[ $far_good -lt 0 ]]; then
  git checkout -q work/ledger-bisect-part-5
  echo "no good ancestor on first-parent line for $WITNESS" >&2
  exit 1
fi
# Walk forward from far_good toward PIN to nearest good (decreasing index)
nearest=$far_good
for ((idx = far_good - 1; idx >= 1; idx--)); do
  rc=$(test_rc "${commits[$idx]}")
  if [[ "$rc" == "0" ]]; then
    nearest=$idx
  elif [[ "$rc" == "1" ]]; then
    break
  fi
done
GOOD="${commits[$nearest]}"
git checkout -q work/ledger-bisect-part-5
echo "$GOOD"

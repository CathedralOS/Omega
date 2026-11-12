#!/bin/bash
# Nearest first-parent ancestor of PIN where fixed-PIN test returns 0.
set -euo pipefail
WITNESS="${1:?witness}"
ROOT="/agent/repos/omega"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
cd "$ROOT"
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_pin.py" "$WITNESS" >/dev/null
export BISECT_WITNESS="$WITNESS"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
git checkout -q work/ledger-bisect-part-5
mapfile -t commits < <(git rev-list --first-parent "$PIN")
test_rc() {
  local c=$1
  git checkout -q "$c"
  set +e
  "$SCRIPT" >/dev/null 2>&1
  local r=$?
  set -e
  echo "find_good probe $c rc=$r $(git log -1 --format='%s' "$c" | head -c 72)" >&2
  echo "$r"
}
pin_rc=$(test_rc "$PIN")
if [[ "$pin_rc" != "1" ]]; then
  echo "PIN is not bad for $WITNESS (rc=$pin_rc)" >&2
  git checkout -q work/ledger-bisect-part-5
  exit 1
fi
n=${#commits[@]}
far_good=-1
for idx in $((n - 1)) $((n / 2)) $((n / 4)) 5000 2000 1000 500 300 200; do
  [[ $idx -lt 1 ]] && continue
  [[ $idx -ge $n ]] && continue
  rc=$(test_rc "${commits[$idx]}")
  if [[ "$rc" == "0" ]]; then
    far_good=$idx
    break
  fi
done
if [[ $far_good -lt 0 ]]; then
  for ((idx = n - 1; idx >= 1; idx -= 50)); do
    rc=$(test_rc "${commits[$idx]}")
    if [[ "$rc" == "0" ]]; then
      far_good=$idx
      break
    fi
  done
fi
if [[ $far_good -lt 0 ]]; then
  git checkout -q work/ledger-bisect-part-5
  echo "no good ancestor on first-parent line for $WITNESS" >&2
  exit 1
fi
# nearest good: binary search (0, good] since index 0 is PIN bad
lo=1
hi=$far_good
while [[ $lo -lt $hi ]]; do
  mid=$(( (lo + hi) / 2 ))
  rc=$(test_rc "${commits[$mid]}")
  if [[ "$rc" == "0" ]]; then
    hi=$mid
  elif [[ "$rc" == "1" ]]; then
    lo=$((mid + 1))
  else
    lo=$((mid + 1))
  fi
done
GOOD="${commits[$lo]}"
final=$(test_rc "$GOOD")
if [[ "$final" != "0" ]]; then
  git checkout -q work/ledger-bisect-part-5
  exit 1
fi
git checkout -q work/ledger-bisect-part-5
echo "$GOOD"

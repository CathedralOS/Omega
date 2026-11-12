#!/bin/bash
# Newest first-parent ancestor of PIN where witness meets PIN-ledger expectation (binary search).
set -euo pipefail
PIN="${PIN_COMMIT:-51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
start_ref=$(git symbolic-ref -q HEAD || git rev-parse HEAD)
cleanup() { git checkout -q "$start_ref" 2>/dev/null || true; }
trap cleanup EXIT

mapfile -t commits < <(git rev-list --first-parent "${PIN}^1" 2>/dev/null || true)
if ((${#commits[@]} == 0)); then
  echo "find_bisect_good: no first-parent history before PIN" >&2
  exit 1
fi

export BISECT_NO_LEDGER_SKIP=1
git checkout -q "$PIN"
rm -f /tmp/bisect_one_record.txt
set +e
"$SCRIPT"
pin_rc=$?
set -e
if [[ "$pin_rc" != "1" ]]; then
  echo "find_bisect_good: PIN must exit 1 (broken), got ${pin_rc:-empty}" >&2
  exit 1
fi
unset BISECT_NO_LEDGER_SKIP

test_rc() {
  local c=$1
  git checkout -q "$c"
  rm -f /tmp/bisect_one_record.txt
  set +e
  "$SCRIPT"
  local r=$?
  set -e
  return "$r"
}

lo=0
hi=$((${#commits[@]} - 1))
best=""
while ((lo <= hi)); do
  mid=$(( (lo + hi) / 2 ))
  set +e
  test_rc "${commits[mid]}"
  rc=$?
  set -e
  if [[ $rc -eq 0 ]]; then
    best="${commits[mid]}"
    hi=$((mid - 1))
  elif [[ $rc -eq 1 || $rc -eq 125 ]]; then
    lo=$((mid + 1))
  else
    echo "find_bisect_good: unexpected rc=$rc at ${commits[mid]}" >&2
    lo=$((mid + 1))
  fi
done

if [[ -z "$best" ]]; then
  echo "find_bisect_good: no good commit among ${#commits[@]} first-parent ancestors" >&2
  exit 1
fi

# Sanity: good commit should be rc=0 in triplicate; immediate child toward PIN should be rc=1
export BISECT_NO_LEDGER_SKIP=1
for i in 1 2 3; do
  set +e
  test_rc "$best"
  g=$?
  set -e
  [[ $g -eq 0 ]] || {
    echo "find_bisect_good: flaky good $best run $i rc=$g" >&2
    exit 1
  }
done
idx=-1
for i in "${!commits[@]}"; do
  [[ "${commits[i]}" == "$best" ]] && idx=$i
done
if ((idx > 0)); then
  set +e
  test_rc "${commits[$((idx - 1))]}"
  br=$?
  set -e
  echo "sanity: newer ${commits[$((idx - 1))][:12]} rc=$br (expect 1)" >&2
fi
unset BISECT_NO_LEDGER_SKIP
echo "$best"

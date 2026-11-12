#!/bin/bash
# First bad commit via binary search on first-parent (GOOD rc=0, PIN rc=1).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:?}"
GOOD="${3:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
mapfile -t chain < <(git rev-list --first-parent "$PIN" --not "$GOOD" | tac)
chain=("$GOOD" "${chain[@]}")
n=${#chain[@]}
((n >= 2)) || { echo "range too small" >&2; exit 1; }
git checkout -q "${chain[0]}"
rc=0; "$TOOL" || rc=$?; [[ "$rc" -eq 0 ]] || { echo "good end not good: $rc at ${chain[0]}" >&2; exit 1; }
git checkout -q "${chain[n-1]}"
rc=0; "$TOOL" || rc=$?; [[ "$rc" -eq 1 ]] || { echo "bad end not bad: $rc at ${chain[n-1]}" >&2; exit 1; }
lo=0
hi=$((n - 1))
while ((lo + 1 < hi)); do
  mid=$(( (lo + hi) / 2 ))
  c="${chain[$mid]}"
  git checkout -q "$c"
  rc=0
  "$TOOL" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    lo=$mid
  elif [[ "$rc" -eq 1 ]]; then
    hi=$mid
  else
    lo=$mid
  fi
done
first_bad="${chain[$hi]}"
git log -1 --oneline "$first_bad"
echo "$first_bad"

#!/bin/bash
set -euo pipefail
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
INTRO="${2:-5734633732932a8a836893d730dbcbe78074a704}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCR="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
mapfile -t COMMITS < <(git -C "$ROOT" rev-list --first-parent "$INTRO".."$PIN" | tac)
n=${#COMMITS[@]}
echo "range_commits=$n intro=$INTRO pin=$PIN"
lo=0
hi=$((n - 1))
best=""
while ((lo <= hi)); do
  mid=$(( (lo + hi) / 2 ))
  c="${COMMITS[$mid]}"
  git -C "$ROOT" checkout -q "$c"
  rc=0
  nice -n 19 "$SCR" || rc=$?
  echo "mid=$mid/$n $c rc=$rc $(git -C "$ROOT" show -s --format='%s' "$c" | head -c 60)"
  if [[ "$rc" -eq 0 ]]; then
    best="$c"
    hi=$((mid - 1))
  elif [[ "$rc" -eq 1 ]]; then
    lo=$((mid + 1))
  else
  # skip / missing fixture — treat as still searching upward
    lo=$((mid + 1))
  fi
done
if [[ -z "$best" ]]; then
  echo "NO_GOOD_IN_RANGE" >&2
  exit 1
fi
echo "GOOD=$best"
git -C "$ROOT" checkout -q "$PIN"

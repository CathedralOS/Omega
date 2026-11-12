#!/bin/bash
# Binary search first-parent ancestry for a good commit (rc=0).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
mapfile -t chain < <(git rev-list --first-parent "$PIN")
n=${#chain[@]}
lo=$((n - 1))
hi=0
# chain[0]=PIN, chain[n-1]=oldest reachable; search hi..lo for oldest good
while ((lo - hi > 1)); do
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
    # skip ambiguous (125): step older
    lo=$((mid + 1))
  fi
done
# verify good
git checkout -q "${chain[$lo]}"
rc=0
"$TOOL" || rc=$?
[[ "$rc" -eq 0 ]] || { echo "failed to find good" >&2; exit 1; }
echo "${chain[$lo]}"

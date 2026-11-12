#!/bin/bash
# Newest first-parent ancestor (closest to PIN) where witness is good (rc=0).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
mapfile -t chain < <(git rev-list --first-parent "$PIN")
n=${#chain[@]}
step=1
while ((step < n)); do
  step=$((step * 2))
done
step=$((step / 2))
[[ "$step" -lt 1 ]] && step=1
found=""
for ((i=0; i<n; i+=step)); do
  c="${chain[$i]}"
  git checkout -q "$c"
  rc=0
  "$TOOL" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    found=$i
    break
  fi
done
if [[ -z "$found" ]]; then
  echo "no good in coarse scan (n=$n)" >&2
  exit 1
fi
lo=$found
hi=$((found - step))
((hi < 0)) && hi=0
while ((lo > hi)); do
  mid=$(( (lo + hi + 1) / 2 ))
  c="${chain[$mid]}"
  git checkout -q "$c"
  rc=0
  "$TOOL" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    lo=$mid
  else
    hi=$((mid - 1))
  fi
done
echo "${chain[$lo]}"

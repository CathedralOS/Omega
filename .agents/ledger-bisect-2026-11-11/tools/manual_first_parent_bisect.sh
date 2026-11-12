#!/bin/bash
# Binary search first-parent range for first commit where witness returns 1 (bad).
set -euo pipefail
GOOD="${1:?good commit}"
BAD="${2:?bad commit}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
mapfile -t COMMITS < <(git -C "$ROOT" rev-list --first-parent --reverse "$GOOD" "$BAD")
if [[ "${COMMITS[0]}" != "$(git rev-parse "$GOOD")" ]]; then
  COMMITS=("$(git rev-parse "$GOOD")" "${COMMITS[@]}")
fi
probe() {
  local c="$1"
  git -C "$ROOT" checkout -q "$c"
  local r=0
  "$SCRIPT" || r=$?
  echo "probe $c -> $r" >&2
  echo "$r"
}

lo=0
hi=$((${#COMMITS[@]} - 1))
while (( lo + 1 < hi )); do
  mid=$(( (lo + hi) / 2 ))
  r=$(probe "${COMMITS[mid]}")
  if [[ "$r" -eq 0 ]]; then
    lo=$mid
  elif [[ "$r" -eq 1 ]]; then
    hi=$mid
  else
  # skip: inch toward bad
    if (( mid + 1 < hi )); then
      mid=$((mid + 1))
      r=$(probe "${COMMITS[mid]}")
      if [[ "$r" -eq 1 ]]; then hi=$mid; elif [[ "$r" -eq 0 ]]; then lo=$mid; fi
    fi
  fi
done
FIRST_BAD="${COMMITS[hi]}"
git -C "$ROOT" show -s --format='%H %s' "$FIRST_BAD"
git -C "$ROOT" checkout -q work/ledger-bisect-part-32 2>/dev/null || git -C "$ROOT" checkout -q "$BAD"

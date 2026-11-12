#!/bin/bash
# First-parent binary search for newest commit where witness still matches ledger (bisect_test exit 0).
set -euo pipefail
WITNESS="${1:?witness}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
ROOT="${BISECT_REPO_ROOT:-$MAIN_REPO}"
SCRIPT="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
START="$(git rev-parse HEAD)"
DEPTH="${BISECT_GOOD_DEPTH:-400}"

test_commit() {
  local commit="$1"
  git checkout -f "$commit" >/dev/null 2>&1 || return 2
  export BISECT_WITNESS="$WITNESS"
  export BISECT_REPO_ROOT="$ROOT"
  local rc=0
  BISECT_FRESH=0 nice -n 19 "$SCRIPT" >/dev/null 2>&1 || rc=$?
  [[ "$rc" -eq 0 ]]
}

mapfile -t COMMITS < <(git rev-list --first-parent -n "$DEPTH" 759c80404428)
n=${#COMMITS[@]}
[[ "$n" -ge 2 ]] || { git checkout -f "$START" >/dev/null 2>&1; exit 1; }

# COMMITS[0] is newest (759c804…); COMMITS[n-1] is oldest.
is_good() {
  local idx="$1"
  test_commit "${COMMITS[$idx]}"
}

if is_good 0; then
  git checkout -f "$START" >/dev/null 2>&1
  echo "${COMMITS[0]}"
  exit 0
fi

if ! is_good $((n - 1)); then
  git checkout -f "$START" >/dev/null 2>&1
  exit 1
fi

lo=$((n - 1))
hi=0
while ((lo > hi + 1)); do
  mid=$(( (lo + hi) / 2 ))
  if is_good "$mid"; then
    lo=$mid
  else
    hi=$mid
  fi
done

git checkout -f "$START" >/dev/null 2>&1
echo "${COMMITS[$lo]}"

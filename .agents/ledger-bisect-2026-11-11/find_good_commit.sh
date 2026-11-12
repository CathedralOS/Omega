#!/bin/bash
# Newest first-parent ancestor of PIN where witness matches fixed PIN-ledger good (exit 0).
set -euo pipefail
WITNESS="${1:?witness}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
ROOT="${BISECT_REPO_ROOT:-$MAIN_REPO}"
PREPARE="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_expect.py"
SCRIPT="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
PIN="$(git -C "$ROOT" rev-parse HEAD)"
cd "$ROOT"
START="$PIN"
DEPTH="${BISECT_GOOD_DEPTH:-800}"

export_file="$(
  BISECT_PIN_LEDGER="${BISECT_PIN_LEDGER:-/tmp/bisect_pin_corpus_outcomes.txt}" \
    python3 "$PREPARE" "$WITNESS"
)"
export BISECT_EXPECT_FILE="$export_file"
export BISECT_WITNESS="$WITNESS"
export BISECT_REPO_ROOT="$ROOT"

mapfile -t COMMITS < <(git rev-list --first-parent -n "$DEPTH" "$PIN")
n=${#COMMITS[@]}
[[ "$n" -ge 2 ]] || { git checkout -f "$START" >/dev/null 2>&1; exit 1; }

test_commit() {
  local commit="$1"
  git checkout -f "$commit" >/dev/null 2>&1 || return 125
  local rc=0
  nice -n 19 "$SCRIPT" >/dev/null 2>&1 || rc=$?
  return "$rc"
}

# COMMITS[0]=PIN; COMMITS[1] newest parent. Find smallest index i>=1 with exit 0.
lo=1
hi=$((n - 1))
found=""
while ((lo <= hi)); do
  mid=$(((lo + hi) / 2))
  rc=0
  test_commit "${COMMITS[$mid]}" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    found="${COMMITS[$mid]}"
    hi=$((mid - 1))
  else
    lo=$((mid + 1))
  fi
done

git checkout -f "$START" >/dev/null 2>&1
if [[ -n "$found" ]]; then
  echo "$found"
  exit 0
fi
exit 1

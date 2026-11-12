#!/bin/bash
# Print a first-parent commit where witness satisfies ledger (exit 0 from bisect_test).
set -euo pipefail
WITNESS="${1:?witness}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
START_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

test_commit() {
  local commit="$1"
  git checkout -q "$commit" || return 1
  export BISECT_WITNESS="$WITNESS"
  if BISECT_FRESH=0 nice -n 19 "$SCRIPT" >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

ledger_sha=""
while IFS= read -r line; do
  [[ "$line" == "$WITNESS "* ]] || continue
  for tok in $line; do
    [[ "$tok" == @* ]] && ledger_sha="${tok#@}" && ledger_sha="${ledger_sha%%+*}"
  done
  break
done <"$ROOT/tests/omega/corpus_outcomes.txt"

candidates=()
[[ -n "$ledger_sha" ]] && candidates+=("$ledger_sha")
candidates+=(
  759c80404428
  741270df60f7eab218e879eb6771d2bc75e64c00^
  ccdb99ca2214
)
declare -A seen
for c in "${candidates[@]}"; do
  [[ -z "$c" || -n "${seen[$c]:-}" ]] && continue
  seen[$c]=1
  if git cat-file -e "${c}^{commit}" 2>/dev/null && test_commit "$c"; then
  git checkout -q "$START_BRANCH"
    echo "$c"
    exit 0
  fi
done

# Walk first-parent from 759c804 backward up to 120 steps
base=759c80404428
for c in $(git rev-list --first-parent -n 40 "$base"); do
  [[ -n "${seen[$c]:-}" ]] && continue
  seen[$c]=1
  if test_commit "$c"; then
    git checkout -q "$START_BRANCH"
    echo "$c"
    exit 0
  fi
done

git checkout -q "$START_BRANCH"
exit 1

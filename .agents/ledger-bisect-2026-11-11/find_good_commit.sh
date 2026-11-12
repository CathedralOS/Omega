#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
ROOT="${BISECT_REPO_ROOT:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
START="$(git rev-parse HEAD)"

test_commit() {
  local commit="$1"
  git checkout -f "$commit" >/dev/null 2>&1 || return 1
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
candidates+=(759c80404428 741270df60f7eab218e879eb6771d2bc75e64c00^)

declare -A seen
for c in "${candidates[@]}"; do
  [[ -z "$c" || -n "${seen[$c]:-}" ]] && continue
  seen[$c]=1
  if git cat-file -e "${c}^{commit}" 2>/dev/null && test_commit "$c"; then
    git checkout -f "$START" >/dev/null 2>&1 || true
    echo "$c"
    exit 0
  fi
done

for c in $(git rev-list --first-parent -n 80 759c80404428); do
  [[ -n "${seen[$c]:-}" ]] && continue
  seen[$c]=1
  if test_commit "$c"; then
    git checkout -f "$START" >/dev/null 2>&1 || true
    echo "$c"
    exit 0
  fi
done

git checkout -f "$START" >/dev/null 2>&1 || true
exit 1

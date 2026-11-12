#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
"$DIR/prepare_bisect_pin.sh" "$WITNESS" "$PIN" "$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_observed.txt"
BRANCH="$(git symbolic-ref -q --short HEAD || true)"
is_ledger_only() {
  local commit="$1"
  git rev-parse -q --verify "${commit}^1" >/dev/null 2>&1 || return 1
  mapfile -t CHANGED < <(git diff --name-only "${commit}^1" "$commit")
  ((${#CHANGED[@]} == 0)) && return 1
  for f in "${CHANGED[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt) ;;
      *.md) ;;
      *) return 1 ;;
    esac
  done
  return 0
}
while read -r commit; do
  git checkout --quiet "$commit" || continue
  if is_ledger_only "$commit"; then
    echo "skip ledger-only $commit $(git log -1 --oneline "$commit")"
    continue
  fi
  code=$("$DIR/bisect_witness.sh" 2>/dev/null || echo 125)
  echo "$commit $code $(git log -1 --oneline "$commit")"
  if [[ "$code" == "0" ]]; then
    [[ -n "$BRANCH" ]] && git checkout --quiet "$BRANCH" || git checkout --quiet "$PIN"
    echo "GOOD=$commit"
    exit 0
  fi
done < <(git rev-list --first-parent "$PIN")
[[ -n "$BRANCH" ]] && git checkout --quiet "$BRANCH" || git checkout --quiet "$PIN"
echo "GOOD=NOT_FOUND"
exit 1

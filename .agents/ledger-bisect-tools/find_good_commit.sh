#!/bin/bash
# Walk first-parent from PIN until witness test returns 0 (fixed PIN ledger).
set -euo pipefail
WITNESS="${1:?}"
MAX="${2:-400}"
TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$TOOL_DIR" rev-parse --show-toplevel)"
cd "$ROOT"
PIN="$(git rev-parse HEAD)"
"$TOOL_DIR/prepare_bisect_witness.sh" "$WITNESS" "$PIN"
commit="$PIN"
steps=0
while [[ $steps -lt $MAX ]]; do
  parent=$(git rev-parse "$commit^1" 2>/dev/null) || break
  git checkout -q "$parent"
  # ledger-only skip without building
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r HEAD)
  only_ledger=true
  for f in "${files[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt|*.md) ;;
      *) only_ledger=false; break ;;
    esac
  done
  if $only_ledger; then
    commit="$parent"
    steps=$((steps + 1))
    continue
  fi
  rc=0
  BISECT_WITNESS="$WITNESS" bash "$TOOL_DIR/bisect_witness.sh" || rc=$?
  if [[ $rc -eq 0 ]]; then
    echo "$parent"
    git checkout -q "$PIN"
    exit 0
  fi
  if [[ $rc -eq 125 ]]; then
    commit="$parent"
    steps=$((steps + 1))
    continue
  fi
  commit="$parent"
  steps=$((steps + 1))
done
git checkout -q "$PIN"
echo "FAILED" >&2
exit 1

#!/bin/bash
# Walk first-parent ancestors of PIN until witness test returns 0.
set -euo pipefail
PIN="${BISECT_PIN:?}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:?}"
export BISECT_REPO_ROOT="$ROOT"
export BISECT_WITNESS_SPEC
SCRIPT="${BISECT_WITNESS_SCRIPT:?}"
cd "$ROOT"
start_branch="$(git branch --show-current 2>/dev/null || true)"
trap 'git checkout -q "${start_branch:-$PIN}" 2>/dev/null || git checkout -q "$PIN"' EXIT

n=0
for commit in $(git rev-list --first-parent "$PIN" | tail -n +2); do
  n=$((n + 1))
  echo "find_good try $n $(git log -1 --oneline "$commit")" >&2
  git checkout -q "$commit"
  set +e
  "$SCRIPT"
  ec=$?
  set -e
  if [[ $ec -eq 0 ]]; then
    echo "find_good found at try $n" >&2
    echo "$commit"
    exit 0
  fi
  if [[ $ec -ne 1 && $ec -ne 125 ]]; then
  :
  fi
done
echo "find_good_commit: no good commit found for $WITNESS" >&2
exit 1

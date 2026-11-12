#!/bin/bash
# Walk first-parent ancestors from PIN until witness matches expected PIN outcome.
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$(dirname "$0")/bisect_witness.sh"
cd "$ROOT"
commit="$PIN"
while [[ -n "$commit" ]]; do
  git checkout -q "$commit"
  result=0
  "$SCRIPT" || result=$?
  echo "probe $commit -> $result" >&2
  if [[ "$result" -eq 0 ]]; then
    echo "$commit"
    git checkout -q work/ledger-bisect-part-32 2>/dev/null || git checkout -q "$PIN"
    exit 0
  fi
  if [[ "$result" -eq 125 ]]; then
    commit=$(git rev-parse "${commit}^1" 2>/dev/null) || break
    continue
  fi
  # result 1 = bad, walk back
  commit=$(git rev-parse "${commit}^1" 2>/dev/null) || break
done
echo "no good commit found" >&2
exit 1

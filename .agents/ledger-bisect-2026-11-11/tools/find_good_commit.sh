#!/bin/bash
# Walk first-parent ancestors from PIN until witness test returns 0.
set -euo pipefail
PIN="${1:?pin commit}"
WITNESS="${BISECT_WITNESS:?}"
TOOLS="$(dirname "$0")"
export BISECT_WITNESS_BASELINE BISECT_PIN_LEDGER
ROOT="$(git -C "$TOOLS/../../.." rev-parse --show-toplevel)"
cd "$ROOT"
cur="$PIN"
steps=0
max="${BISECT_GOOD_WALK_MAX:-80}"
while [[ -n "$cur" && $steps -lt $max ]]; do
  git checkout --quiet "$cur"
  code=0
  bash "$TOOLS/bisect_witness.sh" || code=$?
  if [[ "$code" == "0" ]]; then
    echo "$cur"
    git checkout --quiet "$PIN"
    exit 0
  fi
  cur="$(git rev-parse --first-parent "$cur^" 2>/dev/null || true)"
  steps=$((steps + 1))
done
git checkout --quiet "$PIN"
echo "FAILED" >&2
exit 1

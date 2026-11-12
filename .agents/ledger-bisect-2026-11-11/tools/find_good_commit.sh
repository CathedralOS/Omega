#!/bin/bash
# Walk first-parent from PIN until witness matches PIN-expected outcome (exit 0).
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
: "${BISECT_FIXTURE_DIR:?export BISECT_FIXTURE_DIR before find_good_commit}"
export BISECT_FIXTURE_DIR
current="$PIN"
steps=0
max="${BISECT_GOOD_MAX:-400}"
while [[ $steps -lt $max ]]; do
  git checkout -q "$current"
  short=$(git show -s --format='%h %s' "$current")
  echo "step $steps commit $current $short" >&2
  set +e
  "$SCRIPT"
  rc=$?
  set -e
  echo "step $steps exit=$rc" >&2
  if [[ $rc -eq 0 ]]; then
    echo "$current"
    exit 0
  fi
  parent=$(git rev-parse "$current^1" 2>/dev/null) || break
  current="$parent"
  steps=$((steps + 1))
done
echo "find_good: no good within $max first-parent steps from $PIN" >&2
exit 1

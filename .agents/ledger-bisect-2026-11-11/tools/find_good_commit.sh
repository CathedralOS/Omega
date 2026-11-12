#!/bin/bash
# Walk first-parent ancestors of PIN until witness test returns 0.
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLS="$(dirname "$0")"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"

if [[ ! -f /tmp/pin_witness_observation.json ]]; then
  echo "missing pin snapshot; run pin_snapshot.py on PIN first" >&2
  exit 2
fi

commit="$PIN"
steps=0
max=2500
while [[ -n "$commit" && $steps -lt $max ]]; do
  git checkout -f "$commit" >/dev/null 2>&1 || { echo "checkout fail $commit" >&2; break; }
  set +e
  bash "$TOOLS/bisect_witness.sh"
  ec=$?
  set -e
  if [[ $ec -eq 0 ]]; then
    echo "$commit"
    git checkout -f work/ledger-bisect-part-10 >/dev/null 2>&1 || git checkout -f "$PIN" >/dev/null
    exit 0
  fi
  if [[ $ec -ne 1 && $ec -ne 125 ]]; then
    echo "unexpected exit $ec at $commit" >&2
  fi
  parent=$(git rev-parse "${commit}^1" 2>/dev/null || true)
  if [[ -z "$parent" || "$parent" == "$commit" ]]; then
    break
  fi
  # stay on first-parent chain
  if ! git merge-base --is-ancestor "$parent" "$PIN" 2>/dev/null; then
    break
  fi
  commit="$parent"
  steps=$((steps + 1))
done
git checkout -f work/ledger-bisect-part-10 >/dev/null 2>&1 || git checkout -f "$PIN" >/dev/null
echo "no good commit found" >&2
exit 1

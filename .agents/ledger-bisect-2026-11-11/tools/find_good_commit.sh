#!/bin/bash
# Walk first-parent back from PIN until bisect_witness returns 0.
set -euo pipefail
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCR="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
c="$PIN"
steps=0
while true; do
  git checkout -q "$c"
  rc=0
  nice -n 19 "$SCR" || rc=$?
  echo "commit=$c steps=$steps exit=$rc $(git show -s --format='%s' "$c" | head -c 72)"
  if [[ "$rc" -eq 0 ]]; then
    echo "GOOD=$c"
    exit 0
  fi
  if [[ "$rc" -ne 1 ]]; then
    echo "skip_or_fail rc=$rc at $c" >&2
  fi
  parent=$(git rev-parse "$c^1" 2>/dev/null || true)
  if [[ -z "$parent" ]]; then
    echo "no good commit found" >&2
    exit 1
  fi
  c="$parent"
  steps=$((steps + 1))
  if ((steps > 200)); then
    echo "gave up after 200 steps" >&2
    exit 1
  fi
done

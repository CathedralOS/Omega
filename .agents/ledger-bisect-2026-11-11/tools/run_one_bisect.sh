#!/bin/bash
# Usage: run_one_bisect.sh <log_name> <witness> <good_sha>
set -euo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
NAME="${1:?}"
WITNESS="${2:?}"
GOOD="${3:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/bisect-${NAME}.log"
git checkout work/ledger-bisect-part-27 >/dev/null 2>&1 || git checkout -B work/ledger-bisect-part-27 "$PIN"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" 2>&1 | tee "$LOG"
FIRST_BAD=$(rg -o '^[0-9a-f]{40} is the first bad commit' "$LOG" | awk '{print $1}' || true)
git bisect reset >/dev/null
if [[ -z "$FIRST_BAD" ]]; then
  echo "Bisect failed to find first bad for $NAME" >&2
  exit 1
fi
git show -s --format='%H%n%s' "$FIRST_BAD" >"$ROOT/.agents/ledger-bisect-2026-11-11/bisect-${NAME}.result"
echo "first_bad=$FIRST_BAD"

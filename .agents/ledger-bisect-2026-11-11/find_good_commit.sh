#!/bin/bash
# Most recent first-parent ancestor of PIN where witness returns 0 (fixed PIN criteria).
set -euo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
LOG="${1:-/tmp/find_good.log}"
: >"$LOG"

test_commit() {
  local ref=$1
  git checkout -q "$ref"
  cp "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_test.py" /tmp/bisect_test.py
  cp "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_witness.sh" /tmp/bisect_witness.sh
  chmod +x /tmp/bisect_witness.sh
  /tmp/bisect_witness.sh
}

# Walk first-parent from PIN^1 until first 0.
cursor="$(git rev-parse "$PIN^1")"
steps=0
while [[ -n "$cursor" ]]; do
  steps=$((steps + 1))
  rc=$(test_commit "$cursor") || rc=$?
  echo "step=$steps ${cursor:0:12} rc=$rc" | tee -a "$LOG"
  if [[ "$rc" -eq 0 ]]; then
    echo "GOOD=$cursor" | tee -a "$LOG"
    exit 0
  fi
  if [[ "$rc" -ne 1 && "$rc" -ne 125 ]]; then
    echo "unexpected rc=$rc" | tee -a "$LOG"
    exit 1
  fi
  cursor="$(git rev-parse "$cursor^1" 2>/dev/null || true)"
done
echo "GOOD not found" | tee -a "$LOG"
exit 1

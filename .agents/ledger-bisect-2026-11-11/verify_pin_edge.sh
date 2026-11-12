#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
RUNW=/tmp/ledger-bisect-tools-pinned/run_witness_repeated.sh
W=pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/pinclass_edge_verify.log"
: >"$LOG"
for c in 96ecfb7c12877f0a60fe20c62334f4a1fd973d4a 1bb9690b80851e6b149c9a414b3317439183a882; do
  echo "=== $c ===" | tee -a "$LOG"
  "$RUNW" "$c" "$W" 3 2>&1 | tee -a "$LOG"
done

#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
git checkout -qf work/ledger-bisect-part-1
RUNM=/tmp/ledger-bisect-tools-pinned/run_members_at.sh
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/cluster_members_matrix.log"
PIN_BAD=1bb9690b80851e6b149c9a414b3317439183a882
PIN_PARENT=$(git rev-parse "${PIN_BAD}^")
declare -a CLUSTERS=(
  "2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9|2be2bd0442046813713c68acbdf6da8b35b8b85e|pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit pass/slices/runtime_indexed_store_operand_effect_order_exit pass/slices/runtime_namespace_range_contract_exit pass/slices/runtime_returned_range_extent_contract_exit"
  "${PIN_BAD}|${PIN_PARENT}|pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap pass/facts/foreign_retained_shared_borrow_persistent_restore"
  "643ff5441c15ea62815109514a8047194d7397a0|8e7394363d14aca7dae993adeb86fc348c0d2c7e|pass/borrows/runtime_forwarded_reference_array_bounded_read_exit"
  "ade0aabb979920eea0ab13135fa2a86ca116acaa|028f2b1a70750c30a23802aece64c370b9bd7ace|pass/facts/foreign_retained_shared_borrow_persistent_retention"
)
for cluster in "${CLUSTERS[@]}"; do
  IFS='|' read -r BAD PARENT MEM <<<"$cluster"
  if grep -q "members @ $PARENT" "$LOG" && grep -q "members @ $BAD" "$LOG"; then
    awk "/members @ $BAD/,/members @ $PARENT/" "$LOG" | grep -q "pass/" && continue || true
  fi
  if ! grep -q "members @ $BAD" "$LOG"; then
    echo "=== members @ $BAD ===" | tee -a "$LOG"
    # shellcheck disable=SC2086
    "$RUNM" "$BAD" $MEM 2>&1 | tee -a "$LOG"
  fi
  if ! grep -A20 "members @ $PARENT" "$LOG" | grep -q "pass/\|fail/"; then
    echo "=== members @ $PARENT ===" | tee -a "$LOG"
    # shellcheck disable=SC2086
    "$RUNM" "$PARENT" $MEM 2>&1 | tee -a "$LOG"
  fi
done
git checkout -qf work/ledger-bisect-part-1
echo DONE_MEMBERS

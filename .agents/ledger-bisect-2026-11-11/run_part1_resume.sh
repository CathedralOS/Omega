#!/bin/bash
# Resume: cluster witness stability + member matrix (serial).
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
git checkout -qf work/ledger-bisect-part-1
LOGDIR="$ROOT/.agents/ledger-bisect-2026-11-11"
RUNW=/tmp/ledger-bisect-tools-pinned/run_witness_repeated.sh
RUNM=/tmp/ledger-bisect-tools-pinned/run_members_at.sh

# PIN-class first-bad for cluster 1 (swap_pending diag, rc=1 vs PIN ledger); see pinclass_edge_verify.log
PIN_BAD="${PIN_BAD:-1bb9690b80851e6b149c9a414b3317439183a882}"
PIN_PARENT="$(git rev-parse "${PIN_BAD}^")"

stability_log="$LOGDIR/cluster_witness_stability_v2.log"
if [[ ! -s "$stability_log" ]]; then
  : >"$stability_log"
fi
declare -a PAIRS=(
  "2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9|2be2bd0442046813713c68acbdf6da8b35b8b85e|pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit"
  "${PIN_BAD}|${PIN_PARENT}|pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap"
  "643ff5441c15ea62815109514a8047194d7397a0|8e7394363d14aca7dae993adeb86fc348c0d2c7e|pass/borrows/runtime_forwarded_reference_array_bounded_read_exit"
  "ade0aabb979920eea0ab13135fa2a86ca116acaa|028f2b1a70750c30a23802aece64c370b9bd7ace|pass/facts/foreign_retained_shared_borrow_persistent_retention"
)

for pair in "${PAIRS[@]}"; do
  IFS='|' read -r BAD PARENT W <<<"$pair"
  if grep -q "cluster witness bad=$BAD" "$stability_log" 2>/dev/null; then
    continue
  fi
  echo "=== cluster witness bad=$BAD ===" | tee -a "$stability_log"
  "$RUNW" "$BAD" "$W" 3 2>&1 | tee -a "$stability_log"
  echo "=== cluster witness parent=$PARENT ===" | tee -a "$stability_log"
  "$RUNW" "$PARENT" "$W" 3 2>&1 | tee -a "$stability_log"
done

members_log="$LOGDIR/cluster_members_matrix.log"
if [[ ! -f "$members_log" ]]; then
  : >"$members_log"
fi
declare -a CLUSTERS=(
  "2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9|2be2bd0442046813713c68acbdf6da8b35b8b85e|pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit pass/slices/runtime_indexed_store_operand_effect_order_exit pass/slices/runtime_namespace_range_contract_exit pass/slices/runtime_returned_range_extent_contract_exit"
  "${PIN_BAD}|${PIN_PARENT}|pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap pass/facts/foreign_retained_shared_borrow_persistent_restore"
  "643ff5441c15ea62815109514a8047194d7397a0|8e7394363d14aca7dae993adeb86fc348c0d2c7e|pass/borrows/runtime_forwarded_reference_array_bounded_read_exit"
  "ade0aabb979920eea0ab13135fa2a86ca116acaa|028f2b1a70750c30a23802aece64c370b9bd7ace|pass/facts/foreign_retained_shared_borrow_persistent_retention"
)
for cluster in "${CLUSTERS[@]}"; do
  IFS='|' read -r BAD PARENT MEM <<<"$cluster"
  if grep -q "members @ $BAD" "$members_log" 2>/dev/null; then
    continue
  fi
  echo "=== members @ $BAD ===" | tee -a "$members_log"
  # shellcheck disable=SC2086
  "$RUNM" "$BAD" $MEM 2>&1 | tee -a "$members_log"
  echo "=== members @ $PARENT ===" | tee -a "$members_log"
  # shellcheck disable=SC2086
  "$RUNM" "$PARENT" $MEM 2>&1 | tee -a "$members_log"
done

git checkout -qf work/ledger-bisect-part-1
echo "DONE resume" | tee "$LOGDIR/acceptance_runner.done"

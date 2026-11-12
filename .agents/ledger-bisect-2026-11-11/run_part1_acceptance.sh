#!/bin/bash
# Serial PIN-fixed measurements for part-1 acceptance (one build at a time).
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
git checkout -qf work/ledger-bisect-part-1
LOGDIR="$ROOT/.agents/ledger-bisect-2026-11-11"
RUNW=/tmp/ledger-bisect-tools-pinned/run_witness_repeated.sh
RUNM=/tmp/ledger-bisect-tools-pinned/run_members_at.sh
W_SLOT=pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap

conflict_log="$LOGDIR/conflict_borrowed_slot_swap_3x4.log"
: >"$conflict_log"
for c in \
  f2c75870ebaf890328b787fa6d8ed50e868edbb8 \
  4faf6413467dd7a7f6cf7e62ba77d1c607591f18 \
  2ab64136c02d7879635a92e3e078d7374bec7590 \
  0aeed1114ae3d056f78113e377f10b7126b1e75e
do
  echo "=== $c ===" | tee -a "$conflict_log"
  "$RUNW" "$c" "$W_SLOT" 3 2>&1 | tee -a "$conflict_log"
done

stability_log="$LOGDIR/cluster_witness_stability_v2.log"
: >"$stability_log"
declare -a PAIRS=(
  "2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9|2be2bd0442046813713c68acbdf6da8b35b8b85e|pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit"
  "643ff5441c15ea62815109514a8047194d7397a0|8e7394363d14aca7dae993adeb86fc348c0d2c7e|pass/borrows/runtime_forwarded_reference_array_bounded_read_exit"
  "ade0aabb979920eea0ab13135fa2a86ca116acaa|028f2b1a70750c30a23802aece64c370b9bd7ace|pass/facts/foreign_retained_shared_borrow_persistent_retention"
)
# Cluster 1 PIN-class edge: binary search first rc=1 with swap_pending diag between good and 48b23bf
GOOD=3bbd06849b998f4fef4ccdffb8ef1f6059c07def
BAD_HIGH=48b23bf0bb3103a4991e0bee43e2f446dea54d28
pinclass_log="$LOGDIR/cluster1_pinclass_first_bad.log"
: >"$pinclass_log"
LOW="$GOOD"
HIGH="$BAD_HIGH"
FIRST_PIN=""
while true; do
  COUNT=$(git rev-list --first-parent --count "$HIGH" --not "$LOW" || echo 0)
  if [[ "$COUNT" -le 1 ]]; then
    FIRST_PIN="$HIGH"
    break
  fi
  MID=$(git rev-list --first-parent "$HIGH" --not "$LOW" | awk 'NR==int(('"$COUNT"'/2)+0.5){print; exit}')
  [[ -z "$MID" ]] && { FIRST_PIN="$HIGH"; break; }
  echo "pinclass probe $MID (between $LOW..$HIGH)" | tee -a "$pinclass_log"
  out=$("$RUNW" "$MID" "$W_SLOT" 1 2>&1 | tee -a "$pinclass_log")
  rc=$(echo "$out" | grep '^run1 rc=' | sed 's/run1 rc=//;s/ .*//')
  diag=$(echo "$out" | grep '^run1 rc=' | sed 's/.*diag=//')
  if [[ "$rc" == "0" ]]; then
    LOW="$MID"
  elif [[ "$rc" == "1" ]] && echo "$diag" | grep -q 'swap_pending'; then
  # shellcheck disable=SC2086
    HIGH="$MID"
    FIRST_PIN="$MID"
  else
    # rc=125 other class, or unexpected: treat as not PIN-good
    LOW="$MID"
  fi
done
echo "FIRST_PIN_CLASS=$FIRST_PIN parent=$(git rev-parse "${FIRST_PIN}^")" | tee -a "$pinclass_log"
PARENT_PIN=$(git rev-parse "${FIRST_PIN}^")
PAIRS+=("$FIRST_PIN|$PARENT_PIN|$W_SLOT")

for pair in "${PAIRS[@]}"; do
  IFS='|' read -r BAD PARENT W <<<"$pair"
  echo "=== cluster witness bad=$BAD ===" | tee -a "$stability_log"
  "$RUNW" "$BAD" "$W" 3 2>&1 | tee -a "$stability_log"
  echo "=== cluster witness parent=$PARENT ===" | tee -a "$stability_log"
  "$RUNW" "$PARENT" "$W" 3 2>&1 | tee -a "$stability_log"
done

members_log="$LOGDIR/cluster_members_matrix.log"
: >"$members_log"
declare -a CLUSTERS=(
  "2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9|2be2bd0442046813713c68acbdf6da8b35b8b85e|pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit pass/slices/runtime_indexed_store_operand_effect_order_exit pass/slices/runtime_namespace_range_contract_exit pass/slices/runtime_returned_range_extent_contract_exit"
  "${FIRST_PIN}|${PARENT_PIN}|pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap pass/facts/foreign_retained_shared_borrow_persistent_restore"
  "643ff5441c15ea62815109514a8047194d7397a0|8e7394363d14aca7dae993adeb86fc348c0d2c7e|pass/borrows/runtime_forwarded_reference_array_bounded_read_exit"
  "ade0aabb979920eea0ab13135fa2a86ca116acaa|028f2b1a70750c30a23802aece64c370b9bd7ace|pass/facts/foreign_retained_shared_borrow_persistent_retention"
)
for cluster in "${CLUSTERS[@]}"; do
  IFS='|' read -r BAD PARENT MEM <<<"$cluster"
  echo "=== members @ $BAD ===" | tee -a "$members_log"
  # shellcheck disable=SC2086
  "$RUNM" "$BAD" $MEM 2>&1 | tee -a "$members_log"
  echo "=== members @ $PARENT ===" | tee -a "$members_log"
  # shellcheck disable=SC2086
  "$RUNM" "$PARENT" $MEM 2>&1 | tee -a "$members_log"
done

git checkout -qf work/ledger-bisect-part-1
echo "DONE acceptance runner" | tee "$LOGDIR/acceptance_runner.done"

#!/bin/bash
set -euo pipefail
ROOT="/agent/repos/omega"
VDIR="$ROOT/.agents/ledger-bisect-2026-11-11"
OUT="$ROOT/.agents/ledger-bisect-2026-11-11"
LOG=/tmp/cluster-a-verify.log
INC=(
  pass/facts/selected_native_code_layout_through_const_name_exit
  pass/facts/selected_native_code_layout_through_field_exit
  pass/facts/selected_native_code_layout_through_local_exit
  pass/facts/selected_native_code_layout_through_other_state_exit
  pass/facts/selected_native_code_layout_through_target_body_exit
)
REM=(
  pass/calls/native_code_layout_discarded_exit
  pass/facts/selected_native_code_layout_through_match_join_exit
  pass/facts/source_indexed_family_alpha_positive
  pass/facts/source_indexed_family_recursive_index_positive
  pass/proofs/erased_mathematical_initializers
)
ALL=("${INC[@]}" "${REM[@]}")
D26=d26fa952d579d52642970560c719d24fa2a89f8a
D26P=82d2247b96e872df79f26ec9b7af2fdec0153ce7
PINCLASS=33cefbc6c241048ff6f6c583acc5aca9b0176ec5
PINCLASSP=cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2
{
  echo "=== d26 incomplete 5 ==="
  bash "$VDIR/tools/run_verify_commit.sh" "$D26P" "${INC[3]}" "${INC[@]}"
  bash "$VDIR/tools/run_verify_commit.sh" "$D26" "${INC[3]}" "${INC[@]}"
  echo "=== pin class all 10 witness other_state ==="
  bash "$VDIR/tools/run_verify_commit.sh" "$PINCLASSP" pass/facts/selected_native_code_layout_through_other_state_exit "${ALL[@]}"
  bash "$VDIR/tools/run_verify_commit.sh" "$PINCLASS" pass/facts/selected_native_code_layout_through_other_state_exit "${ALL[@]}"
  echo "=== pin class remaining witness each ==="
  for w in "${REM[@]}"; do
    bash "$VDIR/tools/run_verify_commit.sh" "$PINCLASSP" "$w" "$w"
    bash "$VDIR/tools/run_verify_commit.sh" "$PINCLASS" "$w" "$w"
  done
} | tee "$LOG"
cp "$LOG" "$OUT/verify_cluster_a_full.txt"
echo DONE >"$OUT/verify_cluster_a.done"

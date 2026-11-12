#!/bin/bash
set -euo pipefail
VDIR="/agent/repos/omega/.agents/ledger-bisect-2026-11-11"
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
{
  bash "$VDIR/tools/run_verify_commit.sh" 82d2247b96e872df79f26ec9b7af2fdec0153ce7 pass/facts/selected_native_code_layout_through_other_state_exit "${INC[@]}"
  bash "$VDIR/tools/run_verify_commit.sh" d26fa952d579d52642970560c719d24fa2a89f8a pass/facts/selected_native_code_layout_through_other_state_exit "${INC[@]}"
  bash "$VDIR/tools/run_verify_commit.sh" cdb14cc0c3e5b5bf3ea887f1c9d35737cab677d2 pass/facts/selected_native_code_layout_through_other_state_exit "${REM[@]}"
  bash "$VDIR/tools/run_verify_commit.sh" 33cefbc6c241048ff6f6c583acc5aca9b0176ec5 pass/facts/selected_native_code_layout_through_other_state_exit "${REM[@]}"
} | tee /agent/repos/omega/.agents/ledger-bisect-2026-11-11/verify_cluster_a_key.txt

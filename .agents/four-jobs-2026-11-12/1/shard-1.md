# Route coverage shard 1

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
Harness built at: `bc3e8ee40209155f154fa21d6e47901590be43b7`
Rows completed: 394 (shard size 2174)

## Commands

Build:
```
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison < /dev/null
```

Per fixture (`fail/` uses `--every-machine`):
```
timeout 300 target/debug/examples/route_one_comparison \
  [--every-machine] <fixture>/main.omg < /dev/null
```

## Totals (partial until shard completes)

- route_verdict: {'outside': 254, 'rejected': 12, 'accepted': 75, 'error': 53}
- agreement: {'n-a': 307, 'agree': 14, 'disagree': 73}
- timeouts: 0
- disagreements: 73
- route accepts, current rejects (soundness watch): 73

## Outside by first refusal (top 20)

- `Unsupported`: 253 (e.g. tests/omega/fail/arithmetic/affine_difference_negative_edges/main.omg, tests/omega/fail/arithmetic/checked_flag_proof_shift_result_overflow/main.omg, tests/omega/fail/arithmetic/collection_result_rebound_descriptor/main.omg)

## Route accepts / current rejects

- `tests/omega/fail/arithmetic/immutable_unsigned_mask_false_floor_rejected/main.omg`
- `tests/omega/fail/arithmetic/implicit_integer_policy_erasure_rejected/main.omg`
- `tests/omega/fail/arithmetic/indexed_divisor_wrong_collection/main.omg`
- `tests/omega/fail/arithmetic/truncating_reconstruction_too_tight/main.omg`
- `tests/omega/fail/asm/atomic_release_disposition_missing_store/main.omg`
- `tests/omega/fail/asm/backward_write_stale_ensures/main.omg`
- `tests/omega/fail/asm/instruction_effect_later_operand_mutation/main.omg`
- `tests/omega/fail/asm/instruction_effect_wrong_port/main.omg`
- `tests/omega/fail/asm/wrong_label_entry_fact/main.omg`
- `tests/omega/fail/blockexec/worker_body_requires_suspend_safe_arguments/main.omg`
- `tests/omega/fail/borrow/multi_source_view_source_write/main.omg`
- `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_reuse/main.omg`
- `tests/omega/fail/borrow/write_only_reference_aggregate_conditional_replacement_loan/main.omg`
- `tests/omega/fail/borrows/borrow_duplicate_mut/main.omg`
- `tests/omega/fail/borrows/computed_case_copy_requires_live_bounds/main.omg`
- `tests/omega/fail/borrows/conditional_reference_replacement_true_first_stale_index/main.omg`
- `tests/omega/fail/borrows/copied_borrowed_slot_retains_parent_loan/main.omg`
- `tests/omega/fail/borrows/dropped_reference_refresh_does_not_restore_payload/main.omg`
- `tests/omega/fail/borrows/dynamic_byte_result_retains_first_candidate/main.omg`
- `tests/omega/fail/borrows/dynamic_window_result_cannot_escape_local/main.omg`
- `tests/omega/fail/borrows/local_loaned_field_write_beside_live_loan/main.omg`
- `tests/omega/fail/borrows/runtime_slice_result_requires_bounds/main.omg`
- `tests/omega/fail/borrows/state_loan_shared_ancestor_exclusive_descendant/main.omg`
- `tests/omega/fail/borrows/tail_recursive_write_conflicts_with_live_sibling_loan/main.omg`
- `tests/omega/fail/calls/boundary_computed_selector_out_of_bounds/main.omg`
- `tests/omega/fail/calls/constructed_argument_wrong_nominal_type/main.omg`
- `tests/omega/fail/calls/generic_operator_guard_argument_mismatch/main.omg`
- `tests/omega/fail/calls/outcome_specific_computed_mutated/main.omg`
- `tests/omega/fail/calls/terminal_call_result_type_mismatch_rejected/main.omg`
- `tests/omega/fail/collections/projected_range_call_selected_index/main.omg`
- `tests/omega/fail/concurrency/suspend_with_indirect_resource_backing/main.omg`
- `tests/omega/fail/contracts/cyclic_terminal_missing_premise/main.omg`
- `tests/omega/fail/contracts/float_field_crash_wrong_field/main.omg`
- `tests/omega/fail/contracts/match_payload_contract/main.omg`
- `tests/omega/fail/contracts/mutable_owned_collection_ensures_keeps_poststate/main.omg`
- `tests/omega/fail/contracts/nested_case_guarantee_unselected_sibling/main.omg`
- `tests/omega/fail/contracts/referent_replacement_false_guarantee/main.omg`
- `tests/omega/fail/contracts/runtime_requirement_after_argument_write/main.omg`
- `tests/omega/fail/contracts/unused_argument_bound_mutation/main.omg`
- `tests/omega/fail/core/address_interval_partition_missing_ceiling/main.omg`
- `tests/omega/fail/core/content_membership_return_erased_provider/main.omg`
- `tests/omega/fail/core/content_membership_return_wrong_instance/main.omg`
- `tests/omega/fail/core/qualified_integer_cast_call_all_returns_require_fit/main.omg`
- `tests/omega/fail/core/scoped_integer_call_crash_route_blocks_unfolding/main.omg`
- `tests/omega/fail/core/terminal_call_wrong_selected_crash_guard/main.omg`
- `tests/omega/fail/dependent/data_requires_future_formation/main.omg`
- `tests/omega/fail/dependent/declaration_predicate_requires_unproved/main.omg`
- `tests/omega/fail/domains/borrowed_record_domain_requires_qualified_input/main.omg`
- `tests/omega/fail/domains/captured_predicate_write_retires_membership/main.omg`
- `tests/omega/fail/domains/completed_membership_before_later_guarantee/main.omg`

## All disagreements

- `tests/omega/fail/arithmetic/immutable_unsigned_mask_false_floor_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/arithmetic/implicit_integer_policy_erasure_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/arithmetic/indexed_divisor_wrong_collection/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/arithmetic/truncating_reconstruction_too_tight/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/asm/atomic_release_disposition_missing_store/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/asm/backward_write_stale_ensures/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/asm/instruction_effect_later_operand_mutation/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/asm/instruction_effect_wrong_port/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/asm/wrong_label_entry_fact/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/blockexec/worker_body_requires_suspend_safe_arguments/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrow/multi_source_view_source_write/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_reuse/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrow/write_only_reference_aggregate_conditional_replacement_loan/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/borrow_duplicate_mut/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/computed_case_copy_requires_live_bounds/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/conditional_reference_replacement_true_first_stale_index/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/copied_borrowed_slot_retains_parent_loan/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/dropped_reference_refresh_does_not_restore_payload/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/dynamic_byte_result_retains_first_candidate/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/dynamic_window_result_cannot_escape_local/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/local_loaned_field_write_beside_live_loan/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/runtime_slice_result_requires_bounds/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/state_loan_shared_ancestor_exclusive_descendant/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/borrows/tail_recursive_write_conflicts_with_live_sibling_loan/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/calls/boundary_computed_selector_out_of_bounds/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/calls/constructed_argument_wrong_nominal_type/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/calls/generic_operator_guard_argument_mismatch/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/calls/outcome_specific_computed_mutated/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/calls/terminal_call_result_type_mismatch_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/collections/projected_range_call_selected_index/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/concurrency/suspend_with_indirect_resource_backing/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/cyclic_terminal_missing_premise/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/float_field_crash_wrong_field/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/match_payload_contract/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/mutable_owned_collection_ensures_keeps_poststate/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/nested_case_guarantee_unselected_sibling/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/referent_replacement_false_guarantee/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/runtime_requirement_after_argument_write/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/contracts/unused_argument_bound_mutation/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/address_interval_partition_missing_ceiling/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/content_membership_return_erased_provider/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/content_membership_return_wrong_instance/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/qualified_integer_cast_call_all_returns_require_fit/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/scoped_integer_call_crash_route_blocks_unfolding/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/core/terminal_call_wrong_selected_crash_guard/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/dependent/data_requires_future_formation/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/dependent/declaration_predicate_requires_unproved/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/borrowed_record_domain_requires_qualified_input/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/captured_predicate_write_retires_membership/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/completed_membership_before_later_guarantee/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/domain_cast_adds_unproven_predicate/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/qualified_fixed_byte_array_constructor_snapshot/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/structural_qualification_transition_other_domain/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/domains/wide_array_literal_membership_unproven/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/drops/parent_cleanup_wrong_child_guarantee/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_local_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/abstract_signature_call_through_field_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/address_count_partition_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/affine_discard_root_cleanup_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/applied_requirement_indices_through_field_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_input_authority_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_input_authority_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_result_bound_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/atomic_bounded_fetch/other_machine/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/atomic_bounded_resident/match_join/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/body_bearing_comparison_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/body_bearing_comparison_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/bounded_scalar_sums_through_field_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/call_result_bound_discharge_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/compound_index_guard_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/computed_record_window_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/copied_call_argument_snapshot_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected
- `tests/omega/fail/facts/copied_call_argument_snapshot_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected

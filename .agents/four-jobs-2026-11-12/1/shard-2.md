# Route coverage shard 2 (four-jobs 2026-11-12)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

## Host

```
Linux cursor 6.12.113+ #1 SMP PREEMPT_DYNAMIC Mon Nov  9 16:04:37 UTC 2026 x86_64 x86_64 x86_64 GNU/Linux
```

## Harness

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison
```

Binary: `/agent/repos/omega/target/debug/examples/route_one_comparison`

## Commands

```bash
# Fixture universe (full corpus; this shard = lines with i % 8 == S)
git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' \
  'tests/omega/fail/**/main.omg' | LC_ALL=C sort

# One fixture (fail/ adds --every-machine); timeout 300 per process
timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null
timeout 300 target/debug/examples/route_one_comparison --every-machine \
  <fail-fixture>/main.omg < /dev/null

python3 .agents/four-jobs-2026-11-12/1/run_shard.py 2 3
```

## Shard totals

- Fixtures in shard: **2174**
- Judged (accepted + rejected): **310**
- Outside: **1864**
- Timeout (separate bucket, not counted as outside): **0**
- Error: **0**
- Agree: **95**
- Disagree: **215**
- Agreement n/a: **1864**

### Route verdict counts

- `accepted`: 229
- `outside`: 1864
- `rejected`: 81

### Outside grouped by first refusal (top 30)

- **Unsupported** (1583): `tests/omega/fail/arithmetic/affine_difference_unpaired_term/main.omg`, `tests/omega/fail/arithmetic/authored_requires_numeric_bound/main.omg`, `tests/omega/fail/arithmetic/collection_result_rebound_descriptor_in_later_state/main.omg`
- **no-entry** (150): `tests/omega/fail/build/native_rows_from_two_suppliers_conflict/library/main.omg`, `tests/omega/fail/build/retained_const_argument_out_of_carrier/shapes/main.omg`, `tests/omega/fail/core/content_projection_foreign_package/owner/main.omg`
- **unreached** (131): `tests/omega/fail/asm/arm_subs_wide_alias/main.omg`, `tests/omega/fail/asm/checked_fence_dmb_extra_operand/main.omg`, `tests/omega/fail/asm/cyclic_label/main.omg`

### Disagreements (route accepts, current rejects) — soundness watch

- `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg` (1.333s)
- `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg` (1.448s)
- `tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg` (1.423s)
- `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg` (1.490s)
- `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg` (1.413s)
- `tests/omega/fail/asm/instruction_disposition_preserved_premise/main.omg` (1.549s)
- `tests/omega/fail/asm/instruction_effect_missing_write/main.omg` (1.478s)
- `tests/omega/fail/asm/proposition_wrong_transparent_exit/main.omg` (1.474s)
- `tests/omega/fail/asm/skipped_type_error/main.omg` (1.344s)
- `tests/omega/fail/asm/wrong_label_exit_fact/main.omg` (1.466s)
- `tests/omega/fail/borrow/mutated_selector_entry_actual_case/main.omg` (1.540s)
- `tests/omega/fail/borrow/write_only_affine_replacement_reuse/main.omg` (1.455s)
- `tests/omega/fail/borrow/write_only_exclusive_owned_wrapper_new_backing_conflict/main.omg` (1.500s)
- `tests/omega/fail/borrow/write_only_reference_aggregate_replacement_detached_loan/main.omg` (1.527s)
- `tests/omega/fail/borrows/conditional_returned_descriptor_local_escape/main.omg` (1.516s)
- `tests/omega/fail/borrows/cyclic_arrival_duplicates_exclusive_reference/main.omg` (1.449s)
- `tests/omega/fail/borrows/dynamic_byte_result_retains_second_candidate/main.omg` (1.530s)
- `tests/omega/fail/borrows/guard_subject_alias_held_through_call/main.omg` (1.412s)
- `tests/omega/fail/borrows/local_subslice_live_parent_read_operand/main.omg` (1.413s)
- `tests/omega/fail/borrows/reference_aggregate_conditional_replacement_loan/main.omg` (1.601s)
- `tests/omega/fail/borrows/runtime_slice_result_retains_backing/main.omg` (1.543s)
- `tests/omega/fail/borrows/selected_reference_alias_local_escape/main.omg` (1.442s)
- `tests/omega/fail/borrows/terminal_case_retained_observation/main.omg` (1.489s)
- `tests/omega/fail/build/program_entry_returns_value/main.omg` (3.511s)
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` (3.791s)
- `tests/omega/fail/calls/discarded_call_result/main.omg` (1.472s)
- `tests/omega/fail/calls/resultless_return_contract/main.omg` (1.376s)
- `tests/omega/fail/capabilities/boundary_qualification_subject_rejected/main.omg` (1.379s)
- `tests/omega/fail/collections/guarded_computed_index_overrun/main.omg` (1.565s)
- `tests/omega/fail/collections/projected_range_call_shared_storage/main.omg` (1.366s)
- `tests/omega/fail/constants/affine_rational_named_constant/main.omg` (1.626s)
- `tests/omega/fail/contracts/cyclic_terminal_mutated_result/main.omg` (1.622s)
- `tests/omega/fail/contracts/float_field_crash_wrong_order/main.omg` (1.672s)
- `tests/omega/fail/contracts/later_arm_call_requires_unproven/main.omg` (1.932s)
- `tests/omega/fail/contracts/match_payload_guarantee/main.omg` (1.975s)
- `tests/omega/fail/contracts/named_state_prefix_does_not_inherit_machine_entry_facts/main.omg` (1.637s)
- `tests/omega/fail/contracts/primitive_float_equality_not_reflexive/main.omg` (1.549s)
- `tests/omega/fail/contracts/projected_result_guarantee_after_overwrite/main.omg` (1.540s)
- `tests/omega/fail/contracts/runtime_requirement_after_guard_write/main.omg` (1.577s)
- `tests/omega/fail/contracts/shared_field_current_arithmetic_out_of_bounds/main.omg` (1.543s)
- `tests/omega/fail/core/address_interval_partition_missing_count_bound/main.omg` (1.967s)
- `tests/omega/fail/core/content_conservation_constructed_duplicate_claim/main.omg` (1.539s)
- `tests/omega/fail/core/content_membership_return_fabricated/main.omg` (1.769s)
- `tests/omega/fail/core/content_partition_provider_changed_entry/main.omg` (1.972s)
- `tests/omega/fail/core/scoped_integer_call_guarantee_requires_formation/main.omg` (1.630s)
- `tests/omega/fail/core/terminal_private_call_crash_ceiling/main.omg` (1.554s)
- `tests/omega/fail/data/indexed_element_invariant_local_swapped/main.omg` (1.473s)
- `tests/omega/fail/data/reference_local_store_breaks_collection_predicate/main.omg` (1.728s)
- `tests/omega/fail/dependent/data_requires_invalid_store/main.omg` (1.649s)
- `tests/omega/fail/domains/carried_indexed_payload_qualification/main.omg` (1.694s)

### All disagreements

- `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg` route=accepted current=rejected
- `tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/instruction_disposition_preserved_premise/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/instruction_effect_missing_write/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/proposition_wrong_transparent_exit/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/skipped_type_error/main.omg` route=accepted current=rejected
- `tests/omega/fail/asm/wrong_label_exit_fact/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrow/mutated_selector_entry_actual_case/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrow/write_only_affine_replacement_reuse/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrow/write_only_exclusive_owned_wrapper_new_backing_conflict/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrow/write_only_reference_aggregate_replacement_detached_loan/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/conditional_returned_descriptor_local_escape/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/cyclic_arrival_duplicates_exclusive_reference/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/dynamic_byte_result_retains_second_candidate/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/guard_subject_alias_held_through_call/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/local_subslice_live_parent_read_operand/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/reference_aggregate_conditional_replacement_loan/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/runtime_slice_result_retains_backing/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/selected_reference_alias_local_escape/main.omg` route=accepted current=rejected
- `tests/omega/fail/borrows/terminal_case_retained_observation/main.omg` route=accepted current=rejected
- `tests/omega/fail/build/program_entry_returns_value/main.omg` route=accepted current=rejected
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` route=accepted current=rejected
- `tests/omega/fail/calls/discarded_call_result/main.omg` route=accepted current=rejected
- `tests/omega/fail/calls/resultless_return_contract/main.omg` route=accepted current=rejected
- `tests/omega/fail/capabilities/boundary_qualification_subject_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/collections/guarded_computed_index_overrun/main.omg` route=accepted current=rejected
- `tests/omega/fail/collections/projected_range_call_shared_storage/main.omg` route=accepted current=rejected
- `tests/omega/fail/constants/affine_rational_named_constant/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/cyclic_terminal_mutated_result/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/float_field_crash_wrong_order/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/later_arm_call_requires_unproven/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/match_payload_guarantee/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/named_state_prefix_does_not_inherit_machine_entry_facts/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/primitive_float_equality_not_reflexive/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/projected_result_guarantee_after_overwrite/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/runtime_requirement_after_guard_write/main.omg` route=accepted current=rejected
- `tests/omega/fail/contracts/shared_field_current_arithmetic_out_of_bounds/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/address_interval_partition_missing_count_bound/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/content_conservation_constructed_duplicate_claim/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/content_membership_return_fabricated/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/content_partition_provider_changed_entry/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/scoped_integer_call_guarantee_requires_formation/main.omg` route=accepted current=rejected
- `tests/omega/fail/core/terminal_private_call_crash_ceiling/main.omg` route=accepted current=rejected
- `tests/omega/fail/data/indexed_element_invariant_local_swapped/main.omg` route=accepted current=rejected
- `tests/omega/fail/data/reference_local_store_breaks_collection_predicate/main.omg` route=accepted current=rejected
- `tests/omega/fail/dependent/data_requires_invalid_store/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/carried_indexed_payload_qualification/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/case_payload_guard_exclusion_cannot_read/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/completed_routed_record_join_unissued_arm/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/qualified_fixed_byte_array_write_only_invalid/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/routed_qualification_invalidated_field/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/structural_qualification_transition_routed/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/terminal_result_domain_unestablished/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/write_only_bare_helper_cannot_erase_field_default/main.omg` route=accepted current=rejected
- `tests/omega/fail/drops/composed_cleanup_crash_outcome/main.omg` route=accepted current=rejected
- `tests/omega/fail/expressions/declared_operator_result_loan_write_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_match_join_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/abstract_signature_call_through_local_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/address_count_partition_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/address_count_partition_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/affine_discard_root_cleanup_through_parameter_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/applied_requirement_indices_through_local_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_input_authority_through_const_name_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_result_bound_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/assumed_binding_result_bound_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/atomic_bounded_fetch/other_state/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/atomic_bounded_resident/other_machine/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/body_bearing_comparison_through_const_name_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/compound_index_guard_through_other_machine_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/computed_record_window_through_other_state_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/copied_call_argument_snapshot_through_const_name_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/correlated_invariant_store_through_other_machine_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/default_domain_field_write_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/default_domain_field_write_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_match_join_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/descriptor_range_exclusive_loan_through_local_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/domain_predicate_call_through_other_machine_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/domain_selected_cleanup_conflicting_selections_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/erased_reference_contract_through_match_join_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/exclusive_match_arm_through_const_name_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/exclusive_observation_exposure_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/exclusive_observation_exposure_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/float_comparison_route_through_const_name_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/float_range_delivery_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/float_range_delivery_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/float_result_equality_through_other_state_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/generic_requirement_statement_call_through_match_join_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/indexed_element_mut_loan_through_parameter_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/induction_call_guarantee_through_local_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/length_through_other_machine_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/linear_owner_borrowed_projection_field/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/machine_tuple_dispatch_through_match_join_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/match_computed_root_through_local_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/match_effectful_subject_custody_through_field_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/match_join_place_read_through_call_result_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/match_join_place_read_through_target_body_rejected/main.omg` route=accepted current=rejected
- `tests/omega/fail/facts/match_loan_last_use_through_parameter_rejected/main.omg` route=accepted current=rejected
- … and 115 more (see TSV)

# Job 1 route coverage — shard 0

- PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
- Harness build commit: `0835b3aaf803ed97f905be42d94727ff58459263` (binary from this checkout)
- Branch: `work/route-coverage-shard-0`
- Shard rule: fixture index `i % 8 == 0`
- Fixtures in shard: 2174 (completed 2174)

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
timeout 300 nice -n 19 target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

## Counts

| metric | count |
| --- | ---: |
| judged (route accepted or rejected) | 295 |
| agreed | 86 |
| disagreed | 209 |
| outside | 1879 |
| timeout | 0 |
| error | 0 |

## Top first-refusal constructs (outside)

| construct | count |
| --- | ---: |
| Unsupported: machine service reach | 331 |
| Unsupported: parameter mutable parameter | 272 |
| Unsupported: machine generic parameters | 180 |
| no-entry | 153 |
| Unsupported: type reference | 144 |
| Unsupported: declaration Build selection that is not a machine or a data type | 130 |
| unreached | 119 |
| Unsupported: type data with custody | 100 |
| Unsupported: machine attached data | 82 |
| Unsupported: type constrained | 56 |
| Unsupported: type float | 41 |
| Unsupported: declaration const | 25 |
| Unsupported: type fixed array | 24 |
| Unsupported: contract contracts on a state | 24 |
| Unsupported: expression match on a scalar subject | 19 |
| Unsupported: machine type equations, owned data or conformances | 17 |
| Unsupported: machine assembly target | 15 |
| Unsupported: type generic data | 14 |
| Unsupported: machine suspends or blocks | 11 |
| Unsupported: contract term other than a formal or literal | 10 |
| Unsupported: contract clause that is not a comparison or conjunction | 9 |
| Unsupported: type addr | 8 |
| Unsupported: machine invokes | 8 |
| Unsupported: machine supply mode other than a checked body | 7 |
| Unsupported: contract ranking witness | 6 |

### Examples (top five refusals)

| first refusal | count | example fixtures |
| --- | ---: | --- |
| Unsupported: machine service reach | 331 | `fail/asm/instruction_disposition_no_effect`, `fail/borrows/array_carried_reference_live_source_write`, `fail/borrows/foreign_output_copied_lifetime_source_write` |
| Unsupported: parameter mutable parameter | 272 | `fail/arithmetic/call_argument_snapshot_not_current_field`, `fail/arithmetic/collection_result_false_loop_bound`, `fail/arithmetic/repeated_call_argument_snapshots_are_distinct` |
| Unsupported: machine generic parameters | 180 | `fail/borrow/selector_calls_do_not_share_write_frame`, `fail/borrows/computed_byte_field_window_wrong_nominal`, `fail/borrows/copied_active_case_snapshot_retains_backing` |
| no-entry | 153 | `fail/arithmetic/qualified_float_cast_specification_fraction_rejected`, `fail/generics/dependent_const_named_carrier`, `pass/arithmetic/anonymous_rational_arguments` |
| Unsupported: type reference | 144 | `fail/atomics/atomic_operand_implicit_narrowing`, `fail/borrow/write_only_predicate_argument_dependency`, `fail/borrows/affine_array_projection_copy` |

## Route accepts, current rejects (soundness candidates)

- `tests/omega/fail/arithmetic/boolean_shift_guard_authored_comparison/main.omg`
- `tests/omega/fail/arithmetic/const_binder_unbounded_product/main.omg`
- `tests/omega/fail/arithmetic/guarded_read_count_unguarded_arrival/main.omg`
- `tests/omega/fail/arithmetic/implicit_fractional_integer_landing/main.omg`
- `tests/omega/fail/arithmetic/indexed_divisor_storage_write/main.omg`
- `tests/omega/fail/arithmetic/lifetime_provider_slice_count_cast_requires_fit/main.omg`
- `tests/omega/fail/arithmetic/wrapping_guard_does_not_bound_original/main.omg`
- `tests/omega/fail/asm/atomic_release_disposition_impostor/main.omg`
- `tests/omega/fail/asm/atomic_release_two_hop_rebound_descriptor/main.omg`
- `tests/omega/fail/asm/instruction_effect_helper_stale_operand/main.omg`
- `tests/omega/fail/asm/instruction_effect_wrong_byte/main.omg`
- `tests/omega/fail/asm/proposition_write_invalidates_ensures/main.omg`
- `tests/omega/fail/borrow/captured_selector_result_retires_written_bound/main.omg`
- `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_child/main.omg`
- `tests/omega/fail/borrows/byte_view_shared_element_mutable_reference/main.omg`
- `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg`
- `tests/omega/fail/borrows/dropped_reference_qualification_private_alternative/main.omg`
- `tests/omega/fail/borrows/generic_result_retains_lifetime_candidates/main.omg`
- `tests/omega/fail/borrows/indexed_carrier_conditional_retains_backing/main.omg`
- `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg`
- `tests/omega/fail/borrows/owned_input_reference_escape/main.omg`
- `tests/omega/fail/borrows/record_result_retains_declared_lifetime_origins/main.omg`
- `tests/omega/fail/borrows/returned_indexed_case_out_of_bounds/main.omg`
- `tests/omega/fail/borrows/selected_reference_mutation_live_backing_write/main.omg`
- `tests/omega/fail/borrows/tail_recursive_mutation_retires_field_bound/main.omg`
- `tests/omega/fail/boundary/bare_field_value_rejected/main.omg`
- `tests/omega/fail/build/calling_policy_unproved_helper/main.omg`
- `tests/omega/fail/build/explicit_provider_storage_extent/backing/main.omg`
- `tests/omega/fail/build/program_entry_binding_outside_build/main.omg`
- `tests/omega/fail/calling/call_plan_count_invalid_handback/main.omg`
- `tests/omega/fail/calls/const_boundary_rejects_other_bound/main.omg`
- `tests/omega/fail/calls/constructor_child_named_state_epochs_not_equal/main.omg`
- `tests/omega/fail/calls/ordered_terminal_skipped_arm_type_rejected/main.omg`
- `tests/omega/fail/calls/requires_machine_outer_cannot_read_nested_parameter/main.omg`
- `tests/omega/fail/capabilities/native_slice_external_leaf_rejected/main.omg`
- `tests/omega/fail/cases/borrowed_case_component_cannot_recover_common_fields/main.omg`
- `tests/omega/fail/collections/projected_range_call_selected_extent/main.omg`
- `tests/omega/fail/concurrency/suspend_retained_loan_conflicting_backing_write/main.omg`
- `tests/omega/fail/contracts/aggregate_result_false_guarantee/main.omg`
- `tests/omega/fail/contracts/cyclic_terminal_changed_input/main.omg`
- `tests/omega/fail/contracts/float_field_crash_stale_write/main.omg`
- `tests/omega/fail/contracts/mutable_owned_call_ensures_keeps_poststate/main.omg`
- `tests/omega/fail/contracts/nested_call_outer_write_clears_storage_fact/main.omg`
- `tests/omega/fail/contracts/opaque_call_retires_reference_contents/main.omg`
- `tests/omega/fail/contracts/returned_owned_field_wrong_subject/main.omg`
- `tests/omega/fail/contracts/slice_descriptor_does_not_preserve_mutated_contents/main.omg`
- `tests/omega/fail/control_flow/saved_case_predicate_local_write_retires/main.omg`
- `tests/omega/fail/core/content_membership_return_direct_adapter/main.omg`
- `tests/omega/fail/core/content_membership_return_unsupported_arm/main.omg`
- `tests/omega/fail/core/scoped_integer_call_actual_requires_selected_meaning/main.omg`
- `tests/omega/fail/core/terminal_call_guard_evaluation_not_selected/main.omg`
- `tests/omega/fail/data/boundary_data_relaxed_carry_unadmitted/main.omg`
- `tests/omega/fail/data/opaque_copy_unselected/main.omg`
- `tests/omega/fail/domains/case_carrier_array_rejected/main.omg`
- `tests/omega/fail/domains/issued_record_unknown_branch/main.omg`
- `tests/omega/fail/domains/scalar_domain_unrelated_call_result/main.omg`
- `tests/omega/fail/drops/parent_cleanup_requires_before_guarantees/main.omg`
- `tests/omega/fail/effects/finite_requirement_block_required/main.omg`
- `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_field_rejected/main.omg`
- `tests/omega/fail/facts/abstract_signature_call_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/address_count_partition_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/affine_discard_root_cleanup_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/applied_requirement_indices_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/assumed_binding_input_authority_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/assumed_binding_result_bound_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/atomic_bounded_fetch/match_join/main.omg`
- `tests/omega/fail/facts/atomic_bounded_resident/local/main.omg`
- `tests/omega/fail/facts/body_bearing_comparison_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/call_result_bound_discharge_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/compound_index_guard_through_local_rejected/main.omg`
- `tests/omega/fail/facts/computed_record_window_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/copied_call_argument_snapshot_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/correlated_invariant_store_through_local_rejected/main.omg`
- `tests/omega/fail/facts/default_domain_field_write_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_field_rejected/main.omg`
- `tests/omega/fail/facts/domain_predicate_call_through_local_rejected/main.omg`
- `tests/omega/fail/facts/domain_selected_cleanup_match_erasure_bypass_rejected/main.omg`
- `tests/omega/fail/facts/erased_reference_contract_through_field_rejected/main.omg`
- `tests/omega/fail/facts/exclusive_match_arm_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/exclusive_observation_exposure_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/float_comparison_route_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/float_range_delivery_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/float_result_equality_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/generic_comparison_body_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/generic_requirement_statement_call_through_field_rejected/main.omg`
- `tests/omega/fail/facts/indexed_element_mut_loan_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/induction_call_guarantee_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/length_through_local_rejected/main.omg`
- `tests/omega/fail/facts/linear_owner_borrowed_projection_call_result/main.omg`
- `tests/omega/fail/facts/machine_tuple_dispatch_through_field_rejected/main.omg`
- `tests/omega/fail/facts/match_computed_root_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/match_effectful_subject_custody_through_call_result_rejected/main.omg`
- `tests/omega/fail/facts/match_effectful_subject_custody_through_target_body_rejected/main.omg`
- `tests/omega/fail/facts/match_join_place_read_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/match_loan_last_use_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/match_primitive_local_origin_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/match_runtime_view_extent_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/matched_field_windows_through_call_result_rejected/main.omg`
- `tests/omega/fail/facts/matched_field_windows_through_target_body_rejected/main.omg`
- `tests/omega/fail/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/nested_result_extent_through_local_rejected/main.omg`
- `tests/omega/fail/facts/nominal_callback_reach_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/predicate_type_formation_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/projection_receiver_call_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/range_fact_delivery_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/result_cleanup_guarantees_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_call_index_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_computed_result_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_contract_index_through_field_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_mutating_operator_through_parameter_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_ordinary_call_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/runtime_index_application_qualified_computed_through_match_join_rejected/main.omg`
- `tests/omega/fail/facts/saved_observation_snapshot_after_call_through_other_machine_rejected/main.omg`
- `tests/omega/fail/facts/saved_observation_snapshot_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/scoped_scalar_initializer_through_field_rejected/main.omg`
- `tests/omega/fail/facts/selected_scalar_initializer_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/signed_arithmetic_shift_right_through_const_name_rejected/main.omg`
- `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_call_result_rejected/main.omg`
- `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_target_body_rejected/main.omg`
- `tests/omega/fail/facts/unannotated_direct_boundary_call_through_other_state_rejected/main.omg`
- `tests/omega/fail/facts/write_only_match_join_through_call_result_rejected/main.omg`
- `tests/omega/fail/facts/write_only_match_join_through_target_body_rejected/main.omg`
- `tests/omega/fail/generics/captured_map_nominal_open_const_mismatch/main.omg`
- `tests/omega/fail/generics/concrete_applied_argument_primitive_mismatch/main.omg`
- `tests/omega/fail/generics/const_data_argument_requires_value/main.omg`
- `tests/omega/fail/generics/const_extent_index_without_guard/main.omg`
- `tests/omega/fail/generics/erased_static_const_keeps_declared_carrier/main.omg`
- `tests/omega/fail/generics/field_store_generic_result_parameter_mismatch/main.omg`
- `tests/omega/fail/generics/generic_field_read_transition_value_mismatch/main.omg`
- `tests/omega/fail/generics/local_generic_result_parameter_mismatch/main.omg`
- `tests/omega/fail/generics/phantom_owned_constructor_unproved/main.omg`
- `tests/omega/fail/generics/runtime_index_carrier_call_selected_mutated/main.omg`
- `tests/omega/fail/generics/runtime_value_recipe_typed_narrow_overflow/main.omg`
- `tests/omega/fail/generics/static_callable_unused_local_type_argument_missing/main.omg`
- `tests/omega/fail/host/conditional_process_exit_keeps_return_obligations/main.omg`
- `tests/omega/fail/host/process_exit_reach_keeps_return_obligations/main.omg`
- `tests/omega/fail/layouts/declaration_only_initializer_formation/main.omg`
- `tests/omega/fail/layouts/encoded_view_opaque_write_requires_fit/main.omg`
- `tests/omega/fail/layouts/placed_stable_array_index_unbounded/main.omg`
- `tests/omega/fail/layouts/unused_generic_concrete_integer_at_requires_fit/main.omg`
- `tests/omega/fail/memory/indexed_resident_double_extraction/main.omg`
- `tests/omega/fail/memory/bump_allocator_release_with_live_resident/main.omg`
- `tests/omega/fail/memory/typed_run_empty_nonempty_backing/main.omg`
- `tests/omega/fail/modules/array_extent_type_parameter_shadow/main.omg`
- `tests/omega/fail/operators/domain_qualified_boundary_operator_unestablished_literal/main.omg`
- `tests/omega/fail/ownership/borrowed_content_partition_wrong_conservation/main.omg`
- `tests/omega/fail/ownership/borrowed_window_changed_selector/main.omg`
- `tests/omega/fail/ownership/completed_case_claim_return_double_consume/main.omg`
- `tests/omega/fail/ownership/partial_linear_record_cleanup_keeps_debt/main.omg`
- `tests/omega/fail/proofs/addition_commutativity_false_twin/main.omg`
- `tests/omega/fail/proofs/assignment_parameter_slot_retargeted/main.omg`
- `tests/omega/fail/proofs/call_argument_snapshot_precedes_write/main.omg`
- `tests/omega/fail/proofs/congruence_false_twin/main.omg`
- `tests/omega/fail/proofs/declaration_call_arity/main.omg`
- `tests/omega/fail/proofs/declaration_predicate_shadowed_embedding/main.omg`
- `tests/omega/fail/proofs/mathematical_opaque_operation_source_meaning/main.omg`
- `tests/omega/fail/proofs/nested_machine_rank_bypassed/main.omg`
- `tests/omega/fail/proofs/ordinary_denotational_operator_requires_normal_return/main.omg`
- `tests/omega/fail/proofs/numeric_state_unknown_arrival/main.omg`
- `tests/omega/fail/proofs/proof_integer_named_wrapping_operand/main.omg`
- `tests/omega/fail/proofs/proof_integer_unformed_quotient/main.omg`
- `tests/omega/fail/proofs/recursive_formals_do_not_read_caller_constants/main.omg`
- `tests/omega/fail/proofs/scalar_field_snapshot_after_indexed_store/main.omg`
- `tests/omega/fail/proofs/scalar_state_return_outside_byte_class/main.omg`
- `tests/omega/fail/proofs/self_call_without_witness_not_denotational/main.omg`
- `tests/omega/fail/proofs/value_call_refuted_inequality_rejected/main.omg`
- `tests/omega/fail/providers/linear_structural_provider_duplicate/main.omg`
- `tests/omega/fail/providers/provider_receiver_applied_trait_rejected/main.omg`
- `tests/omega/fail/providers/via_signature_mismatch_rejected/main.omg`
- `tests/omega/fail/recast/non_owning_erasure_false_reestablishment/main.omg`
- `tests/omega/fail/recast/non_owning_erasure_preserves_record_field/main.omg`
- `tests/omega/fail/recast/returned_recast_requires_declared_access/main.omg`
- `tests/omega/fail/references/rebound_parameter_reference_drops_referent_bound/main.omg`
- `tests/omega/fail/slices/captured_range_end_after_scalar_replacement/main.omg`
- `tests/omega/fail/slices/edge_window_beyond_backing/main.omg`
- `tests/omega/fail/slices/matched_open_window_end/main.omg`
- `tests/omega/fail/slices/matched_window_subject_changed_by_field/main.omg`
- `tests/omega/fail/slices/returned_range_extent_contract_keeps_actual_descriptor/main.omg`
- `tests/omega/fail/slices/saved_view_predicate_after_write/main.omg`
- `tests/omega/fail/slices/state_projected_bound_wrong_field/main.omg`
- `tests/omega/fail/slices/unguarded_slice_parameter_index_compile/main.omg`
- `tests/omega/fail/termination/nested_helper_record_measure_result_equation_wrong_subject/main.omg`
- `tests/omega/fail/termination/selected_result_rank_arrival_nondecreasing/main.omg`
- `tests/omega/fail/traits/conformance_phantom_foreign_captured_operand/main.omg`
- `tests/omega/fail/traits/evidence_binder_distinct_lifetime_arguments/main.omg`
- `tests/omega/fail/traits/exact_edge_application_cannot_quantify_invocations/main.omg`
- `tests/omega/fail/traits/family_carrier_pack_dependent_storage_rejected/main.omg`
- `tests/omega/fail/traits/named_const_as_field_type/main.omg`
- `tests/omega/fail/traits/owned_row_captured_region_not_invocation/main.omg`
- `tests/omega/fail/traits/selection_row_law_other_operation/main.omg`
- `tests/omega/fail/traits/structural_identity_stale_requirement/main.omg`
- `tests/omega/fail/types/exact_address_conversion_fixed_bound/main.omg`

## All disagreements

- `tests/omega/fail/arithmetic/boolean_shift_guard_authored_comparison/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/const_binder_unbounded_product/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/guarded_read_count_unguarded_arrival/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/implicit_fractional_integer_landing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/indexed_divisor_storage_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/lifetime_provider_slice_count_cast_requires_fit/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/arithmetic/wrapping_guard_does_not_bound_original/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/asm/atomic_release_disposition_impostor/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/asm/atomic_release_two_hop_rebound_descriptor/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/asm/instruction_effect_helper_stale_operand/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/asm/instruction_effect_wrong_byte/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/asm/proposition_write_invalidates_ensures/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrow/captured_selector_result_retires_written_bound/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_child/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/byte_view_shared_element_mutable_reference/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/dropped_reference_qualification_private_alternative/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/generic_result_retains_lifetime_candidates/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/indexed_carrier_conditional_retains_backing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/owned_input_reference_escape/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/record_result_retains_declared_lifetime_origins/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/returned_indexed_case_out_of_bounds/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/selected_reference_mutation_live_backing_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/borrows/tail_recursive_mutation_retires_field_bound/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/boundary/bare_field_value_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/build/calling_policy_unproved_helper/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/build/explicit_provider_storage_extent/backing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/build/program_entry_binding_outside_build/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/calling/call_plan_count_invalid_handback/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/calls/const_boundary_rejects_other_bound/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/calls/constructor_child_named_state_epochs_not_equal/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/calls/ordered_terminal_skipped_arm_type_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/calls/requires_machine_outer_cannot_read_nested_parameter/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/capabilities/native_slice_external_leaf_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/cases/borrowed_case_component_cannot_recover_common_fields/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/collections/projected_range_call_selected_extent/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/concurrency/suspend_retained_loan_conflicting_backing_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/aggregate_result_false_guarantee/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/cyclic_terminal_changed_input/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/float_field_crash_stale_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/mutable_owned_call_ensures_keeps_poststate/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/nested_call_outer_write_clears_storage_fact/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/opaque_call_retires_reference_contents/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/returned_owned_field_wrong_subject/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/contracts/slice_descriptor_does_not_preserve_mutated_contents/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/control_flow/saved_case_predicate_local_write_retires/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/core/content_membership_return_direct_adapter/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/core/content_membership_return_unsupported_arm/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/core/scoped_integer_call_actual_requires_selected_meaning/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/core/terminal_call_guard_evaluation_not_selected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/data/boundary_data_relaxed_carry_unadmitted/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/data/opaque_copy_unselected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/domains/case_carrier_array_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/domains/issued_record_unknown_branch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/domains/scalar_domain_unrelated_call_result/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/drops/parent_cleanup_requires_before_guarantees/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/effects/finite_requirement_block_required/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/abstract_signature_call_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/address_count_partition_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/affine_discard_root_cleanup_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/applied_requirement_indices_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/assumed_binding_input_authority_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/assumed_binding_result_bound_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/atomic_bounded_fetch/match_join/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/atomic_bounded_resident/local/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/body_bearing_comparison_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/call_result_bound_discharge_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/compound_index_guard_through_local_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/computed_record_window_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/copied_call_argument_snapshot_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/correlated_invariant_store_through_local_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/default_domain_field_write_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/domain_predicate_call_through_local_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/domain_selected_cleanup_match_erasure_bypass_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/erased_reference_contract_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/exclusive_match_arm_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/exclusive_observation_exposure_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/float_comparison_route_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/float_range_delivery_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/float_result_equality_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/generic_comparison_body_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/generic_requirement_statement_call_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/indexed_element_mut_loan_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/induction_call_guarantee_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/length_through_local_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/linear_owner_borrowed_projection_call_result/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/machine_tuple_dispatch_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_computed_root_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_effectful_subject_custody_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_effectful_subject_custody_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_join_place_read_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_loan_last_use_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_primitive_local_origin_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/match_runtime_view_extent_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/matched_field_windows_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/matched_field_windows_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/nested_result_extent_through_local_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/nominal_callback_reach_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/predicate_type_formation_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/projection_receiver_call_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/range_fact_delivery_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/result_cleanup_guarantees_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_call_index_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_computed_result_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_contract_index_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_mutating_operator_through_parameter_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_ordinary_call_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/runtime_index_application_qualified_computed_through_match_join_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/saved_observation_snapshot_after_call_through_other_machine_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/saved_observation_snapshot_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/scoped_scalar_initializer_through_field_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/selected_scalar_initializer_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/signed_arithmetic_shift_right_through_const_name_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/unannotated_direct_boundary_call_through_other_state_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/write_only_match_join_through_call_result_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/facts/write_only_match_join_through_target_body_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/captured_map_nominal_open_const_mismatch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/concrete_applied_argument_primitive_mismatch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/const_data_argument_requires_value/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/const_extent_index_without_guard/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/erased_static_const_keeps_declared_carrier/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/field_store_generic_result_parameter_mismatch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/generic_field_read_transition_value_mismatch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/local_generic_result_parameter_mismatch/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/phantom_owned_constructor_unproved/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/runtime_index_carrier_call_selected_mutated/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/runtime_value_recipe_typed_narrow_overflow/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/generics/static_callable_unused_local_type_argument_missing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/host/conditional_process_exit_keeps_return_obligations/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/host/process_exit_reach_keeps_return_obligations/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/layouts/declaration_only_initializer_formation/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/layouts/encoded_view_opaque_write_requires_fit/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/layouts/placed_stable_array_index_unbounded/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/layouts/unused_generic_concrete_integer_at_requires_fit/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/memory/indexed_resident_double_extraction/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/memory/bump_allocator_release_with_live_resident/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/memory/typed_run_empty_nonempty_backing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/modules/array_extent_type_parameter_shadow/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/operators/domain_qualified_boundary_operator_unestablished_literal/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/ownership/borrowed_content_partition_wrong_conservation/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/ownership/borrowed_window_changed_selector/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/ownership/completed_case_claim_return_double_consume/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/ownership/partial_linear_record_cleanup_keeps_debt/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/addition_commutativity_false_twin/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/assignment_parameter_slot_retargeted/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/call_argument_snapshot_precedes_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/congruence_false_twin/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/declaration_call_arity/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/declaration_predicate_shadowed_embedding/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/mathematical_opaque_operation_source_meaning/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/nested_machine_rank_bypassed/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/ordinary_denotational_operator_requires_normal_return/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/numeric_state_unknown_arrival/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/proof_integer_named_wrapping_operand/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/proof_integer_unformed_quotient/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/recursive_formals_do_not_read_caller_constants/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/scalar_field_snapshot_after_indexed_store/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/scalar_state_return_outside_byte_class/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/self_call_without_witness_not_denotational/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/proofs/value_call_refuted_inequality_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/providers/linear_structural_provider_duplicate/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/providers/provider_receiver_applied_trait_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/providers/via_signature_mismatch_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/recast/non_owning_erasure_false_reestablishment/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/recast/non_owning_erasure_preserves_record_field/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/recast/returned_recast_requires_declared_access/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/references/rebound_parameter_reference_drops_referent_bound/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/captured_range_end_after_scalar_replacement/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/edge_window_beyond_backing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/matched_open_window_end/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/matched_window_subject_changed_by_field/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/returned_range_extent_contract_keeps_actual_descriptor/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/saved_view_predicate_after_write/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/state_projected_bound_wrong_field/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/slices/unguarded_slice_parameter_index_compile/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/termination/nested_helper_record_measure_result_equation_wrong_subject/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/termination/selected_result_rank_arrival_nondecreasing/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/conformance_phantom_foreign_captured_operand/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/evidence_binder_distinct_lifetime_arguments/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/exact_edge_application_cannot_quantify_invocations/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/family_carrier_pack_dependent_storage_rejected/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/named_const_as_field_type/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/owned_row_captured_region_not_invocation/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/selection_row_law_other_operation/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/traits/structural_identity_stale_requirement/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/fail/types/exact_address_conversion_fixed_bound/main.omg` tier=fail route=accepted current=rejected first_refusal=-
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_field_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/machines/private_division_exact_pair_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-

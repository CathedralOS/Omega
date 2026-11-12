# Route coverage map — 2026-11-12
Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`
Merged from eight shard branches (`work/route-coverage-shard-0` … `7`) at each branch head.

## Grand totals

| bucket | count |
| --- | ---: |
| accepted (judged) | 302 |
| rejected (judged) | 544 |
| judged (accepted + rejected) | 846 |
| agree | 694 |
| disagree | 152 |
| partial | 1554 |
| outside | 14501 |
| no-entry (program verdict) | 344 |
| timeout | 0 |
| error | 146 |
| **sum (all program_verdict buckets)** | **17391** |

Identity: agree + disagree = judged (694+152=846).

### Non-construct buckets (outside/partial rows)

| bucket | count |
| --- | ---: |
| no-entry | 1376 |
| checker-stopped | 528 |
| meta:no-entry | 521 |
| meta:no-bound-entry | 429 |
| unreached | 259 |
| meta:no-program-entry | 150 |
| no_program_entry | 24 |

## Top first-refusal constructs — OUTSIDE programs

| construct | programs blocked | example fixtures |
| --- | ---: | --- |
| machine service reach | 2676 | `tests/omega/fail/asm/instruction_disposition_no_effect/main.omg`, `tests/omega/fail/borrows/array_carried_reference_live_source_write/main.omg`, `tests/omega/fail/borrows/foreign_output_copied_lifetime_source_write/main.omg` |
| parameter mutable parameter | 1928 | `tests/omega/fail/arithmetic/call_argument_snapshot_not_current_field/main.omg`, `tests/omega/fail/arithmetic/collection_result_false_loop_bound/main.omg`, `tests/omega/fail/arithmetic/repeated_call_argument_snapshots_are_distinct/main.omg` |
| machine generic parameters | 1374 | `tests/omega/fail/borrow/selector_calls_do_not_share_write_frame/main.omg`, `tests/omega/fail/borrows/computed_byte_field_window_wrong_nominal/main.omg`, `tests/omega/fail/borrows/copied_active_case_snapshot_retains_backing/main.omg` |
| type reference | 1088 | `tests/omega/fail/atomics/atomic_operand_implicit_narrowing/main.omg`, `tests/omega/fail/borrow/write_only_predicate_argument_dependency/main.omg`, `tests/omega/fail/borrows/affine_array_projection_copy/main.omg` |
| declaration Build selection that is not a machine or a data type | 961 | `tests/omega/fail/build/package_owned_callable_identity/main.omg`, `tests/omega/fail/collections/typed_run_relocation_stable/main.omg`, `tests/omega/fail/constants/target_layout_derived_helper_alias_underflow_reject/main.omg` |
| type data with custody | 779 | `tests/omega/fail/arithmetic/array_constructor_divisor_requires_nonzero/main.omg`, `tests/omega/fail/arithmetic/zii_range_excludes_zero_rejected/main.omg`, `tests/omega/fail/blockexec/blocking_executor_ticket_scope_loss/main.omg` |
| machine attached data | 616 | `tests/omega/fail/arithmetic/float_policy_cast_fractional/main.omg`, `tests/omega/fail/arithmetic/narrowing_signedness_rejected/main.omg`, `tests/omega/fail/arithmetic/suffix_magnitude_overflow_rejected/main.omg` |
| type constrained | 411 | `tests/omega/fail/arithmetic/checked_flag_proof_saturating_cast_overflow/main.omg`, `tests/omega/fail/borrows/erased_cast_borrow_overlaps_mutable_borrow/main.omg`, `tests/omega/fail/borrows/owned_capture_replaced_reference_qualification/main.omg` |
| bucket:no-program-entry | 381 | `tests/omega/fail/arithmetic/affine_difference_wrong_coefficients/main.omg`, `tests/omega/fail/arithmetic/captured_boolean_replaced_condition/main.omg`, `tests/omega/fail/arithmetic/collection_result_window_metadata/main.omg` |
| type float | 261 | `tests/omega/fail/domains/terminal_result_domain_incompatible_carrier/main.omg`, `tests/omega/fail/facts/core_float_representation_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_ieee_view_through_field_rejected/main.omg` |

## Top first-refusal constructs — PARTIAL programs

| construct | programs blocked | example fixtures |
| --- | ---: | --- |
| parameter mutable parameter | 265 | `tests/omega/fail/arithmetic/indexed_divisor_storage_write/main.omg`, `tests/omega/fail/asm/proposition_write_invalidates_ensures/main.omg`, `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_child/main.omg` |
| machine generic parameters | 186 | `tests/omega/fail/arithmetic/const_binder_unbounded_product/main.omg`, `tests/omega/fail/borrow/captured_selector_result_retires_written_bound/main.omg`, `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg` |
| bucket:partial-unjudged-defect | 122 | `tests/omega/fail/arithmetic/full_width_literal_requires_explicit_policy_conversion/main.omg`, `tests/omega/fail/arithmetic/immutable_unsigned_shift_underbound_rejected/main.omg`, `tests/omega/fail/asm/finite_prefix_skipped_fact/main.omg` |
| type reference | 121 | `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg`, `tests/omega/fail/borrows/returned_indexed_case_out_of_bounds/main.omg`, `tests/omega/fail/cases/borrowed_case_component_cannot_recover_common_fields/main.omg` |
| machine attached data | 78 | `tests/omega/fail/arithmetic/guarded_read_count_unguarded_arrival/main.omg`, `tests/omega/fail/arithmetic/lifetime_provider_slice_count_cast_requires_fit/main.omg`, `tests/omega/fail/arithmetic/wrapping_guard_does_not_bound_original/main.omg` |
| type constrained | 68 | `tests/omega/fail/core/content_membership_return_unsupported_arm/main.omg`, `tests/omega/fail/domains/issued_record_unknown_branch/main.omg`, `tests/omega/fail/domains/scalar_domain_unrelated_call_result/main.omg` |
| type data with custody | 55 | `tests/omega/fail/borrows/tail_recursive_mutation_retires_field_bound/main.omg`, `tests/omega/fail/contracts/aggregate_result_false_guarantee/main.omg`, `tests/omega/fail/layouts/encoded_view_opaque_write_requires_fit/main.omg` |
| contract contracts on a state | 42 | `tests/omega/fail/core/content_membership_return_direct_adapter/main.omg`, `tests/omega/fail/facts/applied_requirement_indices_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/linear_owner_borrowed_projection_call_result/main.omg` |
| machine type equations, owned data or conformances | 38 | `tests/omega/fail/asm/atomic_release_disposition_impostor/main.omg`, `tests/omega/fail/facts/nominal_callback_reach_through_other_state_rejected/main.omg`, `tests/omega/fail/ownership/borrowed_content_partition_wrong_conservation/main.omg` |
| type float | 32 | `tests/omega/fail/facts/float_comparison_route_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/float_range_delivery_through_other_state_rejected/main.omg`, `tests/omega/fail/proofs/ordinary_denotational_operator_requires_normal_return/main.omg` |
| machine service reach | 29 | `tests/omega/fail/contracts/opaque_call_retires_reference_contents/main.omg`, `tests/omega/fail/facts/assumed_binding_result_bound_through_other_state_rejected/main.omg`, `tests/omega/fail/host/conditional_process_exit_keeps_return_obligations/main.omg` |
| contract term other than a formal or literal | 20 | `tests/omega/fail/proofs/addition_commutativity_false_twin/main.omg`, `tests/omega/fail/proofs/congruence_false_twin/main.omg`, `tests/omega/fail/proofs/proof_integer_unformed_quotient/main.omg` |
| machine target selection | 18 | `tests/omega/fail/facts/matched_field_windows_through_target_body_rejected/main.omg`, `tests/omega/fail/calls/boundary_computed_selector_out_of_bounds/main.omg`, `tests/omega/fail/facts/float_comparison_route_through_target_body_rejected/main.omg` |
| partial-mixed | 18 | `tests/omega/fail/arithmetic/remainder_postcondition_signed_minimum_underbound_rejected/main.omg`, `tests/omega/fail/arithmetic/transitive_bit_guard_missing_arrival/main.omg`, `tests/omega/fail/arithmetic/widened_result_does_not_grant_stricter_bound/main.omg` |
| type fixed array | 17 | `tests/omega/fail/build/calling_policy_unproved_helper/main.omg`, `tests/omega/fail/facts/exclusive_observation_exposure_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/selected_scalar_initializer_through_other_state_rejected/main.omg` |

## Disagreements (judged rows only)

Total disagree rows: **152**.

By direction (`program_verdict` × `current_verdict`):

| program | current | count |
| --- | --- | ---: |
| rejected | accepted | 149 |
| accepted | rejected | 3 |

### Soundness watch (program accepted, current rejected)

Vacuous accepts (`obligations=0`, `judged=0`) are excluded from soundness-watch counts per merge rules.

**Non-vacuous watch rows:** 0

(none)

**Vacuous-accept route✓/current✗ (not soundness watch):** 3

- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` shard=2 vacuous=`*`
- `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` shard=4 vacuous=`*`
- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` shard=5 vacuous=`compilation`

**Documented build-entry gate gaps (re-checked; shard reports):**

- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` shard=2 vacuous=True (harness disagree; known route-one entry-policy gap)
- `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` shard=4 vacuous=True (harness disagree; known route-one entry-policy gap)

### All disagreement fixtures

- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` tier=fail program=accepted current=rejected **[vacuous-accept; not soundness watch]**
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` tier=fail program=accepted current=rejected **[watch / route-accept]**
- `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` tier=fail program=accepted current=rejected **[watch / route-accept]**
- `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_boolean_alias_shift_count_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_boolean_guarded_product_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/calls/private_arrival_identity_return_range_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/abi_multi_arg_spill_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_field_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/case_payload_transport_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/live_across_call_spill_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nested_record_value_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/scalar_sign_zero_extension_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/scalar_sign_zero_extension_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_field_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_arithmetic_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_multiplication_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_exact_subtraction_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_field_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_exact_subtraction_interval_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_call_result_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_local_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_machine_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_parameter_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/machines/private_division_exact_pair_excluded_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/machines/private_division_exact_pair_exit/main.omg` tier=pass program=rejected current=accepted
- `tests/omega/pass/machines/private_division_signed_remainder_exit/main.omg` tier=pass program=rejected current=accepted

## Vacuous accept roots

Rows listing vacuous accepts: **558** (see TSV `vacuous_accept_roots`).

- `tests/omega/fail/arithmetic/boolean_shift_guard_authored_comparison/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/guarded_read_count_unguarded_arrival/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/implicit_fractional_integer_landing/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/indexed_divisor_storage_write/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/lifetime_provider_slice_count_cast_requires_fit/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/truncating_reconstruction_negative_dividend/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/arithmetic/wrapping_guard_does_not_bound_original/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/asm/atomic_release_two_hop_rebound_descriptor/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/asm/instruction_effect_helper_stale_operand/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/asm/instruction_effect_wrong_byte/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/asm/proposition_write_invalidates_ensures/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/byte_view_shared_element_mutable_reference/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/dropped_reference_qualification_private_alternative/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/generic_result_retains_lifetime_candidates/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/indexed_carrier_conditional_retains_backing/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/record_result_retains_declared_lifetime_origins/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/returned_indexed_case_out_of_bounds/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/borrows/tail_recursive_mutation_retires_field_bound/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/boundary/bare_field_value_rejected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/build_fuel_exhausted/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/calling_policy_unproved_helper/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/explicit_provider_storage_extent/backing/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/generated_source_undeclared_call_refused/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/program_entry_binding_outside_build/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/build/staging_entry_limit_exhausted/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/calling/call_plan_count_invalid_handback/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/calls/const_boundary_rejects_other_bound/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/calls/constructor_child_named_state_epochs_not_equal/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/calls/ordered_terminal_skipped_arm_type_rejected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/calls/requires_machine_outer_cannot_read_nested_parameter/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/capabilities/native_slice_external_leaf_rejected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/cases/constructor_subject_range_unsigned_to_signed_unproved/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/collections/projected_range_call_selected_extent/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/concurrency/suspend_retained_loan_conflicting_backing_write/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/contracts/aggregate_result_false_guarantee/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/contracts/cyclic_terminal_changed_input/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/contracts/opaque_call_retires_reference_contents/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/contracts/returned_owned_field_wrong_subject/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/contracts/slice_descriptor_does_not_preserve_mutated_contents/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/control_flow/saved_case_predicate_local_write_retires/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/core/content_membership_return_direct_adapter/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/core/content_membership_return_unsupported_arm/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/data/boundary_data_relaxed_carry_unadmitted/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/data/opaque_copy_unselected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/domains/issued_record_unknown_branch/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/facts/call_result_bound_discharge_through_match_join_rejected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/facts/signed_arithmetic_shift_right_through_const_name_rejected/main.omg` — (all accepted roots vacuous)
- `tests/omega/fail/generics/concrete_applied_argument_primitive_mismatch/main.omg` — (all accepted roots vacuous)

… and 508 more in the merged TSV.

## Timeouts and near-timeouts

### Timeouts (300s cap)

(none)

### Near-timeout (≥ 250s)

- `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg` 297.260s program=outside shard=7
- `tests/omega/fail/build/installed_arrival_without_context/main.omg` 288.843s program=outside shard=6
- `tests/omega/pass/filesystem/runtime_portable_unknown_directory_kinds_exit/main.omg` 265.034s program=outside shard=3

## Per-shard reconciliation

| shard | branch head | rows | accepted | rejected | partial | outside | timeout | error | roots in TSV |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0 | `24d10db2781074710b41b1c5495b24eab944c937` | 2174 | 37 | 63 | 195 | 1879 | 0 | 0 | yes |
| 1 | `291fb6e56cc0cd2743154d3ccec0ae8b2d38386f` | 2174 | 38 | 65 | 208 | 1863 | 0 | 0 | no |
| 2 | `66cdb61be27b2405db0191b6be80cbe95e213926` | 2174 | 37 | 81 | 192 | 1864 | 0 | 0 | no |
| 3 | `bb1179a8fe7358d48fbac0876cdded62d8ea27a0` | 2174 | 34 | 62 | 205 | 1873 | 0 | 0 | no |
| 4 | `0d1fd30b689cbcd5b7910aff5ae6dd7c989834c7` | 2174 | 45 | 72 | 122 | 1445 | 0 | 146 | no |
| 5 | `a9ea414124571aa03f21473ddc5f6287bccf9050` | 2174 | 41 | 73 | 232 | 1828 | 0 | 0 | yes |
| 6 | `81936546a4f4cd3f9b14c75076aa428f14a429f8` | 2174 | 34 | 54 | 204 | 1882 | 0 | 0 | no |
| 7 | `fb6c06dfaf9c9e00b14b2f82ad061079c2180809` | 2173 | 36 | 74 | 196 | 1867 | 0 | 0 | yes |

**Shard inconsistency:** shards [1, 2, 3, 4, 6] ship no per-root verdict column in their TSV. `program_verdict` for those rows is taken from the shard's `route_verdict` without re-derivation from roots (normalization rule 1 not applied).

## Harness and hosts

- PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
- Hosts: Linux `cursor` cloud agents (x86_64), serial one-fixture-per-process per `tools/build_slot.md`.
- Shard 1 harness build note: `198b8cbff01c683fc49157e03474b5d3857c0765` (per shard-1 report).

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison < /dev/null
# pass/run:
timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null
# fail:
timeout 300 target/debug/examples/route_one_comparison --every-machine <fixture>/main.omg < /dev/null
```

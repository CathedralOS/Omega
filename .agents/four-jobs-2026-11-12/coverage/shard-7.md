# Route coverage shard 7 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

Host: `cursor` (linux x86_64)

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example route_one_comparison < /dev/null
python3 .agents/four-jobs-2026-11-12/coverage/run_shard7.py
# per fixture (serial): timeout 300 nice -n 19 target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

Fixtures in shard: 2173 (processed 2173)

## Totals (this shard)

| bucket | count |
| --- | ---: |
| fixtures | 2173 |
| accepted | 51 |
| rejected | 74 |
| outside | 2048 |
| timeout | 0 |
| error | 0 |
| judged (agree+disagree) | 306 |
| agree | 95 |
| disagree | 211 |

## Outside grouped by first refusal (ranked)

| first refusal | blocks | examples |
| --- | ---: | --- |
| machine service reach | 349 | `tests/omega/fail/asm/instruction_disposition_helper_bypass/main.omg`, `tests/omega/fail/borrows/byte_array_state_loan_wrong_extent/main.omg`, `tests/omega/fail/borrows/reference_array_result_duplicate_mutable/main.omg` |
| parameter mutable parameter | 300 | `tests/omega/fail/arithmetic/collection_result_endpoint_effect/main.omg`, `tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/main.omg`, `tests/omega/fail/arithmetic/unit_terminal_unguarded_cast/main.omg` |
| machine generic parameters | 213 | `tests/omega/fail/borrow/bounded_reference_store_preserves_range/main.omg`, `tests/omega/fail/borrow/nested_borrow_carrying_local_escape/main.omg`, `tests/omega/fail/borrow/selected_selector_relation_does_not_supply_equality/main.omg` |
| type reference | 167 | `tests/omega/fail/arithmetic/bounded_store_consumed_before_repair/main.omg`, `tests/omega/fail/arithmetic/indexed_divisor_selector_write/main.omg`, `tests/omega/fail/atomics/atomic_operand_call_argument/main.omg` |
| meta:no-bound-entry | 133 | `tests/omega/fail/modules/array_extent_private_constant/values/main.omg`, `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg`, `tests/omega/pass/arithmetic/compound_assign/main.omg` |
| meta:stopped-before-typed-frontier | 125 | `tests/omega/fail/arithmetic/implicit_fractional_const_landing/main.omg`, `tests/omega/fail/arithmetic/policy_operator_operand_types_rejected/main.omg`, `tests/omega/fail/asm/arm_subs_mixed_registers/main.omg` |
| type data with custody | 113 | `tests/omega/fail/arithmetic/erased_policy_store_into_qualified_element/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_zero_divisor/main.omg`, `tests/omega/fail/arithmetic/widened_result_predicate_invalid_write/main.omg` |
| declaration Build selection that is not a machine or a data type | 99 | `tests/omega/fail/asm/checked_fence_false_ensures/main.omg`, `tests/omega/fail/borrows/nested_machine_carried_staged_input_rejected/main.omg`, `tests/omega/fail/build/interrupt_return_vector_helper/main.omg` |
| machine attached data | 98 | `tests/omega/fail/arithmetic/guarded_read_count_stale_before_assignment/main.omg`, `tests/omega/fail/arithmetic/narrowing_literal_wider_than_target/main.omg`, `tests/omega/fail/borrow/vec_view_invalidated_by_push/main.omg` |
| type constrained | 65 | `tests/omega/fail/borrows/dropped_reference_qualification_mixed_origins/main.omg`, `tests/omega/fail/contracts/captured_predicate_arithmetic_policies/main.omg`, `tests/omega/fail/contracts/completed_anonymous_cast_maximum_index_rejected/main.omg` |
| type float | 41 | `tests/omega/fail/contracts/mixed_result_requirement_integer_bound/main.omg`, `tests/omega/fail/facts/core_float_representation_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_ieee_view_through_const_name_rejected/main.omg` |
| machine type equations, owned data or conformances | 36 | `tests/omega/fail/asm/atomic_release_disposition_changed_value/main.omg`, `tests/omega/fail/asm/atomic_release_disposition_wrong_width/main.omg`, `tests/omega/fail/calls/selected_crash_distinct_limits_rejected/main.omg` |
| type fixed array | 32 | `tests/omega/fail/borrows/collection_metadata_descriptor_slot_suspended/main.omg`, `tests/omega/fail/borrows/state_arrival_recast_keeps_backing_loan/main.omg`, `tests/omega/fail/build/calling_policy_missing_target_member/main.omg` |
| contract contracts on a state | 31 | `tests/omega/fail/borrows/nested_byte_field_capture_stale_carrier/main.omg`, `tests/omega/fail/build/uefi_entry_schema_wrong_owner/main.omg`, `tests/omega/fail/build/uefi_storage_entry_undeclared_port/main.omg` |
| declaration const | 26 | `tests/omega/fail/facts/bounded_products_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/computed_constant_array_extent_index/main.omg`, `tests/omega/fail/facts/foreign_scalar_backing_through_call_result_rejected/main.omg` |
| type generic data | 22 | `tests/omega/fail/borrows/borrowed_literal_backing_keeps_input_loan/main.omg`, `tests/omega/fail/dependent/match_generic_type_bound_wrong_arm/main.omg`, `tests/omega/fail/facts/atomic_try_exchange_once/other_machine/main.omg` |
| expression match on a scalar subject | 20 | `tests/omega/fail/expressions/match_mixed_result_domains/main.omg`, `tests/omega/fail/facts/dependency_type_qualified_call_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_match_join_rejected/main.omg` |
| machine assembly target | 16 | `tests/omega/fail/asm/checked_asm_compare_exit_goal_overwritten/main.omg`, `tests/omega/fail/asm/checked_asm_saved_match_overwritten/main.omg`, `tests/omega/fail/asm/instruction_effect_helper_return_mutation/main.omg` |
| contract term other than a formal or literal | 15 | `tests/omega/fail/arithmetic/exact_left_shift_invalid_count_rejected/main.omg`, `tests/omega/fail/contracts/match_contract_unknown_arm_rejected/main.omg`, `tests/omega/fail/core/scoped_integer_call_abstract_trait_operator_requires_formation/main.omg` |
| machine target selection | 15 | `tests/omega/fail/domains/target_issuer_wrong_machine/main.omg`, `tests/omega/fail/facts/bounded_scalar_sums_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/saturating_integer_arithmetic_through_target_body_rejected/main.omg` |
| contract clause that is not a comparison or conjunction | 13 | `tests/omega/fail/facts/abstract_signature_call_through_call_result_rejected/main.omg`, `tests/omega/fail/facts/abstract_signature_call_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/projection_call_premise_through_parameter_rejected/main.omg` |
| machine suspends or blocks | 11 | `tests/omega/fail/calls/recursive_terminal_block_missing_acknowledgement/main.omg`, `tests/omega/fail/calls/terminal_call_marked_state/main.omg`, `tests/omega/fail/facts/inline_member_helper_through_other_state_rejected/main.omg` |
| machine invokes | 9 | `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`, `tests/omega/fail/facts/finite_family_owned_field/target_body/main.omg`, `tests/omega/fail/facts/guarded_call_entry_through_target_body_rejected/main.omg` |
| typing literal landed type | 7 | `tests/omega/fail/arithmetic/checked_flag_proof_invalid_shift_count/main.omg`, `tests/omega/fail/facts/signed_8bit_shift_left_bound_through_local_rejected/main.omg`, `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_other_state_rejected/main.omg` |
| contract ranking witness | 7 | `tests/omega/fail/termination/constrained_measure_parameter_domain/main.omg`, `tests/omega/fail/termination/explicit_self_without_descent/main.omg`, `tests/omega/fail/termination/remainder_natural_descent/main.omg` |
| call compiler operation call | 6 | `tests/omega/fail/facts/fixed_array_layout_geometry_through_local_rejected/main.omg`, `tests/omega/fail/facts/slice_view_layout_geometry_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/value_location_alignment_through_other_state_rejected/main.omg` |
| declaration trait | 6 | `tests/omega/fail/facts/foreign_package_service_reach_through_other_state_rejected/main.omg`, `tests/omega/pass/facts/positional_implementation_binder_through_local_exit/main.omg`, `tests/omega/pass/providers/provider_type_slot_missing_selection/main.omg` |
| type addr | 5 | `tests/omega/fail/core/address_offset_nested_overflow/main.omg`, `tests/omega/fail/facts/domain_geometry_through_match_join_rejected/main.omg`, `tests/omega/fail/proofs/embedded_address_bound_not_strict/main.omg` |
| expression type parameter | 5 | `tests/omega/fail/facts/build_target_provider_through_field_rejected/reading_provider/main.omg`, `tests/omega/fail/facts/build_target_provider_through_other_state_rejected/reading_provider/main.omg`, `tests/omega/fail/facts/open_const_implementation_private_record_package_rejected/support/main.omg` |
| type generic application | 5 | `tests/omega/fail/facts/runtime_index_application_computed_result_through_local_rejected/main.omg`, `tests/omega/fail/generics/runtime_index_carrier_metadata_write/main.omg`, `tests/omega/pass/facts/runtime_index_application_computed_result_through_field_exit/main.omg` |
| call late-bound selection | 4 | `tests/omega/fail/borrows/predicate_named_call_keeps_installed_loan/main.omg`, `tests/omega/fail/calls/nested_machine_duplicate_helper_overload/main.omg`, `tests/omega/fail/calls/numeric_helper_abs_requires_declaration/main.omg` |
| call private machine of another package | 4 | `tests/omega/fail/build/executed_dependency_private_call/main.omg`, `tests/omega/fail/facts/core_integer_endpoint_through_local_rejected/main.omg`, `tests/omega/pass/core/integer_query_vectors_exit/main.omg` |
| contract result binding | 4 | `tests/omega/fail/calls/outcome_specific_primary_expression_mutated/main.omg`, `tests/omega/fail/facts/outcome_structural_evidence_through_parameter_rejected/main.omg`, `tests/omega/pass/calls/outcome_specific_snapshot_runtime/main.omg` |
| machine supply mode other than a checked body | 4 | `tests/omega/fail/capabilities/native_record_slice_external_leaf_rejected/main.omg`, `tests/omega/fail/contracts/returned_length_cannot_use_later_operand_guarantee/main.omg`, `tests/omega/pass/capabilities/external_leaf_binding_forms/main.omg` |
| machine operator spelling | 4 | `tests/omega/fail/facts/body_bearing_comparison_saved_field_result_rejected/main.omg`, `tests/omega/fail/facts/body_bearing_comparison_through_other_state_rejected/main.omg`, `tests/omega/fail/generics/const_extent_guard_closed_authored_comparison/main.omg` |
| contract disjunction | 4 | `tests/omega/fail/facts/nested_result_extent_through_field_rejected/main.omg`, `tests/omega/fail/proofs/integer_piecewise_wrong_result/main.omg`, `tests/omega/pass/calls/selected_contract_equation_reentry_exit/main.omg` |
| contract clause that projects a formal or result | 4 | `tests/omega/fail/facts/sret_aggregate_return_bound_through_other_machine_rejected/main.omg`, `tests/omega/pass/facts/nested_record_value_through_match_join_exit/main.omg`, `tests/omega/pass/facts/open_record_const_implementation_through_other_state_exit/main.omg` |
| type data without a checked shape | 3 | `tests/omega/fail/data/boundary_data_construction_rejected/main.omg`, `tests/omega/fail/facts/bounded_activation_claim_split_through_call_result_rejected/main.omg`, `tests/omega/pass/facts/bounded_activation_parameter_committed_exit/main.omg` |
| contract domain membership | 3 | `tests/omega/fail/domains/predicate_interval_premise_too_wide/main.omg`, `tests/omega/fail/facts/assumed_binding_input_authority_through_other_state_rejected/main.omg`, `tests/omega/pass/facts/domain_carrier_identity_through_match_join_exit/main.omg` |
| expression cast into a non-exact arithmetic domain | 2 | `tests/omega/fail/calls/final_value_recursive_operand_is_not_tail/main.omg`, `tests/omega/pass/arithmetic/observed_byte_cast_case_domain_policy_exit/main.omg` |
| expression match arm on a value pattern | 2 | `tests/omega/fail/facts/case_where_equality_data_predicate_through_field_rejected/main.omg`, `tests/omega/fail/facts/default_domain_field_write_through_other_machine_rejected/main.omg` |
| type dynamic trait | 2 | `tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_other_state_rejected/main.omg`, `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_other_state_exit/main.omg` |
| declaration domain | 2 | `tests/omega/fail/facts/negated_mathematical_order_through_local_rejected/main.omg`, `tests/omega/pass/facts/projection_receiver_call_through_match_join_exit/main.omg` |
| call receiver, requirement or coercion call | 2 | `tests/omega/fail/facts/unannotated_direct_boundary_call_through_other_machine_rejected/main.omg`, `tests/omega/fail/operators/named_unit_result_local_rejected/main.omg` |
| expression anonymous operation without an exact meaning | 1 | `tests/omega/fail/arithmetic/anonymous_remainder_rejected/main.omg` |
| expression cast naming a domain | 1 | `tests/omega/fail/arithmetic/qualified_cast_decision_wrong_arm/main.omg` |
| spelling name without a symbol | 1 | `tests/omega/fail/asm/checked_asm_nested_implicit_runtime_capture/main.omg` |
| contract case membership | 1 | `tests/omega/fail/atomics/fence_false_result_case/main.omg` |
| contract ensures on a call cycle | 1 | `tests/omega/fail/contracts/cyclic_terminal_bad_result/main.omg` |
| contract named proposition | 1 | `tests/omega/fail/contracts/undischarged_contract_entailment/main.omg` |

Meta buckets: meta:stopped-before-typed-frontier (125), meta:no-bound-entry (133)

## Agreement verification (hand-checked)

Agreement compares the harness `current=` verdict (typed-frontier package check, same field `route_one_comparison` prints) with whether any route root reached `route=accepted` or `route=rejected`. Fixtures where every root is `outside`, `no-entry`, `unreached`, `incomplete`, or `fallback` stay `n-a`.

**Parallelism.** Per `tools/build_slot.md`, test VMs run one heavy job per machine; this shard ran one fixture process at a time (serial), `timeout 300`, `nice -n 19`.

### Five disagreements (sample)

- `arithmetic`: route_verdict=rejected, current=accepted, agreement=disagree. Harness current accepts; route rejected on bound entry (stricter lowered checker).
- `calls`: route_verdict=rejected, current=accepted, agreement=disagree. Harness current accepts; route rejected on bound entry (stricter lowered checker).
- `borrow`: route_verdict=accepted, current=rejected, agreement=disagree. Harness current rejects; at least one machine root `route=accepted` (partial judgement).
- `modules`: route_verdict=accepted, current=rejected, agreement=disagree. Harness current rejects; at least one machine root `route=accepted` (partial judgement).
- `proofs`: route_verdict=accepted, current=rejected, agreement=disagree. Harness current rejects; at least one machine root `route=accepted` (partial judgement).

### Five route-accepts / current-rejects (soundness watch)

- `borrow`: agreement=disagree, route_verdict=accepted, current=rejected. Confirmed harness shows `current=rejected` with a `route=accepted` machine root while other roots may be `no-entry` or `outside` (n/a).
- `comptime`: agreement=disagree, route_verdict=accepted, current=rejected. Confirmed harness shows `current=rejected` with a `route=accepted` machine root while other roots may be `no-entry` or `outside` (n/a).
- `data`: agreement=disagree, route_verdict=accepted, current=rejected. Confirmed harness shows `current=rejected` with a `route=accepted` machine root while other roots may be `no-entry` or `outside` (n/a).
- `proofs`: agreement=disagree, route_verdict=accepted, current=rejected. Confirmed harness shows `current=rejected` with a `route=accepted` machine root while other roots may be `no-entry` or `outside` (n/a).
- `tests/omega/fail/calls/non_unit_compound_expression_requires_use/main.omg`: (not in shard TSV yet)

### Meta buckets (not construct refusals)

- `meta:no-bound-entry`: fail/pass fixture with no bound ProgramEntry; compilation root is not elaborated. With `--every-machine`, machine roots may still be judged.
- `meta:stopped-before-typed-frontier`: `current=stopped` before typed frontier; route never runs (`route=unreached`).
- `timeout`: wall clock `timeout 300` (exit 124). `installed_arrival_wrong_controller` in this shard finished in ~299s as `outside` / `current=accepted`, not a timeout.

## Disagreements

- `tests/omega/fail/borrow/lifetime_argument_arity/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/comptime/array_expression_unused_invalid_operands/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/data/property_carry_nested_restrictive_floor/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/modules/array_extent_ambiguous_imports/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/operators/domain_operator_return_only_overload/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/proofs/declaration_call_argument_type/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/proofs/mathematical_machine_result_requires/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/providers/applied_binding_unused_const_carrier/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/traits/const_selector_not_implementation/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/traits/named_const_argument_carrier/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/traits/ordinary_trait_inherits_boundary/main.omg` tier=fail route=accepted current=rejected **route accepts, current rejects**
- `tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/arithmetic/guarded_read_count_stale_before_assignment/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/arithmetic/indexed_divisor_selector_write/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/arithmetic/widened_result_predicate_invalid_write/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/atomic_release_disposition_changed_value/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/atomic_release_disposition_wrong_width/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/instruction_disposition_helper_bypass/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/instruction_effect_helper_return_mutation/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/instruction_effect_user_model/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/proposition_transfer_invalidates_ensures/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/asm/transition_argument_invalidates_exit_fact/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/atomics/atomic_operand_call_argument/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrow/selected_selector_relation_does_not_supply_equality/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrow/state_reference_permutation_duplicate_exclusive/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/borrowed_literal_backing_keeps_input_loan/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/completed_selector_join_unknown_arm/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/dropped_reference_qualification_mixed_origins/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/generic_result_keeps_lifetime_source/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/match_effectful_subject_fresh_overlap_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/reassigned_reference_preserves_child_backing/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/borrows/terminal_call_keeps_caller_cleanup_loan/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/calls/nested_machine_duplicate_helper_overload/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/calls/outcome_specific_primary_expression_mutated/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/capabilities/native_record_slice_external_leaf_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/concurrency/suspend_reference_permission_without_backing/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/cyclic_terminal_bad_result/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/float_field_crash_false_guard/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/generic_component_nondecreasing_window/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/indexed_store_retires_old_element_observation/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/nested_call_opaque_write_clears_storage_fact/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/opaque_call_retires_mutable_argument/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/recursive_private_arrival_exit/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/returned_length_cannot_use_later_operand_guarantee/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/saved_float_guard_after_write/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/contracts/shift_result_wrong_count/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/control_flow/saved_case_predicate_guard_write_retires/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/core/content_membership_return_consumed_issuance/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/core/content_reunion_mutated_result/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/dependent/data_requires_cyclic_handoff_store_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/dependent/symbolic_slice_predicate_owned_copy_snapshot/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/domains/bodyful_owner_establishment_bypass/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/domains/guard_predicate_wrong_bound_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/domains/returned_record_scalar_predicate_invalidation/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/domains/scalar_domain_unexecuted_call_guarantee/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/drops/narrowed_cleanup_requires_actual_element/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/drops/parent_cleanup_missing_child_guarantee/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/expressions/token_bound_machine_unproved_premise_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/abstract_signature_call_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/abstract_signature_call_through_parameter_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/address_count_partition_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/affine_discard_root_cleanup_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/applied_requirement_indices_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/assumed_binding_input_authority_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/assumed_binding_result_bound_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/atomic_bounded_fetch/local/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/atomic_bounded_resident/field/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/body_bearing_comparison_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/bounded_scalar_sums_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/call_result_bound_discharge_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/compound_index_guard_through_field_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/computed_record_window_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/correlated_invariant_store_through_field_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/default_domain_field_write_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/domain_predicate_call_through_field_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/erased_reference_contract_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/exclusive_match_arm_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/exclusive_observation_exposure_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/float_comparison_route_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/float_range_delivery_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/float_result_equality_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/generic_comparison_body_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/generic_requirement_statement_call_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/indexed_element_mut_loan_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/induction_call_guarantee_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/induction_call_guarantee_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/length_through_field_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/linear_owner_borrowed_projection_parameter/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/machine_tuple_dispatch_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_computed_root_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_computed_root_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_effectful_subject_custody_through_parameter_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_join_place_read_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_loan_last_use_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_primitive_local_origin_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_runtime_view_extent_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/match_runtime_view_extent_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/matched_field_windows_through_parameter_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/nested_result_extent_through_field_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/nominal_callback_reach_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/open_const_implementation_private_record_package_rejected/support/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/predicate_type_formation_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/projection_receiver_call_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/result_cleanup_guarantees_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_call_index_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_computed_result_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_contract_index_through_const_name_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_mutating_operator_through_other_state_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_ordinary_call_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/runtime_index_application_qualified_computed_through_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/saved_observation_snapshot_after_call_through_match_join_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/saved_observation_snapshot_through_call_result_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/saved_observation_snapshot_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/selected_scalar_initializer_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/signed_arithmetic_shift_right_through_target_body_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/static_callback_reach_recursive_terminal_rank/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_parameter_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/typed_run_nested_two_inputs/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/unannotated_direct_boundary_call_through_other_machine_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/facts/write_only_match_join_through_parameter_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/float/guarded_range_requires_current_observation/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/float/indexed_ieee_u64_upper_ceiling_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/float/nested_caller_snapshot_after_sibling_write/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/float/private_arrival_inexact_integer_coercion_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/captured_map_dynamic_open_method/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/captured_map_machine_contract_unused_effect/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/concrete_applied_argument_nested_mismatch/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/const_extent_inclusive_bound/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/contextual_static_machine_applied_evidence_effect_ceiling/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/contextual_static_machine_type_bound/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/erased_static_arguments_keep_binder_order/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/generic_field_read_state_argument_mismatch/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/nominal_binder_structural_coincidence_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/parameter_value_concrete_local_mismatch/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/runtime_value_recipe_forged_descriptor/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/runtime_value_recipe_selected_token_shared/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/generics/static_callable_local_type_argument_wrong_value/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/layouts/declaration_only_array_formation/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/layouts/resident_extraction_requires_ended_view/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/layouts/target_family_integer_at_requires_every_body_fit/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/machines/scalar_cast_for_data_result/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/memory/bump_allocator_projected_claim_duplicate/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/memory/indexed_resident_corrupt_handback/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/memory/typed_run_empty_forgery/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/operators/named_unit_result_local_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/ownership/borrowed_content_partition_unrestored/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/ownership/cyclic_arrival_restores_consumed_field/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/ownership/registration_owned_claim_cannot_requalify/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/ownership/successor_argument_write_keeps_linear_debt/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/assignment_frozen_selector_postcondition/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/conditional_ih_requires_discharge/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/defined_proposition_premise_not_symmetric/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/mathematical_term_in_runtime_arguments/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/numeric_state_selected_operator/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/proof_integer_embedding_runtime/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/proof_integer_unformed_dispatch_pattern/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/quotient_pattern_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/representation_agreement_empty_encoder/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/returned_field_embedded_sum_wrong_guarantee/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/scalar_field_snapshot_after_call/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/scalar_field_snapshot_selector_replaces_collection/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/static_machine_result_wrong_selection/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/proofs/unknown_boolean_return_is_not_omitted/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/providers/linear_provider_stronger_nominal_result/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/recast/non_owning_erasure_preserves_reborrow_alias/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/references/rebound_local_reference_drops_referent_bound/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/captured_range_end_after_descriptor_replacement/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/nested_stored_view_local_escape/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/projected_descriptor_index_requires_live_bound/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/rebound_subslice_loses_byte_predicate/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/returned_mutable_slice_overlapping_arguments/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/returned_shared_slice_mutable_call/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/state_projected_bound_rebound_selector/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/slices/written_subslice_loses_byte_predicate/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/targets/family_forwarded_callback_first_member_only_rejected/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/tasks/task_runtime_provider_contract_narrowing/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/bounded_distance_stalled_slice_length_cursor/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/constrained_measure_parameter_domain/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/nested_helper_record_measure_result_equation_not_decreasing/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/proof_joint_machine_cycle_unmeasured/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/runtime_same_pair_call_occurrence/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/termination/selected_result_rank_arrival_missing_guarantee/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/text/borrowed_byte_snapshot_unbounded/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/traits/captured_const_contract_distinct_binders/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/traits/external_row_does_not_capture_map_region/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/traits/selection_row_contract_not_refined/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/traits/structural_identity_rebound_reference/main.omg` tier=fail route=outside current=rejected
- `tests/omega/fail/types/exact_address_conversion_exclusive_bound/main.omg` tier=fail route=outside current=rejected
- `tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/calls/private_arrival_identity_return_range_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/case_payload_transport_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_local_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/scalar_sign_zero_extension_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_local_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_exact_subtraction_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/unsigned_exact_subtraction_interval_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**
- `tests/omega/pass/machines/private_division_exact_pair_excluded_exit/main.omg` tier=pass route=rejected current=accepted **route rejects, current accepts**

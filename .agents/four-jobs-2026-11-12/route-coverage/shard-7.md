# Route coverage shard 7 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

Host: `cursor` (linux x86_64)

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example route_one_comparison < /dev/null
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard7.py
# serial: timeout 300 nice -n 19 target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

**Verdict rules.** `accepted` only if every root is `route=accepted`. `rejected` if any root is `route=rejected`. `partial` if some roots accepted and some unjudged (`outside`, `no-entry`, `unreached`, etc.). `current_verdict` is harness `current=` from the typed-frontier package check (`accepted` / `rejected` / `stopped`); `stopped` is not treated as checker reject for agreement.

Fixtures in shard: 2173 (bucket sum 2173)

## Totals (this shard)

| bucket | count |
| --- | ---: |
| accepted | 36 |
| rejected | 74 |
| partial | 196 |
| outside | 1867 |
| error | 0 |
| timeout | 0 |
| **sum** | **2173** |
| agree | 90 |
| disagree | 20 |
| agree+disagree (= accepted+rejected rows) | 110 (= 110) |

## Vacuous accept roots (obligations=0, judged=0)

- **accepted** bucket: 6 roots across 6 fixtures
- **partial** bucket: 240 roots across 191 fixtures

## Near-timeout fixtures (finished under 300s limit)

- `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg` outside in 297.26s (flags=near-timeout)

## Outside grouped by first refusal (ranked)

| first refusal | blocks | examples |
| --- | ---: | --- |
| machine service reach | 343 | `tests/omega/fail/borrows/byte_array_state_loan_wrong_extent/main.omg`, `tests/omega/fail/borrows/reference_array_result_duplicate_mutable/main.omg`, `tests/omega/fail/borrows/stored_reference_leaf_child_live_source_write/main.omg` |
| parameter mutable parameter | 259 | `tests/omega/fail/arithmetic/collection_result_endpoint_effect/main.omg`, `tests/omega/fail/arithmetic/unit_terminal_unguarded_cast/main.omg`, `tests/omega/fail/borrow/relational_handback_false_return_guarantee/main.omg` |
| machine generic parameters | 182 | `tests/omega/fail/borrow/bounded_reference_store_preserves_range/main.omg`, `tests/omega/fail/borrow/nested_borrow_carrying_local_escape/main.omg`, `tests/omega/fail/borrows/consumed_nominal_carrier_retains_backing/main.omg` |
| type reference | 147 | `tests/omega/fail/arithmetic/bounded_store_consumed_before_repair/main.omg`, `tests/omega/fail/atomics/stored_reference_non_atomic_shared_store/main.omg`, `tests/omega/fail/borrows/float_selection_store_through_shared_receiver/main.omg` |
| meta:no-bound-entry | 133 | `tests/omega/fail/modules/array_extent_private_constant/values/main.omg`, `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg`, `tests/omega/pass/arithmetic/compound_assign/main.omg` |
| meta:stopped-before-typed-frontier | 125 | `tests/omega/fail/arithmetic/implicit_fractional_const_landing/main.omg`, `tests/omega/fail/arithmetic/policy_operator_operand_types_rejected/main.omg`, `tests/omega/fail/asm/arm_subs_mixed_registers/main.omg` |
| type data with custody | 106 | `tests/omega/fail/arithmetic/erased_policy_store_into_qualified_element/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_zero_divisor/main.omg`, `tests/omega/fail/atomics/atomic_boolean_ordinary_bool_load/main.omg` |
| declaration Build selection that is not a machine or a data type | 99 | `tests/omega/fail/asm/checked_fence_false_ensures/main.omg`, `tests/omega/fail/borrows/nested_machine_carried_staged_input_rejected/main.omg`, `tests/omega/fail/build/interrupt_return_vector_helper/main.omg` |
| machine attached data | 89 | `tests/omega/fail/arithmetic/narrowing_literal_wider_than_target/main.omg`, `tests/omega/fail/borrow/vec_view_invalidated_by_push/main.omg`, `tests/omega/fail/borrows/array_carried_reference_live_source_move/main.omg` |
| type constrained | 53 | `tests/omega/fail/contracts/captured_predicate_arithmetic_policies/main.omg`, `tests/omega/fail/contracts/completed_anonymous_cast_maximum_index_rejected/main.omg`, `tests/omega/fail/data/qualified_record_pattern_unrouted/main.omg` |
| type float | 38 | `tests/omega/fail/contracts/mixed_result_requirement_integer_bound/main.omg`, `tests/omega/fail/facts/core_float_representation_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_ieee_view_through_const_name_rejected/main.omg` |
| type fixed array | 30 | `tests/omega/fail/borrows/collection_metadata_descriptor_slot_suspended/main.omg`, `tests/omega/fail/borrows/state_arrival_recast_keeps_backing_loan/main.omg`, `tests/omega/fail/build/calling_policy_missing_target_member/main.omg` |
| contract contracts on a state | 26 | `tests/omega/fail/borrows/nested_byte_field_capture_stale_carrier/main.omg`, `tests/omega/fail/build/uefi_entry_schema_wrong_owner/main.omg`, `tests/omega/fail/build/uefi_storage_entry_undeclared_port/main.omg` |
| declaration const | 26 | `tests/omega/fail/facts/bounded_products_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/computed_constant_array_extent_index/main.omg`, `tests/omega/fail/facts/foreign_scalar_backing_through_call_result_rejected/main.omg` |
| machine type equations, owned data or conformances | 25 | `tests/omega/fail/calls/selected_crash_distinct_limits_rejected/main.omg`, `tests/omega/fail/core/independent_content_contract_input_unconserved/main.omg`, `tests/omega/fail/domains/inherited_requirement_issuer_wrong_owner/main.omg` |
| type generic data | 19 | `tests/omega/fail/dependent/match_generic_type_bound_wrong_arm/main.omg`, `tests/omega/fail/facts/atomic_try_exchange_once/other_machine/main.omg`, `tests/omega/fail/facts/representation_agreement_through_parameter_rejected/main.omg` |
| expression match on a scalar subject | 18 | `tests/omega/fail/expressions/match_mixed_result_domains/main.omg`, `tests/omega/fail/facts/dependency_type_qualified_call_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/live_across_call_spill_through_match_join_rejected/main.omg` |
| contract term other than a formal or literal | 14 | `tests/omega/fail/arithmetic/exact_left_shift_invalid_count_rejected/main.omg`, `tests/omega/fail/contracts/match_contract_unknown_arm_rejected/main.omg`, `tests/omega/fail/core/scoped_integer_call_abstract_trait_operator_requires_formation/main.omg` |
| machine assembly target | 14 | `tests/omega/fail/asm/checked_asm_compare_exit_goal_overwritten/main.omg`, `tests/omega/fail/asm/checked_asm_saved_match_overwritten/main.omg`, `tests/omega/fail/asm/scratch_invalid_backedge/main.omg` |
| machine target selection | 12 | `tests/omega/fail/domains/target_issuer_wrong_machine/main.omg`, `tests/omega/fail/facts/saturating_integer_arithmetic_through_target_body_rejected/main.omg`, `tests/omega/fail/modules/external_boundary_private_signature/main.omg` |
| machine suspends or blocks | 11 | `tests/omega/fail/calls/recursive_terminal_block_missing_acknowledgement/main.omg`, `tests/omega/fail/calls/terminal_call_marked_state/main.omg`, `tests/omega/fail/facts/inline_member_helper_through_other_state_rejected/main.omg` |
| contract clause that is not a comparison or conjunction | 11 | `tests/omega/fail/facts/projection_call_premise_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/quotient_closed_literal_through_match_join_rejected/main.omg`, `tests/omega/fail/proofs/mathematical_nullary_constructor_mismatch/main.omg` |
| machine invokes | 9 | `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`, `tests/omega/fail/facts/finite_family_owned_field/target_body/main.omg`, `tests/omega/fail/facts/guarded_call_entry_through_target_body_rejected/main.omg` |
| typing literal landed type | 7 | `tests/omega/fail/arithmetic/checked_flag_proof_invalid_shift_count/main.omg`, `tests/omega/fail/facts/signed_8bit_shift_left_bound_through_local_rejected/main.omg`, `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_other_state_rejected/main.omg` |
| declaration trait | 6 | `tests/omega/fail/facts/foreign_package_service_reach_through_other_state_rejected/main.omg`, `tests/omega/pass/facts/positional_implementation_binder_through_local_exit/main.omg`, `tests/omega/pass/providers/provider_type_slot_missing_selection/main.omg` |
| call compiler operation call | 5 | `tests/omega/fail/facts/fixed_array_layout_geometry_through_local_rejected/main.omg`, `tests/omega/fail/facts/slice_view_layout_geometry_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/value_location_alignment_through_other_state_rejected/main.omg` |
| contract ranking witness | 5 | `tests/omega/fail/termination/explicit_self_without_descent/main.omg`, `tests/omega/fail/termination/remainder_natural_descent/main.omg`, `tests/omega/pass/facts/loop_branch_relaxation_ranking_through_call_result_exit/main.omg` |
| call private machine of another package | 4 | `tests/omega/fail/build/executed_dependency_private_call/main.omg`, `tests/omega/fail/facts/core_integer_endpoint_through_local_rejected/main.omg`, `tests/omega/pass/core/integer_query_vectors_exit/main.omg` |
| type addr | 4 | `tests/omega/fail/core/address_offset_nested_overflow/main.omg`, `tests/omega/fail/facts/domain_geometry_through_match_join_rejected/main.omg`, `tests/omega/fail/proofs/embedded_address_bound_not_strict/main.omg` |
| contract clause that projects a formal or result | 4 | `tests/omega/fail/facts/sret_aggregate_return_bound_through_other_machine_rejected/main.omg`, `tests/omega/pass/facts/nested_record_value_through_match_join_exit/main.omg`, `tests/omega/pass/facts/open_record_const_implementation_through_other_state_exit/main.omg` |

## Route accepted, current rejects (soundness watch)

Count (route_verdict=accepted, current_verdict=rejected): 0

No rows with `route_verdict=accepted` and `current_verdict=rejected` under the all-roots-accepted rule. Former partial-accept rows (machine accepted + unjudged roots) are now `partial` and excluded.

| tier | checker diagnostic class | count (partial, current rejects) |
| --- | --- | ---: |
| fail | requirement/fact | 75 |
| fail | machine-body/effect | 42 |
| fail | other | 36 |
| fail | borrow/lifetime | 18 |
| fail | bound/index | 15 |
| fail | type/name | 3 |
| fail | arithmetic-safety | 1 |
| fail | trait/impl | 1 |

### Hand-check (five fixtures)

- `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`: route=outside, current=accepted, checker class=n/a. Not in route✓current✗ set (route=outside, current=accepted).
- `tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg`: route=rejected, current=accepted, checker class=n/a. Stricter route, not soundness watch (route rejects, current accepts).
- `tests/omega/fail/borrow/lifetime_argument_arity/main.omg`: route=partial, current=rejected, checker class=type/name. Not a soundness-watch candidate: route_verdict=partial (accepted mixed with unjudged roots).
- `tests/omega/fail/comptime/array_expression_unused_invalid_operands/main.omg`: route=partial, current=rejected, checker class=type/name. Not a soundness-watch candidate: route_verdict=partial (accepted mixed with unjudged roots).
- `tests/omega/fail/proofs/declaration_call_argument_type/main.omg`: route=partial, current=rejected, checker class=other. Not a soundness-watch candidate: route_verdict=partial (accepted mixed with unjudged roots).

## Disagreements (accepted/rejected rows only)

- `tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/calls/private_arrival_identity_return_range_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/case_payload_transport_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_local_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/scalar_sign_zero_extension_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_local_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_exact_subtraction_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/unsigned_exact_subtraction_interval_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)
- `tests/omega/pass/machines/private_division_exact_pair_excluded_exit/main.omg` tier=pass route=rejected current=accepted (route stricter)

# Route coverage map — 2026-11-12

Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`

Every `program_verdict` is **re-derived from `roots`** (uniform rule). Shard `route_verdict` / `program_verdict` columns are compared but not trusted for the merge.

## Grand totals

| bucket | count |
| --- | ---: |
| accepted (judged) | 301 |
| rejected (judged) | 576 |
| judged (accepted + rejected) | 877 |
| agree | 724 |
| disagree | 153 |
| partial | 1523 |
| outside | 14991 |
| timeout | 0 |
| **sum (program_verdict)** | **17391** |

Identity: agree + disagree = judged (724+153=877).

### Non-construct definitions (mutually exclusive on OUTSIDE rows)

Priority **checker-stopped → no-program-entry → no-entry → unreached → construct-blocked**. Each OUTSIDE fixture is assigned exactly one bucket.

| bucket | rule (first match wins) |
| --- | --- |
| **checker-stopped** | `current_verdict=stopped`, or `first_refusal` prefix `bucket:current-checker-stopped` / `current_stopped_before_typed_trees` |
| **no-program-entry** | `first_refusal` prefix `bucket:no-program-entry` or `meta:no-program-entry`, or exact legacy label `no_program_entry` (underscore spelling merged here) |
| **no-entry** | `first_refusal` prefix `meta:no-entry` or `meta:no-bound-entry` (bound entry missing; distinct from no-program-entry) |
| **unreached** | `first_refusal` prefix `meta:unreached` or root `compilation:unreached` (no unreached OUTSIDE rows remain after checker-stopped priority — all were `stopped`) |
| **construct-blocked** | any other OUTSIDE row; construct refusal in `first_refusal` |

### OUTSIDE identity

| bucket | count |
| --- | ---: |
| construct-blocked | 12259 |
| checker-stopped | 1061 |
| no-program-entry | 878 |
| no-entry | 793 |
| unreached | 0 |
| **sum** | **14991** (= outside total 14991) |

### checker-stopped (single definition)

**Definition:** `current_verdict=stopped` — harness never reached the typed-frontier checker; agreement is always `n-a` (never in agree/disagree).

| metric | count |
| --- | ---: |
| checker-stopped fixtures (corpus-wide) | 1061 |
| … `program_verdict=outside` | 1061 |
| … `program_verdict=accepted` | 0 |
| … `program_verdict=rejected` | 0 |
| … `program_verdict=partial` | 0 |
| subset: sole root `compilation:error` (shard 1: **128**, shard 4: **146**) | 274 |
| agreement on stopped rows | {'n-a': 1061} |

By shard (`current_verdict=stopped`):

| shard | count |
| ---: | ---: |
| 0 | 119 |
| 1 | 128 |
| 2 | 131 |
| 3 | 132 |
| 4 | 146 |
| 5 | 140 |
| 6 | 140 |
| 7 | 125 |

### PARTIAL breakdown (construct vs non-construct primary)

**`bucket:partial-unjudged-defect`:** fail-tier partial programs where at least one machine root is route-accepted while a sibling machine or compilation root stays unjudged (`no-entry` / `outside`), so the fixture is not a wholesale program accept.

| kind | primary reason | count |
| --- | --- | ---: |
| construct | parameter mutable parameter | 265 |
| non-construct | no-program-entry | 210 |
| construct | machine generic parameters | 186 |
| non-construct | bucket:partial-unjudged-defect | 123 |
| construct | type reference | 121 |
| construct | machine attached data | 77 |
| construct | type constrained | 68 |
| construct | type data with custody | 55 |
| construct | contract contracts on a state | 42 |
| construct | machine type equations, owned data or conformances | 38 |
| construct | type float | 32 |
| construct | machine service reach | 29 |
| non-construct | no-entry | 29 |
| construct | contract term other than a formal or literal | 20 |
| construct | type fixed array | 17 |
| construct | machine target selection | 17 |
| construct | contract clause that is not a comparison or conjunction | 16 |
| construct | expression match on a scalar subject | 14 |
| construct | machine operator spelling | 12 |
| construct | contract domain membership | 12 |
| non-construct | first_refusal:outside | 12 |
| construct | type generic data | 11 |
| construct | machine supply mode other than a checked body | 9 |
| construct | machine assembly target | 8 |
| construct | expression match arm on a value pattern | 8 |
| construct | contract disjunction | 8 |
| construct | call receiver, requirement or coercion call | 8 |
| non-construct | first_refusal:(empty) | 8 |
| construct | type generic application | 7 |
| construct | type addr | 7 |
| construct | contract ranking witness | 6 |
| construct | partial-mixed | 5 |
| construct | call compiler operation call | 4 |
| construct | contract result binding | 4 |
| construct | call late-bound selection | 4 |
| construct | statement assembly | 4 |
| construct | type nominal type | 3 |
| construct | contract named proposition | 3 |
| construct | contract ensures on a call cycle | 2 |
| construct | expression cast into a non-exact arithmetic domain | 2 |
| construct | contract clause that projects a formal or result | 2 |
| construct | expression type parameter | 2 |
| construct | expression anonymous value with a fractional intermediate | 1 |
| construct | completion statement after an unconditional arm | 1 |
| construct | typing literal landed type | 1 |
| construct | typing arm value type | 1 |
| construct | call requirement call with a static application feature | 1 |
| construct | expression float literal | 1 |
| construct | call receiver call | 1 |
| construct | machine suspends or blocks | 1 |
| construct | statement expression statement that discards its value | 1 |
| construct | statement store into an immutable binding | 1 |
| construct | incomplete | 1 |
| construct | call mathematical definition | 1 |
| construct | call builtin function | 1 |

Partial subtotals: non-construct **382**, construct **1141**, sum **1523**.

## Top first-refusal constructs — OUTSIDE programs

(Ranked over **12259** `construct-blocked` OUTSIDE rows only; non-construct buckets excluded.)

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
| type float | 261 | `tests/omega/fail/domains/terminal_result_domain_incompatible_carrier/main.omg`, `tests/omega/fail/facts/core_float_representation_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_ieee_view_through_field_rejected/main.omg` |
| declaration const | 221 | `tests/omega/fail/facts/computed_constant_array_extent_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/foreign_scalar_backing_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/narrow_to_wide_signed_8to64_extension_through_const_name_rejected/main.omg` |

## Top first-refusal constructs — PARTIAL programs

(Only PARTIAL rows whose primary reason is **construct**; see breakdown above.)

| construct | programs blocked | example fixtures |
| --- | ---: | --- |
| parameter mutable parameter | 265 | `tests/omega/fail/arithmetic/indexed_divisor_storage_write/main.omg`, `tests/omega/fail/asm/proposition_write_invalidates_ensures/main.omg`, `tests/omega/fail/borrow/write_only_exclusive_call_operand_incoming_child/main.omg` |
| machine generic parameters | 186 | `tests/omega/fail/arithmetic/const_binder_unbounded_product/main.omg`, `tests/omega/fail/borrow/captured_selector_result_retires_written_bound/main.omg`, `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg` |
| type reference | 121 | `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg`, `tests/omega/fail/borrows/returned_indexed_case_out_of_bounds/main.omg`, `tests/omega/fail/cases/borrowed_case_component_cannot_recover_common_fields/main.omg` |
| machine attached data | 77 | `tests/omega/fail/arithmetic/guarded_read_count_unguarded_arrival/main.omg`, `tests/omega/fail/arithmetic/lifetime_provider_slice_count_cast_requires_fit/main.omg`, `tests/omega/fail/arithmetic/wrapping_guard_does_not_bound_original/main.omg` |
| type constrained | 68 | `tests/omega/fail/core/content_membership_return_unsupported_arm/main.omg`, `tests/omega/fail/domains/issued_record_unknown_branch/main.omg`, `tests/omega/fail/domains/scalar_domain_unrelated_call_result/main.omg` |
| type data with custody | 55 | `tests/omega/fail/borrows/tail_recursive_mutation_retires_field_bound/main.omg`, `tests/omega/fail/contracts/aggregate_result_false_guarantee/main.omg`, `tests/omega/fail/layouts/encoded_view_opaque_write_requires_fit/main.omg` |
| contract contracts on a state | 42 | `tests/omega/fail/core/content_membership_return_direct_adapter/main.omg`, `tests/omega/fail/facts/applied_requirement_indices_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/linear_owner_borrowed_projection_call_result/main.omg` |
| machine type equations, owned data or conformances | 38 | `tests/omega/fail/asm/atomic_release_disposition_impostor/main.omg`, `tests/omega/fail/facts/nominal_callback_reach_through_other_state_rejected/main.omg`, `tests/omega/fail/ownership/borrowed_content_partition_wrong_conservation/main.omg` |
| type float | 32 | `tests/omega/fail/facts/float_comparison_route_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/float_range_delivery_through_other_state_rejected/main.omg`, `tests/omega/fail/proofs/ordinary_denotational_operator_requires_normal_return/main.omg` |
| machine service reach | 29 | `tests/omega/fail/contracts/opaque_call_retires_reference_contents/main.omg`, `tests/omega/fail/facts/assumed_binding_result_bound_through_other_state_rejected/main.omg`, `tests/omega/fail/host/conditional_process_exit_keeps_return_obligations/main.omg` |
| contract term other than a formal or literal | 20 | `tests/omega/fail/proofs/addition_commutativity_false_twin/main.omg`, `tests/omega/fail/proofs/congruence_false_twin/main.omg`, `tests/omega/fail/proofs/proof_integer_unformed_quotient/main.omg` |
| type fixed array | 17 | `tests/omega/fail/build/calling_policy_unproved_helper/main.omg`, `tests/omega/fail/facts/exclusive_observation_exposure_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/selected_scalar_initializer_through_other_state_rejected/main.omg` |
| machine target selection | 17 | `tests/omega/fail/facts/matched_field_windows_through_target_body_rejected/main.omg`, `tests/omega/fail/calls/boundary_computed_selector_out_of_bounds/main.omg`, `tests/omega/fail/facts/float_comparison_route_through_target_body_rejected/main.omg` |
| contract clause that is not a comparison or conjunction | 16 | `tests/omega/fail/core/terminal_call_guard_evaluation_not_selected/main.omg`, `tests/omega/fail/facts/abstract_signature_call_through_const_name_rejected/main.omg`, `tests/omega/fail/proofs/mathematical_opaque_operation_source_meaning/main.omg` |
| expression match on a scalar subject | 14 | `tests/omega/fail/facts/call_result_bound_discharge_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/match_primitive_local_origin_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_match_join_rejected/main.omg` |

## Disagreements (judged rows only)

Total disagree rows: **153**.

| program | current | count |
| --- | --- | ---: |
| rejected | accepted | 149 |
| accepted | rejected | 4 |

### Soundness watch — non-vacuous (route accept, current reject)

(none)

### Vacuous-accept route✓ / current✗ (not soundness watch)

- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` shard=5 vacuous=`compilation`
- `tests/omega/fail/build/program_entry_returns_value/main.omg` shard=2 vacuous=`compilation`
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` shard=2 vacuous=`compilation`
- `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` shard=4 vacuous=`compilation`

### All disagreement fixtures

- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` tier=fail program=accepted current=rejected **[vacuous; not soundness watch]**
- `tests/omega/fail/build/program_entry_returns_value/main.omg` tier=fail program=accepted current=rejected **[vacuous; not soundness watch]**
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` tier=fail program=accepted current=rejected **[vacuous; not soundness watch]**
- `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` tier=fail program=accepted current=rejected **[vacuous; not soundness watch]**
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

## Vacuous accept roots (corpus-wide)

Fixtures with vacuous accept annotation: **1527**.

## Timeouts and near-timeouts

### Timeouts (300s cap)

(none)

### Near-timeout (≥ 250s)

- `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg` 297.260s program=outside shard=7
- `tests/omega/fail/build/installed_arrival_without_context/main.omg` 289.513s program=outside shard=6
- `tests/omega/pass/filesystem/runtime_portable_unknown_directory_kinds_exit/main.omg` 263.605s program=outside shard=3

## Per-shard reconciliation

| shard | branch head | rows | accepted | rejected | partial | outside | timeout | recomputed≠shard | mismatch examples |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0 | `24d10db2781074710b41b1c5495b24eab944c937` | 2174 | 37 | 63 | 195 | 1879 | 0 | 0 | — |
| 1 | `e43bf91e6d428c04240f16a0859a45dbc046de67` | 2174 | 38 | 65 | 208 | 1863 | 0 | 128 | `tests/omega/fail/arithmetic/unqualified_case_value_rejected/main.omg` error→outside; `tests/omega/fail/asm/arm_subs_uninitialized_left/main.omg` error→outside; `tests/omega/fail/asm/checked_fence_arm_invented_clobber/main.omg` error→outside; …+125 |
| 2 | `0b891ed4e20364190d0ce74fd88028418da37d7a` | 2174 | 37 | 81 | 192 | 1864 | 0 | 0 | — |
| 3 | `ee199a4927bac3a99dcae083e729e2a13790b503` | 2174 | 34 | 78 | 189 | 1873 | 0 | 0 | — |
| 4 | `7380bbe60eb94873dafbd2589b1612b62b92e54c` | 2174 | 44 | 72 | 123 | 1935 | 0 | 490 | `tests/omega/fail/arithmetic/anonymous_comparison_imported_zero_divisor/main.omg` error→outside; `tests/omega/fail/asm/arm_subs_invented_clobber/main.omg` error→outside; `tests/omega/fail/asm/atomic_completion_wrong_invocation/main.omg` no-entry→outside; …+487 |
| 5 | `a9ea414124571aa03f21473ddc5f6287bccf9050` | 2174 | 41 | 73 | 232 | 1828 | 0 | 0 | — |
| 6 | `6d5700730d329c42f3ae4ce936cb284f2c499003` | 2174 | 34 | 70 | 188 | 1882 | 0 | 0 | — |
| 7 | `fb6c06dfaf9c9e00b14b2f82ad061079c2180809` | 2173 | 36 | 74 | 196 | 1867 | 0 | 0 | — |

## Harness and hosts

- PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
- Hosts: Linux `cursor` cloud agents (x86_64), serial one-fixture-per-process.

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison < /dev/null
timeout 300 target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

# Route coverage shard 5 of 8

- Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`
- Host: Linux x86_64 cloud agent
- Harness: `target/debug/examples/route_one_comparison`
- Updated: 2026-11-12T13:24Z

## Execution model

One `route_one_comparison` process per fixture, serial on this VM (per `tools/build_slot.md`: one heavy corpus-style job at a time on shared test hosts). `timeout 300` per fixture; wall seconds recorded in the TSV.

### Non-construct `first_refusal` buckets

| Bucket | Meaning |
| --- | --- |
| `no_program_entry` | Fixture binds no program entry; compilation root is `route=no-entry` and was not elaborated as a module. |
| `current_stopped_before_typed_trees` | Current checker `current=stopped`; route `unreached` (no typed trees). |

Construct refusals use the payload after the harness refusal label (e.g. `Unsupported: parameter mutable parameter` → `parameter mutable parameter`, matching `structure_elaboration::refuse` detail text).

### Agreement definition

Compare `route_verdict` (`accepted`/`rejected` only) with `current_verdict` from the same harness `program` line (`current=accepted|rejected|stopped`). `outside`, `timeout`, and `error` → `n-a`. For `fail/` with `--every-machine`, fixture `route_verdict` requires every machine root to reach `accepted` before the row is `accepted`; mixed `accepted` + `outside` machine roots stay `outside`.

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 5
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 5 --repair  # re-parse fixes
```

Per fixture (fail tier uses `--every-machine`):

```bash
timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null
timeout 300 target/debug/examples/route_one_comparison --every-machine <fixture>/main.omg < /dev/null
```

## Totals

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 2174 |
| Route accepted | 52 |
| Route rejected | 73 |
| Route outside | 2049 |
| Route timeout | 0 |
| Route error | 0 |
| Agreement agree | 96 |
| Agreement disagree | 29 |
| Agreement n/a | 2049 |

## Top first-refusal constructs (outside)

| Construct | Fixtures | Examples |
| --- | ---: | --- |
| machine service reach | 349 | `tests/omega/fail/asm/instruction_disposition_conditional_call/main.omg`, `tests/omega/fail/asm/instruction_disposition_wrong_resource/main.omg`, `tests/omega/fail/borrows/direct_reference_array_duplicate_mutable/main.omg` |
| parameter mutable parameter | 286 | `tests/omega/fail/arithmetic/distinct_runtime_call_results_do_not_cancel/main.omg`, `tests/omega/fail/arithmetic/indexed_divisor_copied_selector_storage_write/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_stale_capture/main.omg` |
| machine generic parameters | 221 | `tests/omega/fail/borrow/captured_window_wrong_slot_keeps_loan/main.omg`, `tests/omega/fail/borrow/recast_selector_retires_entry_bound/main.omg`, `tests/omega/fail/borrow/scalar_reference_result_does_not_preserve_mutated_bound/main.omg` |
| type reference | 151 | `tests/omega/fail/arithmetic/collection_result_conditional_bound/main.omg`, `tests/omega/fail/arithmetic/zero_increment_unproved_index/main.omg`, `tests/omega/fail/atomics/atomic_boolean_loaded_cell_reference/main.omg` |
| no_program_entry | 141 | `tests/omega/fail/generics/dependent_const_array_named_carrier/main.omg`, `tests/omega/fail/proofs/declaration_predicate_ambiguous_embedding/main.omg`, `tests/omega/pass/arithmetic/addr_algebra/main.omg` |
| current_stopped_before_typed_trees | 140 | `tests/omega/fail/arithmetic/policy_operator_contract_trapping_term/main.omg`, `tests/omega/fail/asm/arm_subs_literal_left/main.omg`, `tests/omega/fail/asm/checked_asm_compare_arm_unencodable_immediate/main.omg` |
| declaration Build selection that is not a machine or a data type | 122 | `tests/omega/fail/boundary/requirement_float_precondition_unproved/main.omg`, `tests/omega/fail/build/interrupt_return_masked_enable/main.omg`, `tests/omega/fail/build/task_register_wrong_tss_extent/main.omg` |
| type data with custody | 108 | `tests/omega/fail/borrows/constructor_case_owner_mismatch/main.omg`, `tests/omega/fail/borrows/replaced_carrier_keeps_new_backing_loan/main.omg`, `tests/omega/fail/calls/completed_narrowing_result_case_bounds/main.omg` |
| machine attached data | 106 | `tests/omega/fail/arithmetic/bounded_assignment_unproven/main.omg`, `tests/omega/fail/arithmetic/fixed_array_larger_length_does_not_bound_index/main.omg`, `tests/omega/fail/arithmetic/implicit_call_operand_conversion_rejected/main.omg` |
| type constrained | 62 | `tests/omega/fail/arithmetic/captured_boolean_wrong_polarity/main.omg`, `tests/omega/fail/arithmetic/total_contract_wrapping_division_unguarded/main.omg`, `tests/omega/fail/borrows/dropped_reference_qualification_join/main.omg` |
| type float | 41 | `tests/omega/fail/arithmetic/anonymous_decimal_zero_division_rejected/main.omg`, `tests/omega/fail/calls/nested_call_argument_constraints_unproven/main.omg`, `tests/omega/fail/facts/core_float_representation_through_field_rejected/main.omg` |
| machine type equations, owned data or conformances | 36 | `tests/omega/fail/asm/atomic_completion_wrong_receiver/main.omg`, `tests/omega/fail/asm/atomic_release_disposition_wrong_cell/main.omg`, `tests/omega/fail/core/content_issuance_consumed_constructed_claims/main.omg` |
| contract contracts on a state | 29 | `tests/omega/fail/borrows/qualified_state_loan_shared_to_mutable/main.omg`, `tests/omega/fail/build/uefi_entry_counterfeit_roots/main.omg`, `tests/omega/fail/core/content_membership_return_missing_guarantee/main.omg` |
| declaration const | 28 | `tests/omega/fail/facts/computed_constant_array_extent_through_local_rejected/main.omg`, `tests/omega/fail/facts/core_integer_endpoint_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/fixed_array_layout_geometry_through_const_name_rejected/main.omg` |
| type fixed array | 26 | `tests/omega/fail/borrows/record_exclusive_view_backing_write/main.omg`, `tests/omega/fail/build/calling_policy_checked_helper_rejection/main.omg`, `tests/omega/fail/calls/foreign_scalar_indirect_result_wrong_width_rejected/main.omg` |
| expression match on a scalar subject | 23 | `tests/omega/fail/borrows/completed_selector_join_out_of_range_destination/main.omg`, `tests/omega/fail/facts/abi_multi_arg_spill_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/match_join_multi_variant_bound_through_match_join_rejected/main.omg` |
| type generic data | 17 | `tests/omega/fail/collections/stored_reference_array_owned_record_empty/main.omg`, `tests/omega/fail/facts/representation_agreement_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/runtime_index_application_through_parameter_rejected/main.omg` |
| contract term other than a formal or literal | 16 | `tests/omega/fail/arithmetic/exact_left_shift_false_ceiling_rejected/main.omg`, `tests/omega/fail/asm/checked_asm_nominal_wrong_subject/main.omg`, `tests/omega/fail/contracts/match_contract_partial_operand_rejected/main.omg` |
| machine target selection | 14 | `tests/omega/fail/domains/target_issuer_nonrealized_predicate/main.omg`, `tests/omega/fail/facts/correlated_invariant_store_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/dependent_subtract_through_target_body_rejected/main.omg` |
| machine assembly target | 13 | `tests/omega/fail/asm/instruction_effect_stale_operand/main.omg`, `tests/omega/fail/asm/scratch_snapshot_after_input_write_wrong_value_rejected/main.omg`, `tests/omega/fail/facts/checked_asm_add_carry_through_const_name_rejected/main.omg` |

## Disagreements (route accepts, current rejects) — soundness candidates

- `tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg` (1.3s)
- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` (2.8s)
- `tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg` (1.4s)
- `tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg` (1.3s)
- `tests/omega/fail/dependent/data_requires_self_formation/main.omg` (1.3s)
- `tests/omega/fail/domains/domain_alias_cycle/main.omg` (1.3s)
- `tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg` (1.3s)
- `tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg` (1.3s)
- `tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg` (1.3s)
- `tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg` (1.3s)

## All disagreements

- `tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg` route=accepted current=rejected
- `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` route=accepted current=rejected
- `tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg` route=accepted current=rejected
- `tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg` route=accepted current=rejected
- `tests/omega/fail/dependent/data_requires_self_formation/main.omg` route=accepted current=rejected
- `tests/omega/fail/domains/domain_alias_cycle/main.omg` route=accepted current=rejected
- `tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg` route=accepted current=rejected
- `tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg` route=accepted current=rejected
- `tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg` route=accepted current=rejected
- `tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg` route=accepted current=rejected
- `tests/omega/pass/arithmetic/runtime_boolean_guarded_product_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_other_machine_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/scalar_sign_zero_extension_through_other_state_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_call_result_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_parameter_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_other_state_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_local_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/signed_shift_left_bound_through_local_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_call_result_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_call_result_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_parameter_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_state_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_call_result_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_division_remainder_through_parameter_exit/main.omg` route=rejected current=accepted
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_local_exit/main.omg` route=rejected current=accepted

## Timeouts

_None.

## Hand-check sample (5 disagreements)

### `tests/omega/pass/arithmetic/runtime_boolean_guarded_product_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/arithmetic/runtime_boolean_guarded_product_exit/main.omg	current=accepted	current_us=3173764	root=compilation	route=rejected	route_us=2752	discharge_us=1939	reverify_us=551	reverified=6	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg	current=accepted	current_us=3086341	root=compilation	route=rejected	route_us=17725	discharge_us=17240	reverify_us=15705	reverified=19	obligations=23	judged=19	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg	current=accepted	current_us=3918633	root=compilation	route=rejected	route_us=10513	discharge_us=9776	reverify_us=8333	reverified=9	obligations=11	judged=9	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=6	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg	current=accepted	current_us=3118241	root=compilation	route=rejected	route_us=67526	discharge_us=66622	reverify_us=66025	reverified=16	obligations=20	judged=16	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg	current=accepted	current_us=3218020	root=compilation	route=rejected	route_us=49453	discharge_us=48512	reverify_us=47095	reverified=12	obligations=16	judged=12	core_refused=0	unproved=4	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

## Hand-check sample (5 route-accepts / current-rejects)

### `tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg`
- TSV: route=accepted current=rejected first_refusal=no_program_entry
- `program	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	current=rejected	current_us=1298117	root=compilation	route=no-entry`
- `program	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	current=rejected	current_us=1298117	root=machine:main	route=accepted	route_us=204	discharge_us=56	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	no-span	data `Bad` field `body` uses undeclared lifetime `'ghost'; declare it in the owner's generic parameter list`

### `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg`
- TSV: route=accepted current=rejected first_refusal=
- `program	tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg	current=rejected	current_us=2813319	root=compilation	route=accepted	route_us=261	discharge_us=59	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg	no-span	hosted `ProgramEntry` exposes no arrival parameters, but `launch` declares `argument``

### `tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg`
- TSV: route=accepted current=rejected first_refusal=no_program_entry
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1371116	root=compilation	route=no-entry`
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1371116	root=machine:size	route=accepted	route_us=219	discharge_us=62	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1371116	root=machine:main	route=accepted	route_us=20	discharge_us=1	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	main.omg:3 bytes 143..148	data `Bad` field `bytes` constant expression call argument `false` does not form parameter `value` of `size`: actual bool, required u64`

### `tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg`
- TSV: route=accepted current=rejected first_refusal=no_program_entry
- `program	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	current=rejected	current_us=1330706	root=compilation	route=no-entry`
- `program	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	current=rejected	current_us=1330706	root=machine:main	route=accepted	route_us=216	discharge_us=60	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	no-span	data `RoomEvent` case `Treasure` payload field `gold` collides with the common field of the same name`

### `tests/omega/fail/dependent/data_requires_self_formation/main.omg`
- TSV: route=accepted current=rejected first_refusal=no_program_entry
- `program	tests/omega/fail/dependent/data_requires_self_formation/main.omg	current=rejected	current_us=1324283	root=compilation	route=no-entry`
- `program	tests/omega/fail/dependent/data_requires_self_formation/main.omg	current=rejected	current_us=1324283	root=machine:guarded	route=accepted	route_us=291	discharge_us=82	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/dependent/data_requires_self_formation/main.omg	no-span	cannot prove requires contract for specification call `guarded` in data `Invalid` default-domain predicate: establish its selected precondition in an independently formed prior requires fact`


## installed_arrival_* fixtures in this shard

_None in shard 5._

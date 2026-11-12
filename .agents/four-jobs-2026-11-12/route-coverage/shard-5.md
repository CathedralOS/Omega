# Route coverage shard 5 of 8

- Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`
- Host: Linux x86_64 cloud agent
- Harness: `target/debug/examples/route_one_comparison`
- Updated: 2026-11-12T16:00Z

## Execution model

One `route_one_comparison` process per fixture, serial on this VM (per `tools/build_slot.md`: one heavy corpus-style job at a time on shared test hosts). `timeout 300` per fixture; wall seconds recorded in the TSV.

### Non-construct `first_refusal` buckets

| Bucket | Meaning |
| --- | --- |
| `no_program_entry` | Fixture binds no program entry; compilation root is `route=no-entry` and was not elaborated as a module. |
| `current_stopped_before_typed_trees` | Current checker `current=stopped`; route `unreached` (no typed trees). |

Construct refusals use the payload after the harness refusal label (e.g. `Unsupported: parameter mutable parameter` → `parameter mutable parameter`, matching `structure_elaboration::refuse` detail text).

### Agreement definition

Compare `route_verdict` (`accepted`/`rejected` only) with `current_verdict` from the same harness `program` line (`current=accepted|rejected|stopped`). `outside`, `partial`, `timeout`, and `error` → `n-a` (excluded from the agreement denominator). All tiers use `--every-machine`. Fixture `route_verdict=accepted` only when every evaluated root is `accepted`; mixed `accepted` + unjudged roots → `partial` with `first_refusal` from the blocking construct.

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 5
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 5 --repair  # re-parse fixes
```

Per fixture (all tiers):

```bash
timeout 300 target/debug/examples/route_one_comparison --every-machine <fixture>/main.omg < /dev/null
```

## Totals

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 2174 |
| Route judged (accepted + rejected) | 135 |
| Route accepted | 62 |
| Route rejected | 73 |
| Route partial | 211 |
| Route outside | 1828 |
| Route timeout | 0 |
| Route error | 0 |
| Agreement agree | 106 |
| Agreement disagree | 29 |
| Agreement n/a | 2039 |

## Top first-refusal constructs (outside)

| Construct | Fixtures | Examples |
| --- | ---: | --- |
| machine service reach | 349 | `tests/omega/fail/asm/instruction_disposition_wrong_resource/main.omg`, `tests/omega/fail/borrows/direct_reference_array_duplicate_mutable/main.omg`, `tests/omega/fail/borrows/stored_reference_constructor_transfer_live_source_write/main.omg` |
| parameter mutable parameter | 244 | `tests/omega/fail/arithmetic/distinct_runtime_call_results_do_not_cancel/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_stale_capture/main.omg`, `tests/omega/fail/borrow/returned_write_destination_backing_conflict/main.omg` |
| machine generic parameters | 195 | `tests/omega/fail/borrow/captured_window_wrong_slot_keeps_loan/main.omg`, `tests/omega/fail/borrow/scalar_reference_result_does_not_preserve_mutated_bound/main.omg`, `tests/omega/fail/borrows/assignment_rhs_carrier_preserves_sibling_backing/main.omg` |
| current_stopped_before_typed_trees | 140 | `tests/omega/fail/arithmetic/policy_operator_contract_trapping_term/main.omg`, `tests/omega/fail/asm/arm_subs_literal_left/main.omg`, `tests/omega/fail/asm/checked_asm_compare_arm_unencodable_immediate/main.omg` |
| type reference | 139 | `tests/omega/fail/arithmetic/collection_result_conditional_bound/main.omg`, `tests/omega/fail/arithmetic/zero_increment_unproved_index/main.omg`, `tests/omega/fail/atomics/atomic_boolean_loaded_cell_reference/main.omg` |
| machine attached data | 138 | `tests/omega/fail/arithmetic/bounded_assignment_unproven/main.omg`, `tests/omega/fail/arithmetic/fixed_array_larger_length_does_not_bound_index/main.omg`, `tests/omega/fail/arithmetic/implicit_call_operand_conversion_rejected/main.omg` |
| declaration Build selection that is not a machine or a data type | 122 | `tests/omega/fail/boundary/requirement_float_precondition_unproved/main.omg`, `tests/omega/fail/build/interrupt_return_masked_enable/main.omg`, `tests/omega/fail/build/task_register_wrong_tss_extent/main.omg` |
| type data with custody | 112 | `tests/omega/fail/borrows/constructor_case_owner_mismatch/main.omg`, `tests/omega/fail/borrows/replaced_carrier_keeps_new_backing_loan/main.omg`, `tests/omega/fail/calls/completed_narrowing_result_case_bounds/main.omg` |
| type constrained | 54 | `tests/omega/fail/arithmetic/captured_boolean_wrong_polarity/main.omg`, `tests/omega/fail/arithmetic/total_contract_wrapping_division_unguarded/main.omg`, `tests/omega/fail/borrows/routed_membership_reference_contract_replacement/main.omg` |
| type float | 39 | `tests/omega/fail/arithmetic/anonymous_decimal_zero_division_rejected/main.omg`, `tests/omega/fail/calls/nested_call_argument_constraints_unproven/main.omg`, `tests/omega/fail/facts/core_float_representation_through_field_rejected/main.omg` |
| machine type equations, owned data or conformances | 29 | `tests/omega/fail/core/content_transform_nested_linear_loss/main.omg`, `tests/omega/fail/core/independent_content_indexed_input_unconserved/main.omg`, `tests/omega/fail/facts/case_where_equality_data_predicate_through_target_body_rejected/main.omg` |
| declaration const | 28 | `tests/omega/fail/facts/computed_constant_array_extent_through_local_rejected/main.omg`, `tests/omega/fail/facts/core_integer_endpoint_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/fixed_array_layout_geometry_through_const_name_rejected/main.omg` |
| contract contracts on a state | 25 | `tests/omega/fail/borrows/qualified_state_loan_shared_to_mutable/main.omg`, `tests/omega/fail/build/uefi_entry_counterfeit_roots/main.omg`, `tests/omega/fail/core/image_address_observation_does_not_grant/main.omg` |
| type fixed array | 25 | `tests/omega/fail/calls/foreign_scalar_indirect_result_wrong_width_rejected/main.omg`, `tests/omega/fail/collections/nested_read_index_uses_selected_extent/main.omg`, `tests/omega/fail/facts/fixed_array_onethousandtwentyfour_elem_index_bound_through_call_result_rejected/main.omg` |
| expression match on a scalar subject | 20 | `tests/omega/fail/borrows/completed_selector_join_out_of_range_destination/main.omg`, `tests/omega/fail/facts/abi_multi_arg_spill_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/nine_arg_stack_calling_plan_through_match_join_rejected/main.omg` |

## Top first-refusal constructs (partial)

| Construct | Fixtures | Examples |
| --- | ---: | --- |
| parameter mutable parameter | 51 | `tests/omega/fail/arithmetic/indexed_divisor_copied_selector_storage_write/main.omg`, `tests/omega/fail/asm/proposition_prior_write_invalidates_requires/main.omg`, `tests/omega/fail/borrow/borrow_carrying_field_reassignment_invalidated/main.omg` |
| machine generic parameters | 29 | `tests/omega/fail/borrow/recast_selector_retires_entry_bound/main.omg`, `tests/omega/fail/borrows/incoming_carrier_duplicate_exclusive_leaf/main.omg`, `tests/omega/fail/borrows/reference_join_escaping_alternative/main.omg` |
| machine attached data | 21 | `tests/omega/fail/calls/element_view_argument_type_rejected/main.omg`, `tests/omega/fail/dependent/case_where_payload_write_exit_open/main.omg`, `tests/omega/fail/facts/call_result_bound_discharge_through_const_name_rejected/main.omg` |
| type reference | 20 | `tests/omega/fail/borrows/captured_reference_membership_ignores_later_selector/main.omg`, `tests/omega/fail/borrows/reference_array_field_requires_declared_predicate/main.omg`, `tests/omega/fail/collections/element_view_read_requires_live_extent/main.omg` |
| type data with custody | 11 | `tests/omega/fail/contracts/address_field_false_result_guarantee/main.omg`, `tests/omega/fail/contracts/cyclic_mixed_disjunction/main.omg`, `tests/omega/fail/facts/integer_width_truncation_mask_bound_through_field_rejected/main.omg` |
| type constrained | 10 | `tests/omega/fail/borrows/dropped_reference_qualification_join/main.omg`, `tests/omega/fail/domains/carried_runtime_index_payload_qualification/main.omg`, `tests/omega/fail/domains/routed_record_capture_after_sibling_mutation/main.omg` |
| machine type equations, owned data or conformances | 7 | `tests/omega/fail/asm/atomic_completion_wrong_receiver/main.omg`, `tests/omega/fail/asm/atomic_release_disposition_wrong_cell/main.omg`, `tests/omega/fail/core/content_issuance_consumed_constructed_claims/main.omg` |
| contract contracts on a state | 7 | `tests/omega/fail/core/content_membership_return_missing_guarantee/main.omg`, `tests/omega/fail/facts/applied_requirement_indices_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/linear_owner_borrowed_projection_other_machine/main.omg` |
| type fixed array | 5 | `tests/omega/fail/borrows/record_exclusive_view_backing_write/main.omg`, `tests/omega/fail/build/calling_policy_checked_helper_rejection/main.omg`, `tests/omega/fail/facts/selected_scalar_initializer_through_local_rejected/main.omg` |
| type float | 5 | `tests/omega/fail/facts/float_comparison_route_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/float_range_delivery_through_local_rejected/main.omg`, `tests/omega/fail/float/false_ieee_ordered_guards_admit_nan/main.omg` |
| machine target selection | 4 | `tests/omega/fail/facts/correlated_invariant_store_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/domain_predicate_call_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/length_through_target_body_rejected/main.omg` |
| expression match on a scalar subject | 4 | `tests/omega/fail/facts/match_join_multi_variant_bound_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/match_primitive_local_origin_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/projection_receiver_call_through_match_join_rejected/main.omg` |
| contract term other than a formal or literal | 4 | `tests/omega/fail/proofs/numeric_state_rounding_boundary/main.omg`, `tests/omega/fail/proofs/proof_integer_self_justified_divisor/main.omg`, `tests/omega/fail/termination/computed_result_rank_missing_precondition/main.omg` |
| machine service reach | 3 | `tests/omega/fail/asm/instruction_disposition_conditional_call/main.omg`, `tests/omega/fail/facts/assumed_binding_result_bound_through_local_rejected/main.omg`, `tests/omega/pass/contracts/proof_fact_indexed_domain_application/main.omg` |
| statement assembly | 3 | `tests/omega/fail/asm/state_fact_skipped_store/main.omg`, `tests/omega/fail/proofs/boolean_return_uses_executed_assembly_order/main.omg`, `tests/omega/pass/proofs/boolean_return_assembly_shape/main.omg` |

## pass/run `--every-machine` impact

Compared with the prior shard TSV (fail-only `--every-machine`): **1082** of **1131** pass/run rows changed `route_verdict`.

## Route accepts / current rejects — soundness watch (all rows)

Harness `current=rejected` is always the typed-frontier checker preserved by `route_one_comparison` (not a later native/Psi run stage). `current-diagnostic` lines are that checker's diagnostics.

| Fixture | Tier | Route | First diagnostic class | Checker diagnostic? | Soundness-watch? |
| --- | --- | --- | --- | --- | --- |
| `tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg` | fail | accepted | lifetime / generic parameter | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg` | fail | accepted | program entry / build | yes (`current-diagnostic` present) | yes — route does not judge program-entry/build rules; checker rejection is typed-frontier |
| `tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg` | fail | accepted | data declaration | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg` | fail | accepted | data declaration | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/dependent/data_requires_self_formation/main.omg` | fail | accepted | domain / alias | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/domains/domain_alias_cycle/main.omg` | fail | accepted | public interface selects private data `Socket` | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg` | fail | accepted | module / declaration | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg` | fail | accepted | operator / dispatch | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg` | fail | accepted | operator / dispatch | yes (`current-diagnostic` present) | yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class |
| `tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg` | fail | accepted | provider / requirement | yes (`current-diagnostic` present) | review — confirm defect site is not covered by accepted machine root |

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
- `program	tests/omega/pass/arithmetic/runtime_boolean_guarded_product_exit/main.omg	current=accepted	current_us=3086283	root=compilation	route=rejected	route_us=2574	discharge_us=1857	reverify_us=618	reverified=6	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/arithmetic/runtime_full_width_unsigned_quotient_exit/main.omg	current=accepted	current_us=3189664	root=compilation	route=rejected	route_us=17676	discharge_us=17119	reverify_us=16376	reverified=19	obligations=23	judged=19	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/four_variant_enum_payload_bound_through_match_join_exit/main.omg	current=accepted	current_us=4057257	root=compilation	route=rejected	route_us=10278	discharge_us=9412	reverify_us=8530	reverified=9	obligations=11	judged=9	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=6	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_machine_exit/main.omg	current=accepted	current_us=3118471	root=compilation	route=rejected	route_us=69885	discharge_us=68913	reverify_us=64481	reverified=16	obligations=20	judged=16	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

### `tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg`
- TSV: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg	current=accepted	current_us=3196170	root=compilation	route=rejected	route_us=54692	discharge_us=53604	reverify_us=45917	reverified=12	obligations=16	judged=12	core_refused=0	unproved=4	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`


## Soundness watch harness traces (all rows)

### `tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg`
- Diagnostic class: lifetime / generic parameter
- First message: data `Bad` field `body` uses undeclared lifetime `'ghost'; declare it in the owner's generic parameter list
- `program	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	current=rejected	current_us=1291852	root=compilation	route=no-entry`
- `program	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	current=rejected	current_us=1291852	root=machine:main	route=accepted	route_us=195	discharge_us=58	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/borrow/undeclared_lifetime_tag/main.omg	no-span	data `Bad` field `body` uses undeclared lifetime `'ghost'; declare it in the owner's generic parameter list`

### `tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg`
- Diagnostic class: program entry / build
- First message: hosted `ProgramEntry` exposes no arrival parameters, but `launch` declares `argument`
- `program	tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg	current=rejected	current_us=2788012	root=compilation	route=accepted	route_us=234	discharge_us=51	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/build/hosted_program_entry_visible_parameter/main.omg	no-span	hosted `ProgramEntry` exposes no arrival parameters, but `launch` declares `argument``

### `tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg`
- Diagnostic class: data declaration
- First message: data `Bad` field `bytes` constant expression call argument `false` does not form parameter `value` of `size`: actual bool, required u64
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1340142	root=compilation	route=no-entry`
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1340142	root=machine:size	route=accepted	route_us=210	discharge_us=53	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	current=rejected	current_us=1340142	root=machine:main	route=accepted	route_us=21	discharge_us=1	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/comptime/array_expression_unused_invalid_call/main.omg	main.omg:3 bytes 143..148	data `Bad` field `bytes` constant expression call argument `false` does not form parameter `value` of `size`: actual bool, required u64`

### `tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg`
- Diagnostic class: data declaration
- First message: data `RoomEvent` case `Treasure` payload field `gold` collides with the common field of the same name
- `program	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	current=rejected	current_us=1282640	root=compilation	route=no-entry`
- `program	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	current=rejected	current_us=1282640	root=machine:main	route=accepted	route_us=196	discharge_us=53	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/data/mixed_payload_field_shadows_common/main.omg	no-span	data `RoomEvent` case `Treasure` payload field `gold` collides with the common field of the same name`

### `tests/omega/fail/dependent/data_requires_self_formation/main.omg`
- Diagnostic class: domain / alias
- First message: cannot prove requires contract for specification call `guarded` in data `Invalid` default-domain predicate: establish its selected precondition in an independently formed prior requires fact
- `program	tests/omega/fail/dependent/data_requires_self_formation/main.omg	current=rejected	current_us=1251991	root=compilation	route=no-entry`
- `program	tests/omega/fail/dependent/data_requires_self_formation/main.omg	current=rejected	current_us=1251991	root=machine:guarded	route=accepted	route_us=256	discharge_us=71	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/dependent/data_requires_self_formation/main.omg	no-span	cannot prove requires contract for specification call `guarded` in data `Invalid` default-domain predicate: establish its selected precondition in an independently formed prior requires fact`

### `tests/omega/fail/domains/domain_alias_cycle/main.omg`
- Diagnostic class: public interface selects private data `Socket`
- First message: public interface selects private data `Socket`
- `program	tests/omega/fail/domains/domain_alias_cycle/main.omg	current=rejected	current_us=1340401	root=compilation	route=no-entry`
- `program	tests/omega/fail/domains/domain_alias_cycle/main.omg	current=rejected	current_us=1340401	root=machine:main	route=accepted	route_us=216	discharge_us=62	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/domains/domain_alias_cycle/main.omg	main.omg:3 bytes 30..36	public interface selects private data `Socket``
- `current-diagnostic	tests/omega/fail/domains/domain_alias_cycle/main.omg	main.omg:4 bytes 76..82	public interface selects private data `Socket``
- `current-diagnostic	tests/omega/fail/domains/domain_alias_cycle/main.omg	no-span	domain alias cycle: Socket::Usable -> Socket::Ready -> Socket::Usable`

### `tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg`
- Diagnostic class: module / declaration
- First message: duplicate data `Zed`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=compilation	route=no-entry`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:start	route=accepted	route_us=728	discharge_us=447	reverify_us=184	reverified=1	obligations=1	judged=0	core_refused=1	unproved=0	refused_other=0	needed_relation=0	needed_branch=1	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:narrow	route=accepted	route_us=87	discharge_us=5	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:no	route=accepted	route_us=24	discharge_us=3	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:local_facts	route=accepted	route_us=1257	discharge_us=1211	reverify_us=995	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:narrow	route=accepted	route_us=121	discharge_us=11	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `program	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	current=rejected	current_us=1447017	root=machine:main	route=accepted	route_us=28	discharge_us=2	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/modules/duplicate_data_keeps_guard_facts/main.omg	no-span	duplicate data `Zed``

### `tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg`
- Diagnostic class: operator / dispatch
- First message: duplicate named requirement overload `Convert::value` with parameter signature `operands(named(name(i32)));flags(mutable=false;const=false)` and result dispatch set `arithmetic:Trapping`; predicate-only result refinements do not distinguish overloads
- `program	tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg	current=rejected	current_us=1334656	root=compilation	route=no-entry`
- `program	tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg	current=rejected	current_us=1334656	root=machine:main	route=accepted	route_us=202	discharge_us=59	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/operators/named_operator_result_overload_duplicate_dispatch/main.omg	no-span	duplicate named requirement overload `Convert::value` with parameter signature `operands(named(name(i32)));flags(mutable=false;const=false)` and result dispatch set `arithmetic:Trapping`; predicate-only result refinements do not distinguish overloads`

### `tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg`
- Diagnostic class: operator / dispatch
- First message: duplicate operator declaration `Slice::index`
- `program	tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg	current=rejected	current_us=1280730	root=compilation	route=no-entry`
- `program	tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg	current=rejected	current_us=1280730	root=machine:main	route=accepted	route_us=228	discharge_us=58	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/operators/root_operator_alpha_equivalent_generic_duplicate/main.omg	no-span	duplicate operator declaration `Slice::index``

### `tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg`
- Diagnostic class: provider / requirement
- First message: trait `Use` state `apply` parameter `counter` uses boundary trait `Counter` as a value type; call a `Counter` requirement directly, or carry an ordinary handle, session, or `dyn` value for runtime multiplicity
- `program	tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg	current=rejected	current_us=1294692	root=compilation	route=no-entry`
- `program	tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg	current=rejected	current_us=1294692	root=machine:main	route=accepted	route_us=240	discharge_us=77	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `current-diagnostic	tests/omega/fail/providers/provider_receiver_requirement_signature_rejected/main.omg	no-span	trait `Use` state `apply` parameter `counter` uses boundary trait `Counter` as a value type; call a `Counter` requirement directly, or carry an ordinary handle, session, or `dyn` value for runtime multiplicity`


## installed_arrival_* fixtures in this shard

_None in shard 5._

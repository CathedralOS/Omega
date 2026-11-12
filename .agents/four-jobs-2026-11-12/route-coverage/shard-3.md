# Route coverage shard 3 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at fan-out).
Host: `cursor`. Generated: 2026-11-12 13:23 UTC.

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
timeout 300 target/debug/examples/route_one_comparison [--every-machine] tests/omega/<tier>/.../main.omg < /dev/null
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard_3.py
```

Harness: `target/debug/examples/route_one_comparison` from the build above.

## Execution model

One fixture per process, serial on this VM. `tools/build_slot.md` requires 
one heavy corpus-style job at a time on test VMs; this shard does not run 
fixtures in parallel.

## Totals

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 2174 |
| Route judged (accepted/rejected) | 300 |
| Route outside | 1874 |
| Route timeout | 0 |
| Route error | 0 |
| Agreement agree | 93 |
| Agreement disagree | 207 |
| Agreement n-a | 1874 |
| Soundness watch (route accepts, current rejects) | 186 |

Agreement compares `current=` from `route_one_comparison` (same check as 
`omega --check` for this harness) with the aggregated route verdict 
(accepted/rejected only; outside/timeout/error are n-a).

### Non-construct buckets

| Bucket | Count |
| --- | ---: |
| `bucket:no-program-entry` | 382 |
| `bucket:current-checker-stopped` | 132 |
| `bucket:prover-incomplete` | 2 |

- `bucket:no-program-entry`: compilation root binds no ProgramEntry.
- `bucket:current-checker-stopped`: `current=stopped`; route unreached.
- `bucket:prover-incomplete`: route=`incomplete` (no construct refusal yet).

## Top first-refusal constructs (outside only)

| Construct | Blocks | Example fixtures |
| --- | ---: | --- |
| machine service reach | 328 | `tests/omega/fail/borrows/pending_registration_write_before_completion/main.omg`, `tests/omega/fail/borrows/suspension_projected_call_view_returned_forbidden_rejected/main.omg`, `tests/omega/fail/borrows/suspension_saved_reference_view_shared_cannot_write_rejected/main.omg` |
| parameter mutable parameter | 196 | `tests/omega/fail/borrow/shared_parameter_referent_assignment/main.omg`, `tests/omega/fail/borrows/dependent_window_region_retains_backing/main.omg`, `tests/omega/fail/build/generic_zero_context_requires_establishment/main.omg` |
| machine generic parameters | 153 | `tests/omega/fail/borrow/captured_window_unknown_slot_keeps_loan/main.omg`, `tests/omega/fail/borrows/callable_multi_input_elision_needs_relation/main.omg`, `tests/omega/fail/borrows/completed_reference_result_retains_unselected_input/main.omg` |
| declaration Build selection that is not a machine or a data type | 122 | `tests/omega/fail/build/segment_selector_missing_machine_control/main.omg`, `tests/omega/fail/build/task_register_missing_machine_control/main.omg`, `tests/omega/fail/collections/vec_invalidated_loan_after_growth/main.omg` |
| type reference | 119 | `tests/omega/fail/contracts/indexed_crash_divergent_match/main.omg`, `tests/omega/fail/contracts/match_reference_private_second_arrival/main.omg`, `tests/omega/fail/dependent/collection_predicate_zero_violating_read_rejected/main.omg` |
| type data with custody | 91 | `tests/omega/fail/facts/abi_multi_arg_spill_through_field_rejected/main.omg`, `tests/omega/fail/facts/exact_integer_narrowing_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/fixed_layout_bit_observation_through_local_rejected/main.omg` |
| type constrained | 42 | `tests/omega/fail/arithmetic/checked_flag_proof_wrapping_cast_overflow/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_saturation/main.omg`, `tests/omega/fail/facts/closed_integer_bound_through_field_rejected/main.omg` |
| machine attached data | 36 | `tests/omega/fail/borrows/nested_array_call_unrestored_field/main.omg`, `tests/omega/fail/facts/guarded_call_entry_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/recast_parent_restoration_through_other_machine_rejected/main.omg` |
| declaration const | 28 | `tests/omega/fail/facts/completed_integer_carrier_bounds_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/computed_constant_array_extent_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/five_variant_enum_payload_bound_through_const_name_rejected/main.omg` |
| type fixed array | 27 | `tests/omega/fail/borrows/reference_selector_guard_rebound_index_rejected/main.omg`, `tests/omega/fail/borrows/stored_reference_write_only_array_copy/main.omg`, `tests/omega/fail/calls/foreign_scalar_indirect_result_copy_slot_rejected/main.omg` |

## Disagreements

Full list: 207 rows (186 soundness watch, 21 other judged disagreements). See `shard-3.tsv` (`agreement=disagree`).

### Soundness watch (route accepts, current rejects)
- `tests/omega/fail/arithmetic/immutable_unsigned_shift_operand_overflow_rejected/main.omg`
- `tests/omega/fail/arithmetic/numeric_operand_abs_unrepresentable/main.omg`
- `tests/omega/fail/arithmetic/unsigned_quotient_operand_landing_rejected/main.omg`
- `tests/omega/fail/asm/atomic_completion_unestablished_claim/main.omg`
- `tests/omega/fail/asm/atomic_release_disposition_relaxed/main.omg`
- `tests/omega/fail/asm/checked_asm_compare_wrong_entry_edge/main.omg`
- `tests/omega/fail/asm/instruction_effect_reverse_mutation/main.omg`
- `tests/omega/fail/asm/requires_after_prior_block_write/main.omg`
- `tests/omega/fail/atomics/atomic_boolean_expected/main.omg`
- `tests/omega/fail/atomics/atomic_store_under_exclusive_loan/main.omg`
- `tests/omega/fail/borrow/mutated_selector_entry_actual_guard/main.omg`
- `tests/omega/fail/borrow/static_callable_write_retires_subject_fact/main.omg`
- `tests/omega/fail/borrow/write_only_exclusive_replacement_incoming_child/main.omg`
- `tests/omega/fail/borrow/write_only_linear_replacement/main.omg`
- `tests/omega/fail/borrow/write_only_reference_aggregate_replacement_new_loan/main.omg`
- `tests/omega/fail/borrows/array_projection_duplicate_exclusive_transfer/main.omg`
- `tests/omega/fail/borrows/borrow_mut_and_read/main.omg`
- `tests/omega/fail/borrows/copied_computed_backing_retains_nested_loan/main.omg`
- `tests/omega/fail/borrows/dropped_reference_qualification_after_alias_write/main.omg`
- `tests/omega/fail/borrows/dyn_image_incoming_backing_consumed/main.omg`
- `tests/omega/fail/borrows/dynamic_window_result_independent_bounds/main.omg`
- `tests/omega/fail/borrows/evidence_bound_record_result_keeps_loan/main.omg`
- `tests/omega/fail/borrows/reference_aggregate_replacement_detached_loan/main.omg`
- `tests/omega/fail/borrows/reference_carrier_state_shared_write/main.omg`
- `tests/omega/fail/borrows/returned_review_keeps_original_backing_loan/main.omg`
- `tests/omega/fail/borrows/returned_view_temporary_backing/main.omg`
- `tests/omega/fail/build/generated_declaration_continuation_domain/kit/main.omg`
- `tests/omega/fail/calls/constructor_child_distinct_epochs_not_equal/main.omg`
- `tests/omega/fail/calls/copied_argument_ensures_cannot_restore_callee_mutated_source/main.omg`
- `tests/omega/fail/calls/duplicate_named_machine_overload_rejected/main.omg`
- … and 156 more

### Other disagreements (route rejects, current accepts)

- `tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg`
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_field_exit/main.omg`
- `tests/omega/pass/facts/live_across_call_spill_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_call_result_exit/main.omg`
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_parameter_exit/main.omg`
- `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_call_result_exit/main.omg`
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_local_exit/main.omg`
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_local_exit/main.omg`
- `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_parameter_exit/main.omg`
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_other_machine_exit/main.omg`
- `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_call_result_exit/main.omg`
- `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_parameter_exit/main.omg`
- `tests/omega/pass/facts/signed_8bit_exact_arithmetic_bound_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/signed_arithmetic_shift_right_through_parameter_exit/main.omg`
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_other_machine_exit/main.omg`
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_other_state_exit/main.omg`
- `tests/omega/pass/facts/unsigned_division_remainder_through_other_machine_exit/main.omg`
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_call_result_exit/main.omg`

## Hand checks

Verified five disagreements and five soundness-watch rows against raw `route_one_comparison` output (`current=` field vs aggregated `route=`).

### Disagreement spot checks

| Fixture | Harness | Notes |
| --- | --- | --- |
| `tests/omega/fail/facts/wrapping_integer_arithmetic_through_parameter_rejected/main.omg` | current=rejected, compilation route=rejected | Judged agree; negative test rejected on both checkers. |
| `tests/omega/fail/arithmetic/immutable_unsigned_shift_operand_overflow_rejected/main.omg` | current=rejected; `machine:main` route=accepted | Tallied `accepted` vs rejected → disagree (soundness watch). |
| `tests/omega/pass/facts/aggregate_struct_return_abi_through_field_exit/main.omg` | current=accepted, route=rejected | Prover gap: current accepts, route rejects. |
| `tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg` | current=accepted, route=rejected (`unproved=1`) | Same pattern as other pass/facts rows. |
| `tests/omega/pass/build/conditional_root_binding/main.omg` | current=accepted, route=accepted | Judged agree. |

### Soundness-watch spot checks

Each is `fail/` with `--every-machine`: compilation `no-entry`, one machine closure `route=accepted`, current checker still `rejected`.

- `immutable_unsigned_shift_operand_overflow_rejected` (`machine:main` accepted)
- `numeric_operand_abs_unrepresentable`
- `atomic_release_disposition_relaxed`
- `borrows/borrow_mut_and_read`
- `collections/bounded_local_index_write_overrun`

### Timeouts

No `installed_arrival_*` fixtures in shard 3; `route_verdict=timeout` count is 0.


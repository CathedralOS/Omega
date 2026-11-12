# Route coverage shard 3 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at fan-out).
Host: `cursor`. Generated: 2026-11-12 15:53 UTC.

## Verdict rules (shard aggregation)

Route **accepted** only when every root (`compilation` and each `machine:…` from `--every-machine`) is judged `accepted`.
**partial** when any root accepts while another is outside, errored, or otherwise not judged; partial is outside the agreement denominator.
**Soundness watch** = `route_verdict=accepted` and `current_verdict=rejected` (checker rejection, not `stopped`).

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
timeout 300 target/debug/examples/route_one_comparison [--every-machine] tests/omega/<tier>/.../main.omg < /dev/null
python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard_3.py
```

Harness: `target/debug/examples/route_one_comparison` from the build above.

## Execution model

One fixture per process, serial on this VM (`tools/build_slot.md`).

## Totals

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 2174 |
| Route judged (accepted + rejected) | 96 |
| Route partial | 205 |
| Route outside | 1873 |
| Route timeout | 0 |
| Route error | 0 |
| Agreement agree | 75 |
| Agreement disagree | 21 |
| Agreement n-a (outside/partial/timeout/error) | 2078 |
| Soundness watch (route✓ current✗) | 0 |

No soundness-watch rows after partial-aware aggregation.

### Non-construct buckets (outside)

| Bucket | Count |
| --- | ---: |
| `bucket:no-program-entry` | 381 |
| `bucket:current-checker-stopped` | 132 |
| `bucket:prover-incomplete` | 2 |

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

Partial rows: 205 (see `first_refusal` prefix `partial:` in TSV).

## Hand checks (soundness-watch candidates)

No `route_verdict=accepted` + `current_verdict=rejected` rows after all-roots-must-accept aggregation (**0**). The prior 186 count was partial tallying noise (`machine:…` accepted beside `no-entry`/`outside`).

### Hand-check: five former false soundness-watch rows (now `partial`)

| Fixture | `current` | Partial sibling refusing | Real soundness watch? |
| --- | --- | --- | --- |
| `tests/omega/fail/arithmetic/boolean_shift_guard_overwritten/main.omg` | rejected | `no-entry; rejected; rejected; accepted` | **No** — not all roots accepted. |
| `tests/omega/fail/arithmetic/immutable_unsigned_shift_operand_overflow_rejected/main.omg` | rejected | `no-entry; outside; accepted` | **No** — not all roots accepted. |
| `tests/omega/fail/arithmetic/numeric_operand_abs_unrepresentable/main.omg` | rejected | `no-entry; accepted; outside` | **No** — not all roots accepted. |
| `tests/omega/fail/arithmetic/relative_midpoint_unsafe_intermediate/main.omg` | rejected | `no-entry; rejected; accepted` | **No** — not all roots accepted. |
| `tests/omega/fail/arithmetic/unsigned_quotient_operand_landing_rejected/main.omg` | rejected | `no-entry; outside; accepted` | **No** — not all roots accepted. |

### Judged disagreements (prover gaps, not soundness watch)

21 rows: route `rejected`, current `accepted` (all pass-tier in this shard).

| Tier | Count |
| --- | ---: |
| pass | 21 |

Five spot-checked:

- `tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg`: diagnostic «no-checker-diagnostic»; roots `rejected` — **not** soundness watch.
- `tests/omega/pass/facts/aggregate_struct_return_abi_through_field_exit/main.omg`: diagnostic «no-checker-diagnostic»; roots `rejected` — **not** soundness watch.
- `tests/omega/pass/facts/live_across_call_spill_through_other_state_exit/main.omg`: diagnostic «no-checker-diagnostic»; roots `rejected` — **not** soundness watch.
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_call_result_exit/main.omg`: diagnostic «no-checker-diagnostic»; roots `rejected` — **not** soundness watch.
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_parameter_exit/main.omg`: diagnostic «no-checker-diagnostic»; roots `rejected` — **not** soundness watch.

### Timeouts

No `installed_arrival_*` in shard 3; timeout count 0.

### Repository scope

Branch commits from PIN touch only `.agents/four-jobs-2026-11-12/`.


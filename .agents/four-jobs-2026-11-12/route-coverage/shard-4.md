# Route coverage map — shard 4 of 8

Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`.
Harness: `target-o3/debug/examples/route_one_comparison` (sha256 prefix `7ef2305bb71c`).

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \
  mbx build -p compiler --example route_one_comparison < /dev/null
nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/reprocess_shard.py 4  # parser fix
nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 4
```

Per fixture:

```bash
timeout 300 nice -n 19 target-o3/debug/examples/route_one_comparison [--every-machine] <fixture> < /dev/null
```

## Agreement definition and hand checks

Agreement compares `current_verdict` to `route_verdict` where both are `accepted` or `rejected`, using only the harness `current=` field from `route_one_comparison` (not corpus tier, not `corpus_outcomes.txt`). `outside`, `error`, and `timeout` are `n-a`.

Fixture-level `route_verdict` follows the harness aggregate: any `rejected` root wins over an `accepted` sibling closure; otherwise first `outside` construct is recorded in `first_refusal` (detail after `Unsupported:` etc., not the reason enum alone). Non-construct buckets use the `bucket:*` prefix (`bucket:no-program-entry`, `bucket:typed-frontier-stopped`, …).

**Five disagreements (route accepts, current rejects)** — all show `current=rejected` on every `program` line while some `machine:*` root has `route=accepted` even though another machine carries the defect the current checker names:

1. `full_width_literal_requires_explicit_policy_conversion`: `machine:invalid` is `outside` (`expression cast into a non-exact arithmetic domain`); `machine:exact` is `route=accepted` while `current=rejected`.
2. `immutable_unsigned_shift_underbound_rejected`: `machine:invalid` outside on contract term; `machine:main` `route=accepted`; current diagnostic targets `invalid`'s ensures.
3. `finite_prefix_skipped_fact`: `machine:seven` `accepted`; `machine:convert` outside on `type addr`.
4. `instruction_disposition_saved_subject_reused`: `machine:main` `accepted`; other machines outside on service reach / assembly target.
5. `instruction_effect_skipped_write`: `machine:main` `accepted`; `machine:observe` outside on `machine assembly target`.

These are genuine `disagree` rows, not tier mix-ups; they are soundness-watch candidates for reconcile.

**Five agreements (both reject)**:

1. `exact_integer_cast_unproven`: `machine:narrow` `route=rejected` with `unproved=1` (wins over `main` accepted).
2. `relative_midpoint_zero_divisor`: `machine:midpoint` `route=rejected`.
3. `short_circuit_cast_wrong_path`: `machine:narrow` and `machine:main` both `route=rejected`.
4. `unequal_observations_do_not_compose`: multiple machines `route=rejected` despite `machine:empty` accepted.
5. `unsigned_quotient_operand_overflow_rejected`: `machine:invalid` `route=rejected`.

## Host build budget (`tools/build_slot.md`)

This shard ran **one fixture per process, serially** (`run_shard.py` / `reprocess_shard.py`), with `nice -n 19` and `timeout 300`. That matches test-VM guidance: one heavy compiler job at a time; no parallel fixture workers on this machine.

## `installed_arrival_*` timeouts

No `installed_arrival_*` fixture falls in shard 4 (`i % 8 == 4` over sorted `main.omg` paths). Nothing to re-verify for the ~6 minute arrival family in this shard.

## Totals (this shard)

| Fixtures measured | 2174 |
| Route judged (accepted ∪ rejected) | 238 |
| Agree | 98 |
| Disagree | 140 |
| Outside (construct refusal) | 1446 |
| Outside / error (bucket rows) | 490 |
| Timeout | 0 |

## Top first-refusal constructs (outside, excluding buckets)

| construct | count | examples |
| --- | ---: | --- |
| machine service reach | 319 | `tests/omega/fail/arithmetic/modular_power_state_seed_outside_bounds/main.omg`, `tests/omega/fail/borrows/boundary_primitive_loan_mutable_from_shared/main.omg`, `tests/omega/fail/borrows/stored_reference_child_constructor_live_source_write/main.omg` |
| parameter mutable parameter | 221 | `tests/omega/fail/borrow/aggregate_field_loan_requires_handback/main.omg`, `tests/omega/fail/borrow/returned_write_destination_argument_read/main.omg`, `tests/omega/fail/borrow/write_only_state_readable_widening/main.omg` |
| machine generic parameters | 159 | `tests/omega/fail/borrows/computed_view_store_retains_local_array_backing/main.omg`, `tests/omega/fail/borrows/constructed_view_argument_retains_completed_child_backing/main.omg`, `tests/omega/fail/borrows/copied_descriptor_operand_retains_consumed_backing/main.omg` |
| type reference | 126 | `tests/omega/fail/arithmetic/collection_result_changed_predecessor/main.omg`, `tests/omega/fail/arithmetic/collection_result_wrong_argument/main.omg`, `tests/omega/fail/atomics/atomic_boolean_fetch/main.omg` |
| declaration Build selection that is not a machine or a data type | 122 | `tests/omega/fail/boundary/requirement_case_precondition_unproved/main.omg`, `tests/omega/fail/build/component_scalar_shared_provider_mutation/producer/main.omg`, `tests/omega/fail/build/segment_selector_shared_gdt/main.omg` |
| type data with custody | 85 | `tests/omega/fail/arithmetic/boolean_shift_guard_subject_written/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_signed_minimum/main.omg`, `tests/omega/fail/borrow/captured_window_write_through_keeps_loan/main.omg` |
| machine attached data | 74 | `tests/omega/fail/arithmetic/bitwise_numeric_into_bool_rejected/main.omg`, `tests/omega/fail/arithmetic/exact_shift_left_value_unproven/main.omg`, `tests/omega/fail/arithmetic/implicit_store_widening_rejected/main.omg` |
| type constrained | 44 | `tests/omega/fail/arithmetic/captured_boolean_replaced_scalar/main.omg`, `tests/omega/fail/borrows/qualified_state_loan_invalid_membership/main.omg`, `tests/omega/fail/domains/domain_pattern_match_bound_routed_boolean_unproven_rejected/main.omg` |
| type float | 31 | `tests/omega/fail/facts/crash_entry_ieee_view_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_nan_equality_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_signed_zero_through_field_rejected/main.omg` |
| type fixed array | 29 | `tests/omega/fail/build/boundary_repeated_record_shape_bound/main.omg`, `tests/omega/fail/calls/foreign_scalar_indirect_result_parameter_overlap_rejected/main.omg`, `tests/omega/fail/collections/anonymous_fractional_call_index/main.omg` |

## Bucket summary (non-construct)

- `bucket:no-program-entry`: 344
- `bucket:typed-frontier-stopped`: 146

Soundness watch (route accepts, current rejects): **122** fixtures.

## Timeouts

None.

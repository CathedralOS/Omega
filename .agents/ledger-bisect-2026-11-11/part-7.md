# Ledger bisect part 7 (k=7, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **6** (report indices 7, 47, 87, 127, 167, 207)
- Fixture rows re-measured: **31**
- Fixed on PIN: **27**
- Still broken on PIN: **4**
- PIN diagnostic clusters (after regroup): **3**

## PIN clusters (regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run rejected | terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view | `pass/termination/measure_field_computed_limit` | `f3612364ffbb` | 2228 | tests: record four case-payload rows that already check (#2228) (tests/omega/corpus_outcomes.txt) | bisected |
Members:
- `pass/termination/measure_computed_endpoint` — still broken: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view
- `pass/termination/measure_field_computed_limit` — still broken: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view

| 1 | pass/run rejected | proposition `chained` expects 2 proof-static binder argument(s), received 0 | `pass/proofs/family_carrier_relation_laws` | `e06a115a8c16` | 945 | psi: implement named FixedLayout policies and explicit representation storage (#945) (Cargo.lock, TASKS.md, omega-rust/omega/application/terminal-runtime/src/ex | bisected |
Members:
- `pass/proofs/family_carrier_relation_laws` — still broken: proposition `chained` expects 2 proof-static binder argument(s), received 0

| 1 | pass/run rejected | terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the rank produced by the selected view | `pass/proofs/kernel_integer_subtract_order` | `f3612364ffbb` | 2228 | tests: record four case-payload rows that already check (#2228) (tests/omega/corpus_outcomes.txt) | bisected |
Members:
- `pass/proofs/kernel_integer_subtract_order` — still broken: terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the rank produced by the se

## Fixed on main (PIN)

- `fail/arithmetic/wrapping_target_plain_operands_rejected`
- `fail/facts/source_indexed_type_parameter_through_call_result_rejected`
- `fail/facts/source_indexed_type_parameter_through_const_name_rejected`
- `fail/facts/source_indexed_type_parameter_through_field_rejected`
- `fail/facts/source_indexed_type_parameter_through_local_rejected`
- `fail/facts/source_indexed_type_parameter_through_match_join_rejected`
- `fail/facts/source_indexed_type_parameter_through_other_machine_rejected`
- `fail/facts/source_indexed_type_parameter_through_other_state_rejected`
- `fail/facts/source_indexed_type_parameter_through_parameter_rejected`
- `fail/facts/source_indexed_type_parameter_through_target_body_rejected`
- `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected`
- `fail/layouts/target_layout_call_checks_all_widths`
- `fail/proofs/kernel_integer_subtract_order_false_twin`
- `pass/borrows/foreign_output_captured_reference_exit`
- `pass/facts/source_indexed_type_parameter_exit`
- `pass/facts/source_indexed_type_parameter_through_call_result_exit`
- `pass/facts/source_indexed_type_parameter_through_const_name_exit`
- `pass/facts/source_indexed_type_parameter_through_field_exit`
- `pass/facts/source_indexed_type_parameter_through_local_exit`
- `pass/facts/source_indexed_type_parameter_through_match_join_exit`
- `pass/facts/source_indexed_type_parameter_through_other_machine_exit`
- `pass/facts/source_indexed_type_parameter_through_other_state_exit`
- `pass/facts/source_indexed_type_parameter_through_parameter_exit`
- `pass/facts/source_indexed_type_parameter_through_target_body_exit`
- `pass/facts/source_indexed_type_parameter_u64_payload_exit`
- `pass/layouts/runtime_recast_reference_captures_index_exit`
- `pass/recast/runtime_byte_region_state_transfer_exit`

## Legacy slice members (report cluster 2026-11-11)

### report index 7 (report rows=18)
- Witness: `fail/facts/source_indexed_type_parameter_through_call_result_rejected`
- Report diagnostic prefix: source constructor predicate derivation failed: Kernel(UnboundLevelParameter { index: 0, arity:
- `fail/facts/source_indexed_type_parameter_through_call_result_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_const_name_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_field_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_local_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_match_join_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_other_machine_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_other_state_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_parameter_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_target_body_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected` — fixed on main
- `pass/facts/source_indexed_type_parameter_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_call_result_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_const_name_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_field_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_local_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_match_join_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_other_machine_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_other_state_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_parameter_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_target_body_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_u64_payload_exit` — fixed on main

### report index 47 (report rows=4)
- Witness: `pass/proofs/kernel_integer_subtract_order`
- Report diagnostic prefix: terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the
- `pass/proofs/kernel_integer_subtract_order` — still broken
- `fail/proofs/kernel_integer_subtract_order_false_twin` — fixed on main
- `pass/termination/measure_computed_endpoint` — still broken
- `pass/termination/measure_field_computed_limit` — still broken

### report index 87 (report rows=2)
- Witness: `pass/layouts/runtime_recast_reference_captures_index_exit`
- Report diagnostic prefix: duplicate machine `_`
- `pass/layouts/runtime_recast_reference_captures_index_exit` — fixed on main
- `fail/layouts/target_layout_call_checks_all_widths` — fixed on main
- `pass/recast/runtime_byte_region_state_transfer_exit` — fixed on main

### report index 127 (report rows=1)
- Witness: `pass/proofs/family_carrier_relation_laws`
- Report diagnostic prefix: authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity
- `pass/proofs/family_carrier_relation_laws` — still broken

### report index 167 (report rows=1)
- Witness: `pass/borrows/foreign_output_captured_reference_exit`
- Report diagnostic prefix: completed output has no captured lifetime-input receipt; completed output from `again` in machin
- `pass/borrows/foreign_output_captured_reference_exit` — fixed on main

### report index 207 (report rows=1)
- Witness: `fail/arithmetic/wrapping_target_plain_operands_rejected`
- Report diagnostic prefix: store in machine `Main::pick` state `pick` local `v` implicitly converts `i32` to `i32 in Wrappi
- `fail/arithmetic/wrapping_target_plain_operands_rejected` — fixed on main

## Provenance

- Host: linux x86_64
- Jobs: `OMEGA_CORPUS_JOBS=1`
- Commands:
  ```bash
  mbx build -p omega < /dev/null
  mbx test --no-run -p compiler --test corpus_runner < /dev/null
  OMEGA_CORPUS_FIXTURE_FILTER=<fixtures> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/pin_records.txt \
    target/debug/build/compiler/*/out/corpus_runner-* < /dev/null
  ```

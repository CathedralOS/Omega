# Ledger bisect part 20

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Re-measure (slice fixtures)

- fixed on main: 11
- still broken: 5
- total rows: 16

## Clusters at PIN (regrouped by first diagnostic)

| Rows | Kind | Diagnostic (truncated) | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run rejected | native artifact target lowering failed: UnsupportedControlFlow { machine: Machin | `pass/borrows/incoming_reference_slot_replacement_exit` | `60d14b27b1eb` | 2308 | psi state-exit requires-window checking in domains_and_predicates.rs regressed native unsupported-co | bisected |
| 1 | pass/run rejected | statement 8 mutates first.value while local borrow first_input is still active | `pass/borrows/runtime_selected_reference_record_helper_exit` | `2ad42b167126` | 2219 | psi call checking reuses stored-value facts in checks_contracts/flow_place, tightening borrow livene | bisected |
| 1 | pass/run rejected | index compatibility condition index-equality not established (distinct normalize | `pass/generics/open_computed_quantity_result` | `` |  |  | pending |
| 1 | pass/run rejected | terminal-artifact Lowering(OverlappingProjectedCustody) | `pass/borrows/runtime_partial_reference_partition_exit` | `` |  |  | pending |

### Members

**cluster_ucf**
- `pass/borrows/incoming_reference_slot_replacement_exit` (still broken)
- `pass/borrows/runtime_owned_carrier_descriptor_replacement_exit` (still broken)
- `pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit` (fixed on main)

**cluster_borrow**
- `pass/borrows/runtime_selected_reference_record_helper_exit` (still broken)

**cluster_index**
- `pass/generics/open_computed_quantity_result` (still broken)

**cluster_overlap**
- `pass/borrows/runtime_partial_reference_partition_exit` (still broken)

## Fixed-on-main (all slice members)

- `pass/atomics/runtime_stored_reference_atomic_events_exit`
- `pass/facts/write_only_indexed_reborrow_through_call_result_exit`
- `pass/facts/write_only_indexed_reborrow_through_const_name_exit`
- `pass/facts/write_only_indexed_reborrow_through_field_exit`
- `pass/facts/write_only_indexed_reborrow_through_local_exit`
- `pass/facts/write_only_indexed_reborrow_through_match_join_exit`
- `pass/recast/runtime_aggregate_slice_representation_recast_exit`
- `pass/slices/saved_byte_predicate_fields_disjoint_exit`
- `pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit`
- `fail/calls/static_boundary_statement_misplaced_evidence`
- `fail/facts/four_variant_enum_payload_bound_through_call_result_rejected`

## Commands

```bash
cp tests/omega/corpus_outcomes.txt /tmp/bisect_pin_corpus_outcomes.txt  # at PIN
mbx build -p omega && mbx build -p compiler --bins --tests
corpus_runner with OMEGA_CORPUS_FIXTURE_FILTER (16 fixtures) -> /tmp/part20_pin.txt
.agents/ledger-bisect-2026-11-11/bisect_cluster.sh <witness> <tag>
```

Host: cloud VM single job, OMEGA_CORPUS_JOBS=1, mbx per-step rebuild.


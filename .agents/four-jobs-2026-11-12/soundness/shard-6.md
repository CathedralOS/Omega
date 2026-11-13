# Soundness mutations shard 6 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Status: complete

Lead review buckets (every mutant row belongs to exactly one bucket below).

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison
export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD
python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
```

## Fixture coverage

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 1130 |
| original-not-accepted | 208 |
| failed-run (original check) | 0 |
| no-mutation | 103 |
| mutated fixtures | 819 |
| Mutant rows | 2286 |
| Mean mutation kinds per mutated fixture | 2.66 |

Reconcile: `fixtures in shard` = original-not-accepted + failed-run + no-mutation + mutated; mutant row buckets sum to `mutant rows`.

## Original-not-accepted diagnostics (top)

| Diagnostic (prefix) | Count |
| --- | ---: |
| error: prior run checked main.omg only; corpus uses build.omg when present | 208 |

## Mutation kinds (all rows)

| Kind | Count |
| --- | ---: |
| `drop_requires` | 553 |
| `mut_ref_instead_of_shared` | 669 |
| `remove_mut` | 309 |
| `shared_instead_of_mut` | 367 |
| `widen_requires` | 388 |

drop_requires header vs body: header=49, body=504

## Q7 mut split (known-gap-q7-mut rows)

- In PR #2638 audit list: **1**
- Same Q7 enforcement class, not audit-listed: **311**
- Audit sites loaded: 173 distinct (path, binding)

## Mutant buckets (sum = mutants attempted)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 57 |
| `known-gap-q7-mut` | 312 |
| `disagreement-valid` | 5 |
| `valid-route-outside` | 265 |
| `discarded-valid` | 92 |
| `rejected-both` | 74 |
| `rejected-current-route-outside` | 1481 |
| `timeout` | 0 |
| `failed-run` | 0 |
| **Total** | **2286** |

## Bucket integrity

- rejected-both rows with current=rejected AND route=rejected: 74 / 74
- valid-route-outside anomalies: 0
- rejected-both anomalies (not both rejected): 0
- mutants missing route verdict: 0

## Fixtures with more than one mutant row

Count: 649 fixtures.
- `tests/omega/pass/arithmetic/checked_flag_proof_signed_mask/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg`: 5 rows
- `tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg`: 4 rows
- `tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg`: 3 rows
- `tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg`: 5 rows
- `tests/omega/pass/asm/atomic_release_disposition_pair_exit/main.omg`: 2 rows
- `tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg`: 5 rows
- `tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg`: 3 rows
- `tests/omega/pass/atomics/placed_atomic_shared_store/main.omg`: 2 rows
- `tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg`: 4 rows
- `tests/omega/pass/borrow/runtime_static_callable_preserves_readonly_subject_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_aggregate_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_returned_view_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/aliased_descriptor_slot_after_last_use_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/borrowed_literal_backing_releases_input_loan_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/copied_input_descriptor_independent_mutation_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/foreign_output_copied_lifetime_last_use/main.omg`: 3 rows
- `tests/omega/pass/borrows/initialized_empty_array_snapshot_replacement_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/literal_index_reference_copy_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/ordinary_select_provider_reference_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/qualified_reference_field_retarget_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_computed_backing_reference_copy_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_match_payload_parent_disjoint_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_reference_call_scalar_snapshot_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_selected_stored_carrier_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg`: 4 rows
- … and 609 more (see TSV)

## Confirmed holes by mutation kind

| Kind | Count |
| --- | ---: |
| `drop_requires` | 5 |
| `mut_ref_instead_of_shared` | 68 |
| `remove_mut` | 244 |
| `shared_instead_of_mut` | 45 |
| `widen_requires` | 7 |

## Confirmed holes by accepted_by

| accepted_by | Count |
| --- | ---: |
| `both` | 9 |
| `current` | 360 |

## Kept rows (sample)

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `values`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `28:     values[direct] = 2.0;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `guard`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `5:     guard.count = count;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `46:         output[output_offset] = narrow_u32_to_u16_wrapping(character.point);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `cursor`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor) + 11);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `30:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/atomics/placed_atomic_shared_store/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__placed_atomic_shared_store__main.omg__remove_mut__698bd3fb4dace34b/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `6:     entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `trace`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_atomic_computed_updates_exit__main.omg__remove_mut__b0b9c59434b0adde/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `6:     trace.code = trace.code * 10 + digit;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `cells`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_indexed_write_only_local_receiver_exit__main.omg__remove_mut__056e2e20b4562b36/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `8:     Cell::replace(&write cells[position], value);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_aggregate_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `held`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_aggregate_exit__main.omg__remove_mut__eba29fb81a6e84fd/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `12:     replace(&write held, incoming);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_returned_view_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `held`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_returned_view_exit__main.omg__remove_mut__026c93f80ef2a4a9/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `12:     replace(&write held, identity(identity(incoming)));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/borrowed_literal_backing_releases_input_loan_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `second`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__borrowed_literal_backing_releases_input_loan_exit__main.omg__remove_mut__fe9e2e25b552a337/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     second.payload = 29i32;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/copied_input_descriptor_independent_mutation_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `counter`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__copied_input_descriptor_independent_mutation_exit__main.omg__remove_mut__3fbf375251282eef/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `11:     let observed: i32 = relay(view, &mut counter);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/foreign_output_copied_lifetime_last_use/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__foreign_output_copied_lifetime_last_use__main.omg__remove_mut__961c6eec0e3affec/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `21:     first[0] = 51;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/initialized_empty_array_snapshot_replacement_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `packet`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__initialized_empty_array_snapshot_replacement_exit__main.omg__remove_mut__039a175236069329/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `12:  packet.before = 17;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/literal_index_reference_copy_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `bytes`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__literal_index_reference_copy_exit__main.omg__remove_mut__d88ab24fb0f32c9e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `59:     bytes[1] = 42;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `bytes`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__remove_mut__95a8b46df06a7f29/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `31:     let view: &mut [u8] = &mut bytes[..];`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — mut_ref_instead_of_shared

- Bucket: `known-gap-q7-mut`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__amp_to_ampmut__d31452bfce2909de/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `8: machine zero(view: &mut [u8])`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `backing`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__remove_mut__42f3387b9632fc4c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `17:     let bundle: Bundle = Bundle { windows: [&mut backing[..]] };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/ordinary_select_provider_reference_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__ordinary_select_provider_reference_exit__main.omg__remove_mut__3a40fd137fb6aedb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `4:     holder.reference = incoming;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `items`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__remove_mut__5d26dbcef23c1942/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `26:     let view: &mut [u8] = &mut items[..];`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — mut_ref_instead_of_shared

- Bucket: `known-gap-q7-mut`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__amp_to_ampmut__1644168050b9d4d2/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `6: // `&mut` loan's last use; `right_seen` reads a field disjoint from the live`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/qualified_reference_field_retarget_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `protected`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__qualified_reference_field_retarget_exit__main.omg__remove_mut__7e9af1b913ab71dd/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `11:     protected.cell = &replacement;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__resident_view_borrowed_helper_exit__main.omg__remove_mut__6bfab553627f460c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_copied_byte_distinct_epochs_exit__main.omg__remove_mut__ebef68fdc4e399fb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `59:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `original`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_formed_child_descriptor_replacement_exit__main.omg__remove_mut__d1996063f1a77255/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `18:     let mut message: Message = Message { body: &mut original };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_match_payload_parent_disjoint_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `backing`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_match_payload_parent_disjoint_exit__main.omg__remove_mut__b715854371e6096e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `13:     let mut container: Message = Message { body: &mut backing };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `cells`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_mutable_reference_captures_selector_exit__main.omg__remove_mut__14d56be69b40f6cc/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:     replace(&mut cells, &mut cursor);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__remove_mut__8ff6ce3aaf315ffa/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `78:     walk_descriptors(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — mut_ref_instead_of_shared

- Bucket: `known-gap-q7-mut`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__amp_to_ampmut__2d7d2472dd381be2/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `27: machine walk_descriptors(main: &mut Main) crashes Trap {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `original`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_qualified_statement_descriptor_replacement_exit__main.omg__remove_mut__73041d9fe1e00b5a/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     let mut message: Message = Message { body: &mut original };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_reference_call_scalar_snapshot_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_reference_call_scalar_snapshot_exit__main.omg__remove_mut__4e323ffe493b9b53/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_selected_stored_carrier_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `left`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_selected_stored_carrier_exit__main.omg__remove_mut__45ed5c0817a0b3a5/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `17:     left.value = 29;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `visits`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_static_callback_snapshot_exit__main.omg__remove_mut__d3ddec1278f4fccd/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `29:     let tick_index: u64 = call_counter<record_tick>(&mut visits);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/borrows/runtime_writeonly_nested_reference_array_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_writeonly_nested_reference_array_exit__main.omg__remove_mut__9aa182cde73c0f94/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:     prepare(&write first, &write second, &write third, &write fourth);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/build/activation_descriptor_write_observed_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `carrier`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__activation_descriptor_write_observed_exit__main.omg__remove_mut__1b026a1991142698/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `22:     apply(&mut carrier)`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/build/installed_entry_ram_two_occurrences_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__installed_entry_ram_two_occurrences_exit__main.omg__remove_mut__dfc12f4db32fd97d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/build/sibling_module_service_exclusion_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__sibling_module_service_exclusion_exit__main.omg__remove_mut__768c7eac2355bdd0/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `25:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — drop_requires

- Bucket: `disagreement-valid`
- Mutation: delete first body requires line
- Mutant: `tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__drop_requires__8f385fdaff67e638`
- Current: `accepted`; route: `rejected`; accepted_by: `current`
- Route detail (hint only): diagnostic: reject main.omg:27 bytes 890..901 cannot prove requires contract for call verify_wide from step: forwarded >= 300 && forwarded <= 500 (needs fact `wide >= 300 && wide <= 500` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` 
- Broken obligation: ``
- (a) Compile-time spec (`` § ):

> 
- Subtype: `(n/a)`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `rejected`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — mut_ref_instead_of_shared

- Bucket: `known-gap-q7-mut`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__amp_to_ampmut__fde2c5322096d104/main.omg`
- Current: `accepted`; route: `rejected`; accepted_by: `current`
- Route detail (hint only): diagnostic: reject main.omg:28 bytes 933..944 cannot prove requires contract for call verify_wide from step: forwarded >= 300 && forwarded <= 500 (needs fact `wide >= 300 && wide <= 500` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry` 
- Broken obligation: `1: // Fact: zero-extending a bounded 16-bit unsigned u16 value (narrow >= 300u16 &mut& narrow <= 500u16) to u32 (narrow as u32) preserves the exact interval (wide >= 300u32 && wide <= 500u32), discharging exact 32-bit subtractions 500u32 - wide and wide - 300u32 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `rejected`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/calls/recursive_terminal_block_acknowledgement_exit/main.omg — remove_mut

- Bucket: `known-gap-q7-mut`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__recursive_terminal_block_acknowledgement_exit__main.omg__remove_mut__0ce9200e1feb5580/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `42:     block main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.


## Hand verification (5 confirmed-hole)

#### Hand verification: tests/omega/pass/control_flow/constant_aggregate_dispatch_complete/main.omg / drop_requires

- Bucket: `confirmed-hole`
- Obligation: `14:     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; } (arithmetic on `hi` formerly guarded by `(self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))`)`

**Mutant diff (excerpt)**
```diff
-     requires (self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))
+     {
- {
+     case Empty;
-     case Empty;
+     case Point(lo: u64, hi: u64);
-     case Point(lo: u64, hi: u64);
+ }
- }
+ const BUNDLE: [Interval; 1] = [Interval::Point { lo: 4, hi: 4 }];
- const BUNDLE: [Interval; 1] = [Interval::Point { lo: 4, hi: 4 }];
+ data Main { width: u64; }
- data Main { width: u64; }
+ machine Main::main(main: &mut Main) {
- machine Main::main(main: &mut Main) {
+     transition BUNDLE[0] {
-     transition BUNDLE[0] {
+         Interval::Point { lo, hi } -> take(lo, hi, main)
-         Interval::Point { lo, hi } -> take(lo, hi, main)
+         Interval::Empty -> done()
-         Interval::Empty -> done()
+     }
-     }
+     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; }
-     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; }
+     machine done() {}
-     machine done() {}
+ }
- }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__constant_aggregate_dispatch_complete__main.omg__drop_requires__1dc76d7a0fe9e0da/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__constant_aggregate_dispatch_complete__main.omg__drop_requires__1dc76d7a0fe9e0da/main.omg	current=accepted	current_us=2365855	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md): A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. Checked compilation must establi

#### Hand verification: tests/omega/pass/machines/computed_arrival_state/main.omg / widen_requires

- Bucket: `confirmed-hole`
- Obligation: `7:     transition { _ -> tally(remaining, base + step) } (arithmetic on `base` formerly guarded by `0 <= remaining && remaining <= 5 && 0 <= base && base <= 5 && 0 <= step && step <= 5`)`

**Mutant diff (excerpt)**
```diff
-     requires 0 <= remaining && remaining <= 5 && 0 <= base && base <= 5 && 0 <= step && step <= 5
+     requires 0 < remaining && remaining <= 5 && 0 <= base && base <= 5 && 0 <= step && step <= 5
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__machines__computed_arrival_state__main.omg__weaken_requires__433d9c3d6b6640c1/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__machines__computed_arrival_state__main.omg__weaken_requires__433d9c3d6b6640c1/main.omg	current=accepted	current_us=2109227	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/machines.md): Every machine application establishes the selected contract's `requires` under the exact argument substitution. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/dependent/closed_case_common_and_active_payload_projection/main.omg / drop_requires

- Bucket: `confirmed-hole`
- Obligation: `16:     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; } (arithmetic on `hi` formerly guarded by `((self in Interval::Point) && (self.lo == self.hi)) || (self in Interval::Other)`)`

**Mutant diff (excerpt)**
```diff
-     requires ((self in Interval::Point) && (self.lo == self.hi)) || (self in Interval::Other)
+     {
- {
+     marker: u64;
-     marker: u64;
+     case Point(lo: u64, hi: u64);
-     case Point(lo: u64, hi: u64);
+     case Other(lo: u64, hi: u64);
-     case Other(lo: u64, hi: u64);
+ }
- }
+ data Main { width: u64; }
- data Main { width: u64; }
+ machine Main::main(main: &mut Main) {
- machine Main::main(main: &mut Main) {
+     transition (Interval::Point { marker: 9, lo: 4, hi: 4 }) {
-     transition (Interval::Point { marker: 9, lo: 4, hi: 4 }) {
+         Interval { marker as _, case as operation } -> match operation {
-         Interval { marker as _, case as operation } -> match operation {
+             Interval::Point { lo, hi } -> take(lo, hi, main)
-             Interval::Point { lo, hi } -> take(lo, hi, main)
+             Interval::Other { lo as _, hi as _ } -> done()
-             Interval::Other { lo as _, hi as _ } -> done()
+         }
-         }
+     }
-     }
+     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; }
-     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; }
+     machine done() {}
-     machine done() {}
+ }
- }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__dependent__closed_case_common_and_active_payload_projection__main.omg__drop_requires__336fa8e7e0b609b1/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__dependent__closed_case_common_and_active_payload_projection__main.omg__drop_requires__336fa8e7e0b609b1/main.omg	current=accepted	current_us=1867494	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md): A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. Checked compilation must establi

#### Hand verification: tests/omega/pass/slices/signed_index_and_range_lower_bounds/main.omg / widen_requires

- Bucket: `confirmed-hole`
- Obligation: `4:     items[index] (index uses `items` formerly guarded by `0 <= index && index < items.len`)`

**Mutant diff (excerpt)**
```diff
- requires 0 <= index && index < items.len
+ requires 0 < index && index < items.len
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__slices__signed_index_and_range_lower_bounds__main.omg__weaken_requires__c125ebaaafbd29e3/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__slices__signed_index_and_range_lower_bounds__main.omg__weaken_requires__c125ebaaafbd29e3/main.omg	current=accepted	current_us=1722339	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/machines.md): Every machine application establishes the selected contract's `requires` under the exact argument substitution. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/termination/computed_rank_endpoint/main.omg / drop_requires

- Bucket: `confirmed-hole`
- Obligation: `7: terminates by remaining in 0..(cap / 5 + 6) { (division uses `cap` formerly guarded by `band `0 <= cap <= 20`; the iterate`)`

**Mutant diff (excerpt)**
```diff
- // strengthened by the requires band `0 <= cap <= 20`; the iterate
+ // strengthened by the // state's own requires repins both bands.
- // state's own requires repins both bands.
+ machine walk(remaining: u64, cap: u64) -> u64
- machine walk(remaining: u64, cap: u64) -> u64
+     requires 0 <= remaining && remaining <= 5 && 0 <= cap && cap <= 20
-     requires 0 <= remaining && remaining <= 5 && 0 <= cap && cap <= 20
+ terminates by remaining in 0..(cap / 5 + 6) {
- terminates by remaining in 0..(cap / 5 + 6) {
+     transition { _ -> iterate(cap, remaining) }
-     transition { _ -> iterate(cap, remaining) }
+     machine iterate(limit: u64, pending: u64) -> u64
-     machine iterate(limit: u64, pending: u64) -> u64
+         requires 0 <= limit && limit <= 20 && 0 <= pending && pending <= 5
-         requires 0 <= limit && limit <= 20 && 0 <= pending && pending <= 5
+     terminates by pending in 0..=5
-     terminates by pending in 0..=5
+     {
-     {
+         transition pending > 0 {
-         transition pending > 0 {
+             true -> iterate(limit - 0, pending - 1)
-             true -> iterate(limit - 0, pending - 1)
+             false -> pending
-             false -> pending
+         }
-         }
+     }
-     }
+ }
- }
+ 
- 
+ data Main {}
- data Main {}
+ machine Main::main(main: &mut Main) { }
- machine Main::main(main: &mut Main) { }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__termination__computed_rank_endpoint__main.omg__drop_requires__2115a6749be74983/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__termination__computed_rank_endpoint__main.omg__drop_requires__2115a6749be74983/main.omg	current=accepted	current_us=1863072	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md): A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. Checked compilation must establi


## Hand verification (5 discarded-valid)

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — could not regen mutant
#### Hand verification: tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg / widen_requires

- Bucket: `discarded-valid`
- Obligation: `11:     view[0] = 0; (index uses `view` formerly guarded by `view.len >= 1`)`

**Mutant diff (excerpt)**
```diff
- requires view.len >= 1
+ requires view.len > 1
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__widen_requires__435aebcce7b6daff/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__widen_requires__435aebcce7b6daff/main.omg	current=accepted	current_us=3368565	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

#### Hand verification: tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg — could not regen mutant
#### Hand verification: tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg / widen_requires

- Bucket: `discarded-valid`
- Obligation: `15:     view[0] = 0; (index uses `view` formerly guarded by `view.len >= 1`)`

**Mutant diff (excerpt)**
```diff
- requires view.len >= 1
+ requires view.len > 1
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__widen_requires__cfde70333307ace9/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__widen_requires__cfde70333307ace9/main.omg	current=accepted	current_us=3642430	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

#### Hand verification: tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — could not regen mutant

## Hand verification (5 known-gap-q7-mut)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / remove_mut

- Bucket: `known-gap-q7-mut`
- Obligation: `28:     values[direct] = 2.0;`

**Mutant diff (excerpt)**
```diff
-     let mut values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
+     let values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1593325	root=compilation	route=no-entry

```


> Spec (wiki/spec/language/expressions.md): Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg / remove_mut

- Bucket: `known-gap-q7-mut`
- Obligation: `5:     guard.count = count;`

**Mutant diff (excerpt)**
```diff
-     let mut guard: Guard;
+     let guard: Guard;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=3362522	root=compilation	route=outside	Unsupported: type data with custody

```


> Spec (wiki/spec/language/expressions.md): Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg / remove_mut

- Bucket: `known-gap-q7-mut`
- Obligation: `46:         output[output_offset] = narrow_u32_to_u16_wrapping(character.point);`

**Mutant diff (excerpt)**
```diff
-     let mut output: [u16; 4096];
+     let output: [u16; 4096];
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3883393	root=compilation	route=outside	Unsupported: type reference

```


> Spec (wiki/spec/language/expressions.md): Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg / remove_mut

- Bucket: `known-gap-q7-mut`
- Obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor) + 11);`

**Mutant diff (excerpt)**
```diff
-     let mut cursor: u64 = 2;
+     let cursor: u64 = 2;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg	current=accepted	current_us=3346709	root=compilation	route=outside	Unsupported: contract term other than a formal or literal

```


> Spec (wiki/spec/language/expressions.md): Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.

#### Hand verification: tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg / remove_mut

- Bucket: `known-gap-q7-mut`
- Obligation: `30:     Main::main(&mut entry_storage);`

**Mutant diff (excerpt)**
```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg	current=accepted	current_us=3280972	root=compilation	route=outside	Unsupported: type reference

```


> Spec (wiki/spec/language/expressions.md): Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.


## Hand verification (5 valid-route-outside)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — could not regen mutant
#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / widen_requires

- Bucket: `valid-route-outside`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
- requires position >= 0 && position <= 3 && (side == 0 || side == 1)
+ requires position > 0 && position <= 3 && (side == 0 || side == 1)
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__widen_requires__d9d0b9a2e04725c2/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__widen_requires__d9d0b9a2e04725c2/main.omg	current=accepted	current_us=1488426	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / widen_requires

- Bucket: `valid-route-outside`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
- requires position >= 0 && position <= 3 && (side == 0 || side == 1)
+ requires position >= 0 && position < 3 && (side == 0 || side == 1)
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__widen_requires__b82daf74280bd955/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__widen_requires__b82daf74280bd955/main.omg	current=accepted	current_us=1545955	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg / remove_mut

- Bucket: `valid-route-outside`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
-     let mut condition: Condition = Condition { selected: false };
+     let condition: Condition = Condition { selected: false };
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__remove_mut__87faae966e7fddd1/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__remove_mut__87faae966e7fddd1/main.omg	current=accepted	current_us=3376442	root=compilation	route=outside	Unsupported: type constrained

```

#### Hand verification: tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg / remove_mut

- Bucket: `valid-route-outside`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
-         let mut cell: i32 = byte;
+         let cell: i32 = byte;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__remove_mut__b9a3bdab948d8393/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__remove_mut__b9a3bdab948d8393/main.omg	current=accepted	current_us=4546828	root=compilation	route=outside	Unsupported: machine service reach

```


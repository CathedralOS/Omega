# Soundness mutations shard 6 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Status: in progress

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
| Originals skipped | 128 |
| Fixtures checked (original accepted) | 1002 |
| Fixtures with zero generated mutants (no-mutation) | 51 |
| Fixtures with ≥1 mutant row | 446 |
| Mutant rows | 1015 |
| Mean mutants per mutated fixture | 2.28 |

Reconcile: `fixtures in shard` = skipped + checked; `checked` = no-mutation + mutated fixtures; mutant row buckets sum to `mutant rows`.

## Mutant buckets (sum = mutants attempted)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 176 |
| `known-gap-q7-mut` | 0 |
| `disagreement-valid` | 3 |
| `valid-route-outside` | 165 |
| `discarded-valid` | 42 |
| `rejected-both` | 629 |
| `timeout` | 0 |
| `failed-run` | 0 |
| `failed-run` | 0 |
| **Total** | **1015** |

## Fixtures with more than one mutant row

Count: 334 fixtures.
- `tests/omega/pass/arithmetic/checked_flag_proof_signed_mask/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg`: 5 rows
- `tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg`: 4 rows
- `tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg`: 2 rows
- `tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg`: 4 rows
- `tests/omega/pass/asm/atomic_release_disposition_pair_exit/main.omg`: 2 rows
- `tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg`: 4 rows
- `tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg`: 2 rows
- `tests/omega/pass/atomics/placed_atomic_shared_store/main.omg`: 2 rows
- `tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_static_callable_preserves_readonly_subject_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_aggregate_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_returned_view_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/aliased_descriptor_slot_after_last_use_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/borrowed_literal_backing_releases_input_loan_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/copied_input_descriptor_independent_mutation_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/foreign_output_copied_lifetime_last_use/main.omg`: 2 rows
- `tests/omega/pass/borrows/initialized_empty_array_snapshot_replacement_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/literal_index_reference_copy_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/ordinary_select_provider_reference_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/qualified_reference_field_retarget_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_computed_backing_reference_copy_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_match_payload_parent_disjoint_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_reference_call_scalar_snapshot_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_selected_stored_carrier_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg`: 3 rows
- … and 294 more (see TSV)

## Confirmed holes by mutation kind

| Kind | Count |
| --- | ---: |
| `amp_to_ampmut` | 22 |
| `drop_requires` | 2 |
| `remove_mut` | 152 |

## Confirmed holes by accepted_by

| accepted_by | Count |
| --- | ---: |
| `both` | 3 |
| `current` | 173 |

## Kept rows

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — remove_mut

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
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

- Bucket: `confirmed-hole`
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

### tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_constructor_child_membership_snapshot_exit__main.omg__remove_mut__13883144ba2c718c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `44:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/calls/runtime_nested_constructor_child_snapshots_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_nested_constructor_child_snapshots_exit__main.omg__remove_mut__3dcc8afc0799b9d3/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `66:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/calls/runtime_selected_call_argument_from_fieldwise_local_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_selected_call_argument_from_fieldwise_local_exit__main.omg__amp_to_ampmut__f327f5c05a1a0c4f/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `3: // whole literal. A selected (`&mut&`-guarded) consuming call of one of its fields`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/cases/match_reference_local/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__match_reference_local__main.omg__amp_to_ampmut__81e62aff68db5139/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `16: machine Main::main(main: &mut Main) crashes Trap {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/cases/runtime_case_payload_policy_cast_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__runtime_case_payload_policy_cast_exit__main.omg__remove_mut__40657d915d1c6e49/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `50:     main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/collections/runtime_captured_structural_copy_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `matrix`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_captured_structural_copy_exit__main.omg__remove_mut__3352e43724ecaaff/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `35:     let observed: u32 in Wrapping = observe(&mut matrix, &mut cursor);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/collections/runtime_sequential_index_observations_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cursor`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_sequential_index_observations_exit__main.omg__remove_mut__75bed46b9d98a569/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `8:     cursor.index = 0;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/collections/runtime_stored_reference_array_replacement_contract_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `backing`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_stored_reference_array_replacement_contract_exit__main.omg__remove_mut__878c721939bb0ec9/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     let mut windows: [&mut [u8]; 1] = [&mut backing[..]];`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/control_flow/constant_aggregate_dispatch_complete/main.omg — drop_requires

- Bucket: `confirmed-hole`
- Mutation: delete first requires line
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__constant_aggregate_dispatch_complete__main.omg__drop_requires__1dc76d7a0fe9e0da/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `14:     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; } (arithmetic on `hi` formerly guarded by `(self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))`)`
- (a) Compile-time spec (`wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` § Nested machines and identity; Private entry inference):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `inferred-entry`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/control_flow/runtime_static_type_terminal_unit_cleanup_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `frame`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_static_type_terminal_unit_cleanup_exit__main.omg__remove_mut__cf8567b11d4d4cf6/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:         true -> first(effects, &mut frame)`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/control_flow/saved_case_predicate_disjoint_effects_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `marker`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__saved_case_predicate_disjoint_effects_exit__main.omg__remove_mut__e099ab9407eb97fb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     let observed: u64 = quotient(&choice, &mut marker);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/control_flow/shared_view_selected_disposition_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__shared_view_selected_disposition_exit__main.omg__remove_mut__63da36eeb828e99d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     holder.calls = ((holder.calls as u64 in Wrapping) + 1) as u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/core/numeric_widening_preserves_values/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__core__numeric_widening_preserves_values__main.omg__amp_to_ampmut__68a3e7c84c37a15e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23: // than an inline `&mut&`, so the checking machines stay straight-line code.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/dependent/closed_case_common_and_active_payload_projection/main.omg — drop_requires

- Bucket: `confirmed-hole`
- Mutation: delete first requires line
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__dependent__closed_case_common_and_active_payload_projection__main.omg__drop_requires__336fa8e7e0b609b1/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `16:     machine take(lo: u64, hi: u64, main: &mut Main) { main.width = hi - lo; } (arithmetic on `hi` formerly guarded by `((self in Interval::Point) && (self.lo == self.hi)) || (self in Interval::Other)`)`
- (a) Compile-time spec (`wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` § Nested machines and identity; Private entry inference):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `inferred-entry`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/dependent/data_requires_cyclic_handoff_walk_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `idle`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__dependent__data_requires_cyclic_handoff_walk_exit__main.omg__remove_mut__e9c283cd9374fafa/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `32:     drain(&mut idle);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/dependent/runtime_borrowed_coupled_last_store_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `envelope`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__dependent__runtime_borrowed_coupled_last_store_exit__main.omg__remove_mut__72827da96129ab9f/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `17:     replace(&mut envelope.item, 11);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/domains/carried_nested_payload_qualification/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `envelope`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__domains__carried_nested_payload_qualification__main.omg__remove_mut__1f7cbc8dd6f2cae9/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `12:     envelope.counter = 1;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/domains/domain_pattern_match_bound_ordered_effects_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `visits`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__domains__domain_pattern_match_bound_ordered_effects_exit__main.omg__remove_mut__7084f4d567b6fc96/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     visits = 1u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/domains/qualified_record_capture_after_mutation/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `value`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__domains__qualified_record_capture_after_mutation__main.omg__remove_mut__eeefd42efc6b1945/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `4:     value = 1;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/borrowed_projection_earlier_call_capture_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `seed`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__borrowed_projection_earlier_call_capture_exit__main.omg__remove_mut__4f20e45210d4cd70/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     seed.code = 0;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/result_cleanup_named_boolean_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `count`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__result_cleanup_named_boolean_exit__main.omg__remove_mut__c3f505d40a5e66bf/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `7:     count = ((count as u64 in Wrapping) + 1) as u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/runtime_cleanup_case_evidence_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `drops`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__runtime_cleanup_case_evidence_exit__main.omg__remove_mut__d91dd3ae5d42127e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `40:         before: 11, slot: make(which, Ticket { drops: &mut drops, value: 73 }), after: 19 } });`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/runtime_nested_empty_array_owner_cleanup_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__runtime_nested_empty_array_owner_cleanup_exit__main.omg__remove_mut__cc5b95fbc0b083e9/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `8:     holder.value = 0;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/runtime_owner_scoped_generic_cleanup_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__runtime_owner_scoped_generic_cleanup_exit__main.omg__remove_mut__ed378d41fe7fd200/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `25:         destination: &mut first, marker: [41, 42], extra: true,`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/drops/runtime_replaced_record_custody_transfer_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `pair`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__drops__runtime_replaced_record_custody_transfer_exit__main.omg__remove_mut__8464ec7dd72903ba/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `8:     pair.right = Token { value: 13 };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/expressions/runtime_match_structural_predicate_effect_order_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `tag_counter`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__expressions__runtime_match_structural_predicate_effect_order_exit__main.omg__remove_mut__f733506f8494a23e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `22:         Probe::Active { value: count(&mut tag_counter) } if allow(&mut tag_counter) -> 0`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/expressions/runtime_owned_match_multiple_projected_claims_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__expressions__runtime_owned_match_multiple_projected_claims_exit__main.omg__remove_mut__fb37a640536210f7/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `31:     let observed: bool = select(&mut output, choose);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/additional_input_domain_mutation_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cell`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__additional_input_domain_mutation_through_other_state_exit__main.omg__remove_mut__d6198f3d7eb17af7/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `6:     cell.value = 73;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/affine_discard_root_cleanup_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `dropped_true`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__affine_discard_root_cleanup_through_match_join_exit__main.omg__remove_mut__b8209565aad2ab57/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `34:     let result_true: u64 = exercise(&mut dropped_true, true);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/aggregate_storage_address_identity_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `value`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__aggregate_storage_address_identity_through_local_exit__main.omg__remove_mut__cd33bc027c100bb5/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `9:     value.left = 13;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/alias_fact_transfer_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `a`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__alias_fact_transfer_through_const_name_exit__main.omg__remove_mut__a64919ae7959f864/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `34:     Main::main(&mut a);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/atomic_bounded_fetch/match_join/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `path`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__atomic_bounded_fetch__match_join__main.omg__remove_mut__801670d01cf9c645/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `13:     path = 1u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/borrowed_projection_cleanup_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `direct_drops`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__borrowed_projection_cleanup_through_call_result_exit__main.omg__remove_mut__b97214308261d890/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `44:     let direct_code: u64 = capture(&CapturedReceipt { code: seed.code, counter: &mut direct_drops }).receipt.code;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/borrowed_restoration_window_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cell`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__borrowed_restoration_window_through_parameter_exit__main.omg__remove_mut__07bb73e33f4b6d3a/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     cell.value = 100u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/borrowed_restoration_window_through_parameter_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__borrowed_restoration_window_through_parameter_exit__main.omg__amp_to_ampmut__351c0e28b14dacf0/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: retiring a borrow restoration window restores full owner access and post-borrow invariant value >= 100u64 &mut& value <= 100u64 for exact subtraction value - 100u64.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/borrowed_store_call_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `subject`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__borrowed_store_call_through_other_state_exit__main.omg__remove_mut__2497b924cb959505/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `25:     forward(&mut subject, &second);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/boundary_provider_call_through_other_machine_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__boundary_provider_call_through_other_machine_exit__main.omg__amp_to_ampmut__e37aea917a49f090/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: a provider-settled boundary call whose contract establishes result >= 100u64 &mut& result <= 100u64 discharges exact subtraction result - 100u64.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/boundary_zero_origin_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `request`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__boundary_zero_origin_through_match_join_exit__main.omg__remove_mut__8ed83448f5068ed7/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `11:     let status: i32 = Io::acquire(&mut request);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/bounded_activation_computed_bound_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `live`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__bounded_activation_computed_bound_exit__main.omg__remove_mut__0ec1c3b459196a37/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `25:         let bytes: &mut [u8] = ActivationClaim::bytes(&mut live);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/bounded_activation_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `live`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__bounded_activation_through_local_exit__main.omg__remove_mut__c60c583fe55d7823/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `24:         let bytes: &mut [u8] = ActivationClaim::bytes(&mut live);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/build_activation_borrow/other_state/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `owned`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__build_activation_borrow__other_state__main.omg__remove_mut__ce651f22d92a5547/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:         apply(&mut owned)`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/c_layout_nested_representation_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__c_layout_nested_representation_through_match_join_exit__main.omg__remove_mut__13dd7332668edcd5/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:         entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/call_result_bound_discharge_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__call_result_bound_discharge_through_local_exit__main.omg__remove_mut__0e2398e6b9dbab88/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `36:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/captured_storage_bounds_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__captured_storage_bounds_through_const_name_exit__main.omg__remove_mut__d10e3c1f01c122bb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `51:     update(&mut first, true);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/captured_window_slot_retirement_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `original`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__captured_window_slot_retirement_through_call_result_exit__main.omg__remove_mut__7228b49f4a17f720/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `52:     let mut windows: [Window; 2] = [Window { values: &mut original[..] }, Window { values: &mut neighbor[..] }];`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/case_dependent_default_domain_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `current`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__case_dependent_default_domain_through_parameter_exit__main.omg__remove_mut__4bee6a9f0d9f2268/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `48:         Interval::Range { .. } -> both_widths(&mut current)`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/case_payload_stored_loan_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `backing`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__case_payload_stored_loan_through_local_exit__main.omg__remove_mut__6f01379bead232e7/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `27:     let loan: Loan = lend(&mut backing);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/case_where_equality_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__case_where_equality_through_call_result_exit__main.omg__remove_mut__32c15c39f5e13732/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `41:     run_main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/case_where_equality_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__case_where_equality_through_target_body_exit__main.omg__remove_mut__bd2b5c214d2f5637/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `42:     run_main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/checked_asm_nominal_predicate_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cursor`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__checked_asm_nominal_predicate_through_parameter_exit__main.omg__remove_mut__07f36b6d18426d7b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/checked_predicate_disjoint_loans_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `values`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__checked_predicate_disjoint_loans_through_local_exit__main.omg__remove_mut__acb1934b67bc7404/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `18:     replace(&mut values[..], selected, 1);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/component_scalar_lending_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `value`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__component_scalar_lending_through_call_result_exit__main.omg__remove_mut__a7dd694292cedaa5/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     let reported: u64 = suspend Lending::write(&mut value, &token, original, member);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/component_scalar_lending_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `value`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__component_scalar_lending_through_target_body_exit__main.omg__remove_mut__098537a652870fef/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `13:     let reported: u64 = suspend Lending::write(&mut value, &token, original, member);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/compound_index_guard_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__compound_index_guard_through_other_state_exit__main.omg__remove_mut__24bb6f6894cac5ed/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `58:     run_main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/computed_nested_state_loan_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `rows`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__computed_nested_state_loan_through_local_exit__main.omg__remove_mut__e23341008ccdb42d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `31:     transition { _ -> apply(&mut rows[choose_row(cursor, selected_row)].data[choose_column(cursor, selected_column)], replacement(cursor)) }`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/computed_record_window_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__computed_record_window_through_field_exit__main.omg__remove_mut__1c337da79bb32f16/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `13:         0 -> &mut holder.samples[0..holder.which + 2]`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/copied_call_argument_snapshot_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `source`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__copied_call_argument_snapshot_through_match_join_exit__main.omg__remove_mut__2fc31179b72ce3f2/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `26:     let observed: u64 = receive(source, overwrite(&mut source));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/copied_case_before_later_argument_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__copied_case_before_later_argument_through_local_exit__main.omg__remove_mut__b486a8d7fe7d7bc3/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     let observed: u64 = read_payload(chosen, set_to_absent(&mut chosen));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/correlated_invariant_store_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `interior`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__correlated_invariant_store_through_call_result_exit__main.omg__remove_mut__29454044adfab08d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `18:     advance(&mut interior, 40);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/correlated_invariant_store_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `interior`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__correlated_invariant_store_through_target_body_exit__main.omg__remove_mut__770cacc954ac068a/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `54:     SelectedAdvance::advance(&mut interior, 40);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/data_where_standing_mutable_recast_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__data_where_standing_mutable_recast_through_field_exit__main.omg__remove_mut__225a162aabd99316/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `16:     replace(&mut storage.value, replacement);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/default_domain_field_write_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `span`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__default_domain_field_write_through_const_name_exit__main.omg__remove_mut__79c5f1bc24c5d1d3/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     span.hi = NEXT;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/dependent_subtract_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `a`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__dependent_subtract_through_other_state_exit__main.omg__remove_mut__8348ed845c0f0818/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `20:     Main::main(&mut a);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__descriptor_payload_exclusive_loan_through_other_machine_exit__main.omg__remove_mut__1bb1e9a3b00ec71f/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `17:         true -> &mut chosen.value[..]`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/descriptor_range_exclusive_loan_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__descriptor_range_exclusive_loan_through_match_join_exit__main.omg__remove_mut__992611e2dd0cb678/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `24:         true -> &mut chosen.value[0..2]`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/disjoint_field_borrow_windows_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__disjoint_field_borrow_windows_through_field_exit__main.omg__remove_mut__7168fef90ee0ae43/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     touch_left(&mut holder.pair.left);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/disjoint_windows_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__disjoint_windows_through_const_name_exit__main.omg__remove_mut__be12a722ffd2c5ca/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `17:     let written: bool = exercise(&mut entries[..]);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/domain_selected_cleanup_computed_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__domain_selected_cleanup_computed_match_join_exit__main.omg__remove_mut__d43572d830587f93/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `36:     exercise(&mut first, true, 11);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/domain_selected_cleanup_erased_predicates_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__domain_selected_cleanup_erased_predicates_exit__main.omg__remove_mut__98c40e54fe4fd93d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `21:     let keeper: Keeper in Held = Keeper { output: &mut output, code: 7 };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/domain_selected_cleanup_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__domain_selected_cleanup_through_match_join_exit__main.omg__remove_mut__a3aaca3232870393/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     consume(select(true, Keeper { output: &mut output, code: 7 }));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/double_indexed_rmw_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `grid`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__double_indexed_rmw_through_local_exit__main.omg__remove_mut__b8ab6e597cfd9189/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `6:     transition { _ -> grid_rmw(&mut grid, 1, 2) }`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/dynamic_trait_dispatch_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first_observation`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__dynamic_trait_dispatch_through_other_machine_exit__main.omg__remove_mut__08f04f21d977711b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `56:     let first: u64 = probe(first_selector, 60u64, 40u64, &mut first_observation);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/exact_sub_guard_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__exact_sub_guard_through_field_exit__main.omg__remove_mut__1581154cd8d2b55b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `40:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/exclusive_observation_exposure_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `items`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__exclusive_observation_exposure_through_call_result_exit__main.omg__remove_mut__1214eff899bcf83d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     let held: u64 = expose(&mut items[0]);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/exclusive_observation_exposure_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `items`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__exclusive_observation_exposure_through_target_body_exit__main.omg__remove_mut__9b78f5fb1091669a/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `44:         let held: u64 = expose(&mut items[0]);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/field_where_index_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__field_where_index_through_parameter_exit__main.omg__remove_mut__e71730720d52dade/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `46:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/finite_family_exclusive_result/other_machine/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `counter`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__finite_family_exclusive_result__other_machine__main.omg__remove_mut__17ff25fc0f8efe3e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `62:     helper(&mut counter);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/finite_family_input_index/match_join/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `limit`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__finite_family_input_index__match_join__main.omg__remove_mut__2f3a96de76f6a724/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `42:     let item: Indexed<saved> = Family::row<width>(selected, input(&mut limit, observations), limit, observations);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/fixed_layout_bit_observation_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__fixed_layout_bit_observation_through_field_exit__main.omg__remove_mut__8cd0dde93dc63ac6/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__fixed_layout_byte_roundtrip_through_const_name_exit__main.omg__remove_mut__44ca91179bed8deb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:         entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/fixed_layout_storage_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__fixed_layout_storage_through_match_join_exit__main.omg__remove_mut__3c0f1b4060dd6917/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `9:         entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 8 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/fixed_region_byte_roundtrip_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `destination`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__fixed_region_byte_roundtrip_through_local_exit__main.omg__remove_mut__429694099da1c614/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `4:     destination = source;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_nested_record_backing_match_join/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `buffer`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_nested_record_backing_match_join__main.omg__remove_mut__6a5ce98bd1ee1446/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `43:     let pending: Pending in Pending::Retained = Writer::register(capacity, &mut buffer);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_reference_result_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `left`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_reference_result_through_const_name_exit__main.omg__remove_mut__e5bb48741e52b0f5/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `24:  let first: bool = journey(&mut left, true);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_call_result/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `buffer`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_retained_mutable_borrow_call_result__main.omg__remove_mut__53849f66332b7eac/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `40:     let pending: Pending = Writer::register(capacity, &mut buffer);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_parameter/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `buffer`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_retained_mutable_borrow_parameter__main.omg__remove_mut__52ddc65fc89daa1f/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `40:     let pending: Pending = Writer::register(capacity, &mut buffer);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_retained_shared_borrow_local/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `observed`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_retained_shared_borrow_local__main.omg__remove_mut__c1bf859d2ca517d1/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `38:     let returned: Capacity in Capacity::Available = Reader::complete(pending, &mut observed);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/foreign_retained_shared_borrow_raw_same_carrier_called/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `observed`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__foreign_retained_shared_borrow_raw_same_carrier_called__main.omg__remove_mut__e48b1ef34a243271/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `44:     let returned: Capacity in Capacity::Available = Reader::complete(pending, &mut observed);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/free_entry_local_state_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `tally`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__free_entry_local_state_through_const_name_exit__main.omg__remove_mut__2fdd5bddd12f79fb/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `13:     settle(&Mark { slot: &mut tally, code: code });`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/generic_requirement_statement_call_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__generic_requirement_statement_call_through_parameter_exit__main.omg__remove_mut__e3fb9cac8b517251/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     notify<i32>(&subject, 9, &mut first);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/guarded_call_entry_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `context`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__guarded_call_entry_through_other_machine_exit__main.omg__remove_mut__3c33595041e1c7ee/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `14:     Plan(&mut context);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/guarded_line_loan_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `line`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__guarded_line_loan_through_match_join_exit__main.omg__remove_mut__3884fbf094f4ca5c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `11:     line[0] = 37;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/guarded_record_line_loan_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `line`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__guarded_record_line_loan_through_local_exit__main.omg__remove_mut__efbfa4b629f58edc/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     line[0] = 37;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_field_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__ieee_float_32bit_ordered_bound_through_field_exit__main.omg__amp_to_ampmut__58dc9f7dadee298d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: a 32-bit IEEE f32 ordered comparison guard (reading >= 1.25f32 &mut& reading <= 2.75f32) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring reading >= 1.0f32 && reading <= 3.0f32 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/ieee_float_ordered_comparison_through_const_name_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__ieee_float_ordered_comparison_through_const_name_exit__main.omg__amp_to_ampmut__083782e5bfdb0de1/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: an IEEE f64 ordered comparison guard (reading >= 1.5 &mut& reading <= 2.5) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring reading >= 1.0 && reading <= 3.0 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/in_place_view_argument_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `packet`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__in_place_view_argument_through_parameter_exit__main.omg__remove_mut__93f62bfd3b01c1ec/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `19:     packet[0] = 11;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/indexed_recoverable_replacement_window_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `holder`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__indexed_recoverable_replacement_window_through_target_body_exit__main.omg__remove_mut__bbbfe6b55455ac76/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `58:         let first: Outcome = attempt(alias, succeed, observe(&mut holder.sibling));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/indexed_storage_address_identity_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `items`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__indexed_storage_address_identity_through_parameter_exit__main.omg__remove_mut__81b9e39ab2581aaa/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     observe(&mut items, input)`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/integer_width_truncation_mask_bound_through_other_machine_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__integer_width_truncation_mask_bound_through_other_machine_exit__main.omg__amp_to_ampmut__41684a62d4639b8d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: masking an unsigned 64-bit integer to its low byte (val &mut 255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/invariant_window_snapshot_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `first`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__invariant_window_snapshot_through_match_join_exit__main.omg__remove_mut__1c10ef8e0179a8ea/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `40:     let first_result: i32 = inspect(&mut first, -1, 3, true);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/linear_owner_borrowed_projection_call_result/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cells`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__linear_owner_borrowed_projection_call_result__main.omg__remove_mut__379ce58fdf094cb3/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `30:     let mut buffer: Buffer = make_buffer(&mut cells, &mut cleanup_slots, expected_route);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/linear_owner_borrowed_projection_target_body/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cells`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__linear_owner_borrowed_projection_target_body__main.omg__remove_mut__23c6a4157fa9d1d6/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `27:     let mut buffer: Buffer = Buffer { marker: 91, route: expected_route, cells: &mut cells, cleanup: &mut cleanup_slots };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_computed_root_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_computed_root_through_field_exit__main.omg__remove_mut__a2c65857ac8c1803/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `21:         17 -> &mut chosen[0]`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_effectful_subject_custody_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `kept`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_effectful_subject_custody_through_const_name_exit__main.omg__remove_mut__47876c782699ff18/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     kept.mark = 40;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_join_multi_variant_bound_through_call_result_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_join_multi_variant_bound_through_call_result_exit__main.omg__amp_to_ampmut__549c045761da16d5/main.omg`
- Current: `accepted`; route: `rejected`; accepted_by: `current`
- Route detail (hint only): diagnostic: reject main.omg:25 bytes 1016..1017 exact arithmetic in machine `run` state `entry` local `diff1` may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation) (no proof found from the facts available here: the operands' bounds exceed 
- Broken obligation: `1: // Exhaustively matching a three-variant case value where every arm establishes joined >= 20u64 &mut& joined <= 60u64 preserves the joined interval bound across all nine places, discharging exact subtraction 60u64 - joined and joined - 20u64.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `rejected`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_join_multi_variant_bound_through_target_body_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_join_multi_variant_bound_through_target_body_exit__main.omg__amp_to_ampmut__87b0fc451819b81b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Exhaustively matching a three-variant case value where every arm establishes joined >= 20u64 &mut& joined <= 60u64 preserves the joined interval bound across all nine places, discharging exact subtraction 60u64 - joined and joined - 20u64.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_loan_last_use_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_loan_last_use_through_other_state_exit__main.omg__remove_mut__5985f87bd18a7630/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `22:             true -> &mut chosen.value[0..2]`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/match_runtime_view_extent_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `pair`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__match_runtime_view_extent_through_field_exit__main.omg__remove_mut__c431136142ee4387/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `18:         true -> &mut pair.chosen`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/matched_field_windows_through_const_name_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `bytes`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__matched_field_windows_through_const_name_exit__main.omg__remove_mut__7f3246260360ab07/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `26:     let window: &mut [u8] = from_const(&mut bytes, 0);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_choice_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_choice_through_call_result_exit__main.omg__remove_mut__551e459a752193a0/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `31:     let marker: u64 in Wrapping = add(overwrite(choose(&mut chosen, &mut other, true)), zero);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_choice_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_choice_through_target_body_exit__main.omg__remove_mut__3a5472f280a69b2b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `80:     let found: u8 = SelectedProbe::probe(&mut chosen, &mut other, &items);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_codec_argument_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `buffer`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_codec_argument_through_parameter_exit__main.omg__remove_mut__e2b849554e907c51/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `22:     Blob::encode(&sample, &mut buffer, choose(chosen, other, true));`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_codec_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_codec_through_other_state_exit__main.omg__remove_mut__b848ff2554c36b61/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `18:     let counter: Counter = Counter { slot: &mut chosen };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_computed_receiver_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_computed_receiver_through_other_machine_exit__main.omg__remove_mut__e293a95b74b1bfbe/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `11:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_named_state_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_named_state_through_match_join_exit__main.omg__remove_mut__3f834e67939988a6/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `51:     let marker: u64 in Wrapping = add(overwrite(relay(&mut chosen, &mut other, true)), zero);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_nested_helper_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `chosen`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_nested_helper_through_local_exit__main.omg__remove_mut__4de7c8c9da3fa933/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `31:     let marker: u64 in Wrapping = add(overwrite(relay(&mut chosen, &mut other, true)), zero);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/may_write_stored_rebind_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `bounds`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__may_write_stored_rebind_through_field_exit__main.omg__remove_mut__57880cc0b36f13db/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `28:     referent = choose(&mut bounds.chosen, &mut bounds.other, true);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/method_array_operand_inference_through_const_name_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__method_array_operand_inference_through_const_name_exit__main.omg__amp_to_ampmut__644530b9393cd5cc/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: with Count supplied as 3, the omitted element T of Bytes::last is inferred as u8 from the actual &mut[u8; 3], and the selected implementation's [T; Count] carrier is the caller's copyable [u8; 3].`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `backing`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__mutable_slice_element_write_bound_through_other_machine_exit__main.omg__remove_mut__419704cb6ed7bd34/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     &mut backing.items`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_field_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_narrow_truncation_64to8_mask_bound_through_field_exit__main.omg__amp_to_ampmut__705cba2116dd822e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut 31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_call_result_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_signed_16to64_extension_through_call_result_exit__main.omg__amp_to_ampmut__2c02829d1b1c722e/main.omg`
- Current: `accepted`; route: `rejected`; accepted_by: `current`
- Route detail (hint only): diagnostic: reject main.omg:20 bytes 962..973 cannot prove requires contract for call verify_wide from ext_call_result: wide >= 300 && wide <= 500 (needs fact `wide >= 300 && wide <= 500` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`
- Broken obligation: `1: // Fact: sign-extending a bounded 16-bit signed i16 value (narrow >= 300i16 &mut& narrow <= 500i16) to i64 (narrow as i64) preserves the exact interval (wide >= 300i64 && wide <= 500i64), discharging exact 64-bit signed subtractions 500i64 - wide and wide - 300i64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `rejected`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_target_body_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_signed_16to64_extension_through_target_body_exit__main.omg__amp_to_ampmut__1df9ccba5af1b5a7/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: sign-extending a bounded 16-bit signed i16 value (narrow >= 300i16 &mut& narrow <= 500i16) to i64 (narrow as i64) preserves the exact interval (wide >= 300i64 && wide <= 500i64), discharging exact 64-bit signed subtractions 500i64 - wide and wide - 300i64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_parameter_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_signed_8to64_extension_through_parameter_exit__main.omg__amp_to_ampmut__164224e1a2bcf8e5/main.omg`
- Current: `accepted`; route: `accepted`; accepted_by: `both`
- Route detail (hint only): route=accepted
- Broken obligation: `1: // Fact: sign-extending a bounded 8-bit signed i8 value (narrow >= 30i8 &mut& narrow <= 100i8) to i64 (narrow as i64) preserves the exact interval (wide >= 30i64 && wide <= 100i64), discharging exact 64-bit signed subtractions 100i64 - wide and wide - 30i64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `accepted`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_unsigned_16to64_extension_through_other_state_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_unsigned_16to64_extension_through_other_state_exit__main.omg__amp_to_ampmut__455ff522efc1a806/main.omg`
- Current: `accepted`; route: `accepted`; accepted_by: `both`
- Route detail (hint only): route=accepted
- Broken obligation: `1: // Fact: zero-extending a bounded 16-bit unsigned u16 value (narrow >= 300u16 &mut& narrow <= 500u16) to u64 (narrow as u64) preserves the exact interval (wide >= 300u64 && wide <= 500u64), discharging exact 64-bit subtractions 500u64 - wide and wide - 300u64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `accepted`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_unsigned_32to64_extension_through_other_machine_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_unsigned_32to64_extension_through_other_machine_exit__main.omg__amp_to_ampmut__75c21b76e80e34f2/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `1: // Fact: zero-extending a bounded 32-bit unsigned u32 value (narrow >= 70000u32 &mut& narrow <= 90000u32) to u64 (narrow as u64) preserves the exact interval (wide >= 70000u64 && wide <= 90000u64), discharging exact 64-bit subtractions 90000u64 - wide and wide - 70000u64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrow_to_wide_unsigned_8to64_extension_through_match_join_exit/main.omg — amp_to_ampmut

- Bucket: `confirmed-hole`
- Mutation: first shared & -> &mut
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrow_to_wide_unsigned_8to64_extension_through_match_join_exit__main.omg__amp_to_ampmut__1f6be21b582cf90f/main.omg`
- Current: `accepted`; route: `accepted`; accepted_by: `both`
- Route detail (hint only): route=accepted
- Broken obligation: `1: // Fact: zero-extending a bounded 8-bit unsigned u8 value (narrow >= 50u8 &mut& narrow <= 180u8) to u64 (narrow as u64) preserves the exact interval (wide >= 50u64 && wide <= 180u64), discharging exact 64-bit subtractions 180u64 - wide and wide - 50u64 across all nine places.`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; forming `&mut` without writable authority violates exclusivity and mutability rules. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `accepted`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/narrowed_integer_write_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__narrowed_integer_write_through_field_exit__main.omg__remove_mut__273b4da4a5640673/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/nested_borrow_qualification_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `words`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__nested_borrow_qualification_through_parameter_exit__main.omg__remove_mut__62a8a7fd737a2395/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `28:     words[0] = 11;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/nested_result_extent_through_parameter_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `bytes`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__nested_result_extent_through_parameter_exit__main.omg__remove_mut__7ce1ebf24effadff/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `21:     let first: u64 = trial(&mut bytes[..], &other[..], 13);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/nominal_reference_cleanup_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__nominal_reference_cleanup_through_other_state_exit__main.omg__remove_mut__25d066f03ed5dc37/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `21:     observe(&Keeper { output: &mut output, code: seed.code });`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/placed_stable_write_through_call_result_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__placed_stable_write_through_call_result_exit__main.omg__remove_mut__8d2cbf672f0d6e54/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/placed_stable_write_through_target_body_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__placed_stable_write_through_target_body_exit__main.omg__remove_mut__69fb746a2d9c8663/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `10:     entries[0] = FieldEntry {`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/predicate_array_copy_through_other_state_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `values`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__predicate_array_copy_through_other_state_exit__main.omg__remove_mut__7d7dc7b281d4670b/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `5:     values[0] = 37;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/predicate_type_formation_through_other_machine_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `formed`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__predicate_type_formation_through_other_machine_exit__main.omg__remove_mut__9bcdac7ba967ad5a/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `24:     fill_true(&mut formed);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/progress_selector_nested_arrival_through_match_join_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `calls`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__progress_selector_nested_arrival_through_match_join_exit__main.omg__remove_mut__ecf400fa6f4634a6/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     calls = ((calls as u64 in Wrapping) + 1) as u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/progress_selector_owned_arrival_through_local_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `calls`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__progress_selector_owned_arrival_through_local_exit__main.omg__remove_mut__e9f7d6f53284ac84/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     calls = ((calls as u64 in Wrapping) + 1) as u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/facts/progress_selector_through_field_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `calls`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__facts__progress_selector_through_field_exit__main.omg__remove_mut__78776d7cd7004573/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `15:     calls = ((calls as u64 in Wrapping) + 1) as u64;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.


## Hand verification (5 confirmed-hole)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / remove_mut

- Bucket: `confirmed-hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1401057	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=2959717	root=compilation	route=outside	Unsupported: type data with custody

```

#### Hand verification: tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3649673	root=compilation	route=outside	Unsupported: type reference

```

#### Hand verification: tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg	current=accepted	current_us=2943009	root=compilation	route=outside	Unsupported: contract term other than a formal or literal

```

#### Hand verification: tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg	current=accepted	current_us=2977486	root=compilation	route=outside	Unsupported: type reference

```


## Hand verification (5 discarded-valid)

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg / drop_requires

- Bucket: `discarded-valid`
- Obligation: `8:         true -> decimal(rest - 10, count + 1) (arithmetic on `count` formerly guarded by `count <= 9 && embed(rest) + embed(count) * 10 <= 99`)`

**Mutant diff (excerpt)**
```diff
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     transition rest >= 10 {
-     transition rest >= 10 {
+         true -> decimal(rest - 10, count + 1)
-         true -> decimal(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine next(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine next(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ { count }
- { count }
+ 
- 
+ machine local(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine local(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let selected: bool = rest >= 10;
-     let selected: bool = rest >= 10;
+     let copied: bool = selected;
-     let copied: bool = selected;
+     let combined: bool = copied && copied;
-     let combined: bool = copied && copied;
+     transition combined {
-     transition combined {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine replaced(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine replaced(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let mut condition: Condition = Condition { selected: false };
-     let mut condition: Condition = Condition { selected: false };
+     condition.selected = rest >= 10;
-     condition.selected = rest >= 10;
+     transition condition.selected {
-     transition condition.selected {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine field(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine field(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let condition: Condition = Condition { selected: rest >= 10 };
-     let condition: Condition = Condition { selected: rest >= 10 };
+     let copied: Condition = condition;
-     let copied: Condition = condition;
+     transition copied.selected {
-     transition copied.selected {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine denied(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine denied(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let below: bool = rest < 10;
-     let below: bool = rest < 10;
+     transition below {
-     transition below {
+         false -> next(rest - 10, count + 1)
-         false -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine historical(mut rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine historical(mut rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let observed: u64 in Trapping = rest;
-     let observed: u64 in Trapping = rest;
+     let selected: bool = rest >= 10;
-     let selected: bool = rest >= 10;
+     rest = 0;
-     rest = 0;
+     transition selected {
-     transition selected {
+         true -> next(observed - 10, count + 1)
-         true -> next(observed - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine main() crashes Trap {
- machine main() crashes Trap {
+     let digits: u64 in Trapping = decimal(99, 0);
-     let digits: u64 in Trapping = decimal(99, 0);
+     let zero: u64 in Trapping = decimal(0, 0);
-     let zero: u64 in Trapping = decimal(0, 0);
+     let one: u64 in Trapping = decimal(10, 0);
-     let one: u64 in Trapping = decimal(10, 0);
+     let from_local: u64 in Trapping = local(49, 5);
-     let from_local: u64 in Trapping = local(49, 5);
+     let from_field: u64 in Trapping = field(29, 7);
-     let from_field: u64 in Trapping = field(29, 7);
+     let from_denial: u64 in Trapping = denied(19, 8);
-     let from_denial: u64 in Trapping = denied(19, 8);
+     let from_history: u64 in Trapping = historical(39, 6);
-     let from_history: u64 in Trapping = historical(39, 6);
+     let from_replacement: u64 in Trapping = replaced(59, 4);
-     let from_replacement: u64 in Trapping = replaced(59, 4);
+     transition digits == 9 && zero == 0 && one == 1
-     transition digits == 9 && zero == 0 && one == 1
+         && from_local == 6 && from_field == 8 && from_denial == 9
-         && from_local == 6 && from_field == 8 && from_denial == 9
+         && from_history == 7 && from_replacement == 5 {
-         && from_history == 7 && from_replacement == 5 {
+         true -> done()
-         true -> done()
+         _ -> wrong()
-         _ -> wrong()
+     }
-     }
+     machine done() {}
-     machine done() {}
+     machine wrong() { crash Trap; }
-     machine wrong() { crash Trap; }
+ }
- }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg	current=accepted	current_us=2944038	root=compilation	route=outside	Unsupported: type constrained

```

#### Hand verification: tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg / weaken_requires

- Bucket: `discarded-valid`
- Obligation: `11:     view[0] = 0; (index uses `view` formerly guarded by `view.len >= 1`)`

**Mutant diff (excerpt)**
```diff
- requires view.len >= 1
+ requires view.len > 1
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__weaken_requires__1f65f214778af6b4/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__weaken_requires__1f65f214778af6b4/main.omg	current=accepted	current_us=2878262	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

#### Hand verification: tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg / drop_requires

- Bucket: `discarded-valid`
- Obligation: `8:     wrapper.bundle.windows[0].len == 1 && wrapper.bundle.windows[0][0] == 19 (index uses `windows` formerly guarded by `wrapper.bundle.windows[0].len == 1`)`

**Mutant diff (excerpt)**
```diff
- requires wrapper.bundle.windows[0].len == 1
+ {
- {
+     wrapper.bundle.windows[0].len == 1 && wrapper.bundle.windows[0][0] == 19
-     wrapper.bundle.windows[0].len == 1 && wrapper.bundle.windows[0][0] == 19
+ }
- }
+ data Container<'bundle, 'storage> { wrappers: [Wrapper<'bundle, 'storage>; 1]; }
- data Container<'bundle, 'storage> { wrappers: [Wrapper<'bundle, 'storage>; 1]; }
+ machine relay<'bundle, 'storage>(container: Container<'bundle, 'storage>) -> Container<'bundle, 'storage>
- machine relay<'bundle, 'storage>(container: Container<'bundle, 'storage>) -> Container<'bundle, 'storage>
+ ensures result.wrappers[0].bundle.windows[0].len == container.wrappers[0].bundle.windows[0].len
- ensures result.wrappers[0].bundle.windows[0].len == container.wrappers[0].bundle.windows[0].len
+ { container }
- { container }
+ machine main() crashes Trap {
- machine main() crashes Trap {
+     let mut backing: [u8; 1] = [19];
-     let mut backing: [u8; 1] = [19];
+     let bundle: Bundle = Bundle { windows: [&mut backing[..]] };
-     let bundle: Bundle = Bundle { windows: [&mut backing[..]] };
+     let wrapper: Wrapper = Wrapper { bundle: &bundle };
-     let wrapper: Wrapper = Wrapper { bundle: &bundle };
+     let stored: Container = Container { wrappers: [wrapper] };
-     let stored: Container = Container { wrappers: [wrapper] };
+     let selected: Container = relay(stored);
-     let selected: Container = relay(stored);
+     let observed: bool = observe(&selected.wrappers[0]);
-     let observed: bool = observe(&selected.wrappers[0]);
+     transition observed { true -> done() false -> bad() }
-     transition observed { true -> done() false -> bad() }
+     machine done() {}
-     machine done() {}
+     machine bad() crashes Trap { crash Trap; }
-     machine bad() crashes Trap { crash Trap; }
+ }
- }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__drop_requires__69dc4db1d8501575/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__drop_requires__69dc4db1d8501575/main.omg	current=accepted	current_us=3012763	root=compilation	route=outside	Unsupported: type reference

```

#### Hand verification: tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg / weaken_requires

- Bucket: `discarded-valid`
- Obligation: `15:     view[0] = 0; (index uses `view` formerly guarded by `view.len >= 1`)`

**Mutant diff (excerpt)**
```diff
- requires view.len >= 1
+ requires view.len > 1
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__weaken_requires__3398e68f2ec52a15/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__weaken_requires__3398e68f2ec52a15/main.omg	current=accepted	current_us=3213764	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

#### Hand verification: tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg / drop_requires

- Bucket: `discarded-valid`
- Obligation: `63:         let next: u32 = off + stride; (arithmetic on `off` formerly guarded by `off <= 32` contract;`)`

**Mutant diff (excerpt)**
```diff
- //     forwarded offset into each nested machine's `requires off <= 32` contract;
+ //     forwarded offset into each nested machine's `//   - the stride: the guard `0 < stride <= 16` on the stored descriptor size
- //   - the stride: the guard `0 < stride <= 16` on the stored descriptor size
+ //     bounds `off + stride`;
- //     bounds `off + stride`;
+ //   - the view footprint: `high(off) + size_of(Desc) <= len(buf)` through
- //   - the view footprint: `high(off) + size_of(Desc) <= len(buf)` through
+ //     the walk's current requirements and incoming edges give the offset -- 0
- //     the walk's current requirements and incoming edges give the offset -- 0
+ //     and the `next <= 32` guard;
- //     and the `next <= 32` guard;
+ //   - no runtime bounds checks, no linear-counter workaround.
- //   - no runtime bounds checks, no linear-counter workaround.
+ // Three descriptors at offsets 0/16/32 carry kind tags 3, 4, 5 and page
- // Three descriptors at offsets 0/16/32 carry kind tags 3, 4, 5 and page
+ // counts 10, 20, 30; the walk sums the page counts of kind >= 4 entries:
- // counts 10, 20, 30; the walk sums the page counts of kind >= 4 entries:
+ // 20 + 30 = 50.
- // 20 + 30 = 50.
+ //
- //
+ // Expected exit: 50
- // Expected exit: 50
+ use omega::language::core::numeric_conversion;
- use omega::language::core::numeric_conversion;
+ data Desc { kind: u32; pages: u64; }
- data Desc { kind: u32; pages: u64; }
+ data Main {
- data Main {
+     buf: [u8; 48];
-     buf: [u8; 48];
+     descriptor_size: u32;
-     descriptor_size: u32;
+     total: u32 in Wrapping;
-     total: u32 in Wrapping;
+ }
- }
+ machine walk_descriptors(main: &mut Main) crashes Trap {
- machine walk_descriptors(main: &mut Main) crashes Trap {
+     // Descriptor 0: kind 3, pages 10.
-     // Descriptor 0: kind 3, pages 10.
+     main.buf[0] = 3;
-     main.buf[0] = 3;
+     main.buf[8] = 10;
-     main.buf[8] = 10;
+     // Descriptor 1: kind 4, pages 20.
-     // Descriptor 1: kind 4, pages 20.
+     main.buf[16] = 4;
-     main.buf[16] = 4;
+     main.buf[24] = 20;
-     main.buf[24] = 20;
+     // Descriptor 2: kind 5, pages 30.
-     // Descriptor 2: kind 5, pages 30.
+     main.buf[32] = 5;
-     main.buf[32] = 5;
+     main.buf[40] = 30;
-     main.buf[40] = 30;
+     main.descriptor_size = 16;
-     main.descriptor_size = 16;
+     transition { _ -> walk(main, 0) }
-     transition { _ -> walk(main, 0) }
+ 
- 
+     // The view is taken once per entry. Current arrival requirements and the
-     // The view is taken once per entry. Current arrival requirements and the
+     // guarded back edge establish its footprint over the original buffer.
-     // guarded back edge establish its footprint over the original buffer.
+     machine walk(main: &mut Main, off: u32) requires off <= 32 crashes Trap {
-     machine walk(main: &mut Main, off: u32) requires off <= 32 crashes Trap {
+         let d: &Desc = &main.buf[off] as &Desc;
-         let d: &Desc = &main.buf[off] as &Desc;
+         let pages: u64 = d.pages;
-         let pages: u64 = d.pages;
+         transition d.kind >= 4 {
-         transition d.kind >= 4 {
+             true -> tally(main, off, pages)
-             true -> tally(main, off, pages)
+             _ -> step(main, off)
-             _ -> step(main, off)
+         }
-         }
+     }
-     }
+     machine tally(main: &mut Main, off: u32, pages: u64) requires off <= 32 crashes Trap {
-     machine tally(main: &mut Main, off: u32, pages: u64) requires off <= 32 crashes Trap {
+         let counted: u32 in Wrapping =
-         let counted: u32 in Wrapping =
+             narrow_u64_to_u32_wrapping(pages) as u32 in Wrapping;
-             narrow_u64_to_u32_wrapping(pages) as u32 in Wrapping;
+         main.total = main.total + counted;
-         main.total = main.total + counted;
+         transition { _ -> step(main, off) }
-         transition { _ -> step(main, off) }
+     }
-     }
+     machine step(main: &mut Main, off: u32) requires off <= 32 crashes Trap {
-     machine step(main: &mut Main, off: u32) requires off <= 32 crashes Trap {
+         let stride: u32 = main.descriptor_size;
-         let stride: u32 = main.descriptor_size;
+         transition stride > 0 && stride <= 16 {
-         transition stride > 0 && stride <= 16 {
+             true -> advance(main, off, stride)
-             true -> advance(main, off, stride)
+             _ -> done(main)
-             _ -> done(main)
+         }
-         }
+     }
-     }
+     machine advance(main: &mut Main, off: u32, stride: u32) requires off <= 32 requires stride <= 16 crashes Trap {
-     machine advance(main: &mut Main, off: u32, stride: u32) requires off <= 32 requires stride <= 16 crashes Trap {
+         let next: u32 = off + stride;
-         let next: u32 = off + stride;
+         transition next <= 32 {
-         transition next <= 32 {
+             true -> walk(main, next)
-             true -> walk(main, next)
+             _ -> done(main)
-             _ -> done(main)
+         }
-         }
+     }
-     }
+     machine done(main: &mut Main) crashes Trap {
-     machine done(main: &mut Main) crashes Trap {
+         transition main.total == 50 { true -> good() _ -> bad() }
-         transition main.total == 50 { true -> good() _ -> bad() }
+     }
-     }
+ }
- }
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() crashes Trap {
- machine entry_start() crashes Trap {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     walk_descriptors(&mut entry_storage);
-     walk_descriptors(&mut entry_storage);
+ }
- }
+ 
- 
+ machine good() {}
- machine good() {}
+ machine bad() crashes Trap { crash Trap; }
- machine bad() crashes Trap { crash Trap; }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__drop_requires__885ac42f221b8727/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__drop_requires__885ac42f221b8727/main.omg	current=accepted	current_us=3434822	root=compilation	route=outside	Unsupported: parameter mutable parameter

```


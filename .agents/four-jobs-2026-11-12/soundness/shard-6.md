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
| Originals skipped | 92 |
| Fixtures checked (original accepted) | 1038 |
| Fixtures with zero generated mutants (no-mutation) | 24 |
| Fixtures with ≥1 mutant row | 109 |
| Mutant rows | 227 |
| Mean mutants per mutated fixture | 2.08 |

Reconcile: `fixtures in shard` = skipped + checked; `checked` = no-mutation + mutated fixtures; mutant row buckets sum to `mutant rows`.

## Mutant buckets (sum = mutants attempted)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 51 |
| `known-gap-q7-mut` | 0 |
| `disagreement-valid` | 1 |
| `valid-route-outside` | 31 |
| `discarded-valid` | 16 |
| `rejected-both` | 128 |
| `timeout` | 0 |
| `failed-run` | 0 |
| `failed-run` | 0 |
| **Total** | **227** |

## Fixtures with more than one mutant row

Count: 75 fixtures.
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
- … and 35 more (see TSV)

## Confirmed holes by mutation kind

| Kind | Count |
| --- | ---: |
| `amp_to_ampmut` | 6 |
| `drop_requires` | 1 |
| `remove_mut` | 44 |

## Confirmed holes by accepted_by

| accepted_by | Count |
| --- | ---: |
| `current` | 51 |

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1393779	root=compilation	route=no-entry

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=2934817	root=compilation	route=outside	Unsupported: type data with custody

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3472749	root=compilation	route=outside	Unsupported: type reference

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg	current=accepted	current_us=2930978	root=compilation	route=outside	Unsupported: contract term other than a formal or literal

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg	current=accepted	current_us=2971584	root=compilation	route=outside	Unsupported: type reference

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg	current=accepted	current_us=3031031	root=compilation	route=outside	Unsupported: type constrained

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__weaken_requires__1f65f214778af6b4/main.omg	current=accepted	current_us=2915143	root=compilation	route=outside	Unsupported: parameter mutable parameter

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__drop_requires__69dc4db1d8501575/main.omg	current=accepted	current_us=2896578	root=compilation	route=outside	Unsupported: type reference

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__weaken_requires__3398e68f2ec52a15/main.omg	current=accepted	current_us=3021491	root=compilation	route=outside	Unsupported: parameter mutable parameter

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__drop_requires__885ac42f221b8727/main.omg	current=accepted	current_us=2984726	root=compilation	route=outside	Unsupported: parameter mutable parameter

```


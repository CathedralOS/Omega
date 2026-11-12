# Soundness mutations shard 0

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target
export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD
timeout 120 nice -n 19 target/debug/examples/route_one_comparison <mutant>/main.omg < /dev/null
mbx build -p omega -p compiler --example route_one_comparison
python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --shard 0 --resume
```

## Counts

- Fixtures visited this run: 0
- Fixtures completed (progress file): 781
- Originals skipped (not accepted): 25
- Mutants generated (this run): 0
- Mutants run (cumulative): 859
## Mutant bucket table (sums to mutant total)

| Bucket | Count |
| --- | ---: |
| confirmed-invalid | 80 |
| known-gap-q7-mut | 184 |
| discarded-valid | 595 |
| discarded-no-rule | 0 |
| mutant-run-failed | 0 |
| mutant-generate-failed | 0 |
| **mutant total** | **859** |

Baseline mutant total before this resume pass: 859. Delta explained in shard-0.meta.json.

Downgraded from pre-review `confirmed-invalid`: 0 (reclassified into the buckets above, chiefly `discarded-valid`).

- Confirmed-invalid by mutation kind: {'remove_mut (q7)': 184, 'remove_mut': 29, 'flip_borrow_mutability': 38, 'widen_requires': 13}

## Confirmed-invalid and known-gap rows

### tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg — remove_mut

- Original: `tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_runtime_unit_dc1f1a502f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L4: located(&mut result, 5);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arrays/runtime_index_recast/main.omg — remove_mut

- Original: `tests/omega/pass/arrays/runtime_index_recast/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L9: entries[0] = FieldEntry {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arrays/runtime_index_recast/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arrays/runtime_index_recast/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L3: // `&main.bytes[main.position] as &mut RawView` views the byte at a`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg — remove_mut

- Original: `tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_atomic_release_disp_b91482c80d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L20: Completion::complete(ack, &mut word);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/checked_label_edges_exit/main.omg — remove_mut

- Original: `tests/omega/pass/asm/checked_label_edges_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_checked_label_edges_35e4663f16/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_97b8432341/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L54: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_ffa91e3d99/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let slot: &write Held = &write held;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_9232230b4b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L16: replace(&write cells, main.position, incoming);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — widen_requires

- Original: `tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Expected exit: 0.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L5: let independent_address: addr = address_of(&mut independent[1]);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_borrowed_record_99db62a2e7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L14: replace(&mut holder, &after);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_call_replaced_l_b5c60db84c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L15: x.value = 5;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/collection_element_loans_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/collection_element_loans_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_collection_elem_c0417b82f0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L25: set(&mut values[1], 71);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_copied_input_de_91ca8bd3f3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let observed: i32 = relay(view, &mut counter);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_erased_input_be_1bf3efc7c7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L47: let total: u64 = exercise(&proof, &mut one, &mut two, &mut other);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_foreign_output__6a4211a4b3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L31: let first_number: i64 = refresh(&first_view, &mut output);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_foreign_output__6a4211a4b3/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: machine refresh<'source>(source: &'source InputDirectory<'source>, output: &mut Output<'source>) -> i64`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_nested_referenc_6ac3210cce/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L7: bytes[0] = 37;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_opaque_incoming_f057633817/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let bundle: Bundle = Bundle { windows: [&mut backing[..]] };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_aliased_acb842d2c6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let pair: Pair = Pair { left: View { bytes: &mut left[..] }, right: View { bytes: &mut right[..] } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_byte_vi_f87f448851/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L7: machine nonempty(bytes: &mut [u8]) { write(&mut bytes[0]); }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_compute_1a51da560d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L14: let view: View = View { bytes: &mut items[..] };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_depende_2e9acbd49b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: let observed: u64 = receive_inferred(snapshot(direct, direct_start, end_and_reset(&mut direct_start), &mut direct_counter), overwrite(&mut direct));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_dyn_mul_5749c73d82/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L42: let first: u64 = receive(Windowing::window(choose(true), original.bytes, original.sibling), overwrite(&mut original, "123456789"), 11);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_indexed_aa711e4395/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_mixed_byte_view_reference_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_mixed_byte_view_reference_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_mixed_b_e911c78306/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L9: let payload: Payload = make("AZ", &mut cell);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_nested__aa8380265e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L8: let messages: [[&mut Cell; 2]; 1] = collect(&mut pair);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_owned_s_fa1c685752/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L18: let carrier: Carrier = Carrier { message: Message { body: &mut original } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_project_f740484ddc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L8: let messages: [&mut Cell; 2] = collect(&mut pair);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_reassig_f07d443358/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L8: Choice::First -> &mut view[0]`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_scalar__8c2dfc7a56/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_selecte_7bddbfcf86/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L43: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_shared_view_record_presentation_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_shared_view_record_presentation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_shared__6809b457d2/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L26: let first: Window = wrap(&mut values);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_tail_re_8f07a6450e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L8: transition envelope.work.count <= 8 { true -> guarded(&mut envelope) false -> bad() }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_writing_2b535c4ae9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: let observed: u64 = receive(apply<keep>(original, &mut counter), overwrite(&mut original, "123456789"));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_supplied_output_3ee210c685/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L20: install(&backing, &mut output);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg — remove_mut

- Original: `tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_boundary_interleaved_st_12d0438fc3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_boundary_interleaved_st_12d0438fc3/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L21: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg — remove_mut

- Original: `tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_explicit_lifetime_adc511b61f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L16: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg — remove_mut

- Original: `tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_static_assumed_bi_bd7ffe305a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: Build::assume_binding_contract(&mut builder);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg — widen_requires

- Original: `tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_native_code_layou_876f0b3c29/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L2: // The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/calls/outcome_specific_primary_expression/main.omg — remove_mut

- Original: `tests/omega/pass/calls/outcome_specific_primary_expression/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_outcome_specific__8b4e92e192/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L15: current.threshold = 6;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_returned_nested_r_20d2fc1902/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L15: touch_left(&mut packet.pair.left);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_cd2a640e3b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L32: let skipped: bool = selected(false, &mut count);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_0116bfd000/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L24: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_0116bfd000/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L24: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_fused_chi_648f5881e2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L29: probe(&mut output);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_guarded_t_3ce4e9aa53/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: let skipped: u64 = choose(&mut count, false);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_indexed_c_07c28cc901/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L41: let first: u64 = reserve(&mut owner, 0, 0, 47);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_selected__7214fdfaca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L7: true -> &mut pair.right`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_static_ta_8c8cea1f5d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L33: let first: u64 = Counter::even(&mut counter, 1000);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_static_boundary_h_da452228fa/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L31: let exclusive: i32 = exclusive_field(&mut holder);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg — remove_mut

- Original: `tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_capabilities_sysv_small_991977704a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/cases/arm_returns_mut_ref/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/cases/arm_returns_mut_ref/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_arm_returns_mut_r_c55faf76ab/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L2: // `&level.rooms[i]` into `-> &mut Room` — the selected loan writes`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/collections/array_local_control/main.omg — remove_mut

- Original: `tests/omega/pass/collections/array_local_control/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_array_local_17310fb910/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L8: let row: [u8; 2] = keep([write(&mut current, 7u8), write(&mut current, value)]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_arr_022fc549f9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L49: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_com_aa21562af2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let observed:u32 in Wrapping=keep(cells[select(&mut cursor)]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_dyn_4c98030ea3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: Writer::store(selected, &mut rows[cursor.row].data[cursor.column], cursor.row == 2 && later(cursor));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_mul_e871a920ca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L20: let cells: &mut [Cell] = &mut grid.cells[..];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_ord_bf2e9012d1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let observed:u32 in Wrapping=keep(cells[select(&mut cursor,1)][select(&mut cursor,2)].value);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_comptime_runtime_generi_9e9492e92e/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L31: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg — remove_mut

- Original: `tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_concurrency_runtime_arr_af71a849a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L44: suspend Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_concurrency_runtime_arr_af71a849a8/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L44: suspend Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg — remove_mut

- Original: `tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_constants_runtime_predi_f51969e043/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L48: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/borrowed_field_ensures/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/borrowed_field_ensures/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_borrowed_fiel_b60c543e78/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L32: let scope: Scope = Scope::open(&mut picker);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/borrowed_field_ensures/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/borrowed_field_ensures/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_borrowed_fiel_b60c543e78/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L14: machine Scope::open(picker: &mut Picker) -> Scope`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_nested_call_l_1132177519/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: let first: u64 = retain(take(&mut counter, 48, 16));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_returned_data_393b98e9e9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L42: let forwarded: bool = observe(&mut storage[1u64..4u64]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_dispatch_l_6ba06ed8c4/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: countdown(&mut output, 2);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_br_367dc6d22c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L29: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_di_4ae6a0c7a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L51: let first: u64 = choose(true, &mut node);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_gu_b533a5091a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: let mut left: [Window; 1] = [Window { values: &mut left_values[..] }];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/dependent/data_where_disequality_compile/main.omg — widen_requires

- Original: `tests/omega/pass/dependent/data_where_disequality_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_di_42d0db8130/widen_requires/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // A `data where` clause may require a disequality: `Handle` is valid only`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg — remove_mut

- Original: `tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_runtime_symbo_c2d12c9b3d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L77: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_qualified_fixed_9db6b45d66/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L11: mutate(&mut bytes);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/domains/routed_selected_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/domains/routed_selected_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_routed_selected_d7314712cd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L32: let observed: u64 = replace(&mut frames, 3, issue());`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_mixed_tem_cfa3626fd2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L24: let answer: u64 = consume(make_pair(&mut counter).first, &make_token(&mut counter), &mut observed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_projected_5c6d00870c/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L38: let written: i64 = PosixConsoleIo::write(1, &mut cell, 1);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_selected__a90cb6a2ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L58: select_and_cleanup(&mut output, &mut spare, pick);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_selected__a90cb6a2ed/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: machine write(descriptor: i32, buffer: &mut i32, count: u32) -> i32`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_temporary_404b2bc071/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L11: let answer: u64 = read(&Token { value: source, marker: replace(&mut source) });`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_wildcard_owned_st_0b341be49b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L32: let first: Packet = selected_value(Choice::First { receipt: Receipt { destination: &mut cleanup } }, &mut bodies);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/entry_mut_scalar_param/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/expressions/entry_mut_scalar_param/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_entry_mut_s_f9e89c8333/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Broken obligation: `L2: // borrowed scalar context, not a `&mut Main`: stores through `out``
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_mat_bcaa124482/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L14: let observed: i32 = match produce(&mut counter, &cell) {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_nes_bf47dfaf19/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L9: make(produce(&mut count)).value`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg — widen_requires

- Original: `tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_own_2fe8e27d33/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Expected exit: 0`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_own_2fe8e27d33/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L16: replace(&mut parcel) -> parcel`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/additional_input_domain_mutation_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_additional_input__07fd2a2fd2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L11: overwrite(&write cell);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_additional_input__93cb2ee216/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L61: SelectedProbe::exercise(&mut cell)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_affine_discard_ro_20ce259c10/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L35: let result: u64 = exercise(&mut dropped);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_aggregate_storage_b11599fb98/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Compatible references to a live Pair expose the same nonzero address without retaining a loan.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_body_bearing_comp_65d43bb294/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L32: let carriers: Carriers = Carriers { left: produced(&mut count), right: produced(&mut count) };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_projecti_a776592e2f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L44: let direct_code: u64 = capture(&CapturedReceipt { code: envelope.seed.code, counter: &mut direct_drops }).receipt.code;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_restorat_2b15a32145/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: write_cell(&mut cell);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_42c6c46741/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: update(forward(&mut subject), &second);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_a5ccb2229f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L31: Inspect::update(&mut subject, &second);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_boundary_zero_ori_884b7a854b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L14: let status: i32 = transport(&mut request);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_c2f33aa611/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: let bytes: &mut [u8] = ActivationClaim::bytes(&mut holder.claim);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_b17a2b1f8c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L25: let bytes: &mut [u8] = ActivationClaim::bytes(&mut live);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/build_activation_borrow/call_result/main.omg — remove_mut

- Original: `tests/omega/pass/facts/build_activation_borrow/call_result/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__8a1b07674c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: apply(&mut carrier)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/build_activation_borrow/target_body/main.omg — remove_mut

- Original: `tests/omega/pass/facts/build_activation_borrow/target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__628c87c68f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: apply(&mut carrier)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_nested_r_ab7bc981ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L10: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_scalar_r_4a4c3c3155/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L14: record.first = 17u8;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_call_result_bound_03596d51f2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L44: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_storage__9aec5b07c3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L48: update(&mut first, true);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_window_s_7e97362406/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L53: let mut windows: [Window; 2] = [Window { values: &mut original[..] }, Window { values: &mut neighbor[..] }];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_dependent_de_ea48f47ac9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L48: Interval::Range { .. } -> both_widths(&mut current)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_match_payloa_8955a60679/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: let mut message: Message = Message { body: &mut backing };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_excl_713b35b2ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L19: true -> &mut chosen.value`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_stor_1e2db0fb70/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L28: let loan: Loan = forward(lend(&mut backing));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_where_equali_62b73a8913/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: run_main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_flag__e023be858d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L5: observe(&mut object);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_nomin_4ba90d1b0c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L13: let ignored: u64 = reset(&mut cursor);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_rank_decrease_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_rank_decrease_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_rank__e27d7cbf8f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L17: transition output == 0 { true -> done() false -> bad() }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_predicate_6200799d72/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L9: let first: &mut Cell = &mut values[first_index];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__6c78287f21/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: let reported: u64 = suspend Lending::write(&mut value, &token, original, member);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__2b9c1f7d3a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L9: let reported: u64 = suspend Lending::write(&mut value, &token, original, member);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_4fd30e9354/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L62: run_main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_fdf7190daa/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L63: run_main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_nested_s_e9b201f0e3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L29: transition { _ -> apply(&mut rows[choose_row(cursor, row)].data[choose_column(cursor, column)], replacement(cursor)) }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_record_w_e89b872c1e/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // In the which == 0 arm, [0..which + 2] has length 2; hold preserves that`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_record_w_e89b872c1e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: 0 -> &mut samples[0..(match flag { true -> which + 2, false -> 2 })]`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_copied_call_argum_363055d885/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L28: let observed: u64 = receive(source, overwrite(&mut source));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_correlated_invari_f1bcb739eb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L18: advance(&mut interior, 40);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/crash_entry_signed_zero_through_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/crash_entry_signed_zero_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_crash_entry_signe_710e1f63d2/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: an entry f32 equal to -0.0 discharges maybe_nonzero's value == 0.0 Trap crash guard without a caller crash ceiling, because the guard reads recorded IEEE equality and -0.0 compares equal to 0.0.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_ba5e1b3bba/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L21: && first.before == 17 && first.after == 53 && first.marker == 19`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_0fd418b741/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: && first.before == 17 && first.after == 53 && first.marker == 19`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_a3fb49c52d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L59: && first.before == 17 && first.after == 53 && first.marker == 19`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_1ecda18efb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L16: replace(&mut storage.value, replacement);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_predicate_helper_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_predicate_helper_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_76e5fd1487/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L14: let selected: &mut i32 = &mut counter.value;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_d462a51454/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L16: replace(&mut upper, 60);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/default_domain_field_write_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/default_domain_field_write_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_default_domain_fi_e065fc6dad/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: span.hi = 3;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dependency_type_qualified_call_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/dependency_type_qualified_call_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependency_type_q_74a838c993/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: let total: u64 = lib::ops::Wrapped::describe(&mut pair.left, &pair.right);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_88c2cba30b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L19: Main::main(&mut a);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_f3ecfb9ec1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L42: Main::main(&mut a);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_payloa_0ce946e244/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: true -> &mut chosen.value[..]`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_range__6768f5ee8f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L18: transition { _ -> pick(&mut chosen, &mut other, flag, visits) }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_field_bo_21dc8cb974/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: touch_left(&mut pair.left);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_windows__f842790519/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: let written: bool = exercise(&mut entries[..]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_62970e46d3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: exercise(&mut first, true, 11);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_239a198f5d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: consume(Keeper { output: &mut output, code: 7 });`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_aaf570e880/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: forward(Keeper { output: &mut output, code: 7 });`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_double_indexed_rm_2b3ceec031/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L30: Writer::run(&mut writer, &mut grid, &mut rows, 1, 2, 2, 1)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_double_indexed_rm_2b3ceec031/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L7: machine Writer::run(writer: &Writer, grid: &mut [[i32 in Wrapping; 4]; 3], rows: &mut [Row; 3],`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dynamic_trait_dis_b66acc04f2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L48: let first: u64 = probe(first_selector, 60u64, 40u64, &mut first_observation);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exact_sub_guard_t_0bcb8b6d57/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_observa_a204c6a65a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L18: let held: u64 = expose(&mut items[0]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_observa_a204c6a65a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: let held: u64 = expose(&mut items[0]);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_field_where_index_26fc5847d7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L41: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg — remove_mut

- Original: `tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_exc_2840302c53/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L62: parameter(&mut counter);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/finite_family_input_index/other_state/main.omg — remove_mut

- Original: `tests/omega/pass/facts/finite_family_input_index/other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_inp_985a0d68c0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L42: let item: Indexed<saved> = Family::row<width>(selected, input(&mut limit, observations), limit, observations);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_bit__c0dedd418d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L10: entries[0] = FieldEntry {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_byte_e770b9ce71/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L14: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_2994c6e431/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L11: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 8 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_9f4129df70/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L9: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 8 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_region_byte_172becc274/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: let output: &mut [u8; 40] = &mut destination[1..6] as &mut [u8; 40];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_90e3cbbe66/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: demand(measured == 5 && rest == 3 && text[0] == 111 && text[5] == 0 && text[6] == 120 && text[7] == 121);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_006ea89868/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L23: demand(text[6] == 120 && text[7] == 121);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_nested_re_eb078b095b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: let pending: Pending = Writer::register(capacity, &mut buffer);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_reference_a575aab8a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: let first: bool = journey(&mut left, true);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__4a2951f787/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L39: let pending: Pending = Writer::register(capacity, &mut buffer);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__2f8d756126/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: let pending: Pending = Writer::register(capacity, &mut buffer);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__359c206d3b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L43: let returned: Capacity in Capacity::Available = Reader::complete(routed, &mut observed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__c31590c33e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L40: let returned: Capacity in Capacity::Available = Reader::complete(pending, &mut observed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_free_entry_local__cc33d3d1f4/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L13: settle(&Mark { slot: &mut tally, code: code });`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_requireme_92ee591776/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L28: notify<i32>(&subject, MARK, &mut first);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_call_entr_f54f32260b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L11: receive<Plan>(&mut context, remaining_steps)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_call_entr_f54f32260b/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L6: public machine CompactBinary::read_record<machine Plan>(slots: &mut [u64], remaining_steps: u64, choose: bool) -> Observation`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_line_loan_d4fa33548a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L19: let result: ReadOutcome = receive(&mut line);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_record_li_5376f47177/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: let result: ReadOutcome = receive(&mut line, end);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/implementation_region_forwarding_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/implementation_region_forwarding_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_implementation_re_cede2c1302/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L20: let result: u64 = exercise(&mut environment, &storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_in_place_view_arg_a3036b322e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L18: packet[0] = 11;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/indexed_crash_guard_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_crash_guard_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_crash_gua_8ff9c0a2ec/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L20: transition items[0] == 7 { true -> finish(read_seven(&mut items, 0, true), &items) _ -> bad() }`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_recoverab_7fe62eb409/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: repair(&mut envelope.holder, false);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_storage_a_6bf08fcbf6/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // For runtime u64 index < 2 into live [u64; 2], compatible references to items[index] expose the same nonzero addr and differ from the other live element address.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_storage_a_6bf08fcbf6/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L7: let selected: addr = address_of(&mut items[index]);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_invariant_window__a517ffd939/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: let first_result: i32 = inspect(&mut first, -1, 3, true);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg — remove_mut

- Original: `tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_linear_owner_borr_2f54468b2d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L33: let mut holder: Holder = Holder { leading: 17, buffer: Buffer { marker: 91, route: expected_route, cells: &mut cells, cleanup: &mut cleanup_slots }, trailing: 29 };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_computed_ro_d213116412/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: 17 -> &mut chosen[0]`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_effectful_s_0997df5678/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L13: kept.mark = 40;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_loan_last_u_e67c5a9ffd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L41: true -> &mut chosen.value[0..2]`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_runtime_vie_573c6be3c9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L24: true -> &mut chosen`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_matched_field_win_dace8967c8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L25: let window: &mut [u8] = from_local(&mut bytes, 0);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_choice__20b7074239/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: let marker: u64 in Wrapping = add(overwrite(choose(&mut bounds.chosen, &mut bounds.other, true)), zero);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_a_5bc20dc91f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L31: Blob::encode(&sample, &mut buffer, choose(&mut chosen, &mut other, true));`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_a_5bc20dc91f/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L31: Blob::encode(&sample, &buffer, choose(&mut chosen, &mut other, true));`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_65e02a488e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: let counter: Counter = Counter { slot: &mut chosen };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_f97f9e8553/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L24: encode_packet(&sample, &mut buffer, counter.slot);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_compute_7746b6aa09/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L11: entries[0] = FieldEntry {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_named_s_0096ce0e69/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L36: let marker: u64 in Wrapping = add(overwrite(relay(&mut chosen, &mut other, true)), zero);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_nested__d5b7aa8c89/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L37: let marker: u64 in Wrapping = add(overwrite(relay(&mut chosen, &mut other, true)), zero);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_stored__3ff715a295/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L35: referent = choose(&mut chosen, &mut other, true);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/method_array_operand_inference_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/method_array_operand_inference_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_method_array_oper_5bd8a21182/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L12: let observed: u8 = Bytes::last<3u64>(&mut bytes);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_81b291ac6a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L12: let r: &mut Counter = &mut c;`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_eed6193d44/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L28: let r: &mut Counter = &mut c;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_eed6193d44/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L28: let r: &mut Counter = &mut c;`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_slice_ele_195cc7b76d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: &mut backing.items`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_slice_ele_195cc7b76d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L10: &mut backing.items`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/named_const_callable_family_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/named_const_callable_family_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_named_const_calla_7a5b955638/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L34: transition relay(&mut four, &two) {`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_narrowed_integer__2b96879f59/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L10: entries[0] = FieldEntry {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_borrow_qua_638eadb35a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L14: attributes.name.words[0] == 11`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_borrow_qua_638eadb35a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L43: Main::main(&mut entry_storage);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_result_ext_fd456485b1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: let first: u64 = trial(&mut bytes[..], &other[..], 13);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_loan_back_afe909b1dc/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L21: machine write(descriptor: i32, buffer: &mut i32, count: u32) -> i32`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_reference_04e5ecf716/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L20: observe(&Keeper { output: &mut output, code: seed.code });`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_placed_stable_wri_fc6fc7e60b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L10: entries[0] = FieldEntry {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_d5b2121817/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L13: initialize(&mut values);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_ffb8e7affd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L56: initialize(&mut values);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_923093c622/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L41: let first: u64 = select(left, right, 0, &mut calls);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_d0b18c045d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L38: let first: u64 = select(left, right, 0, &mut calls);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_7ec44511e5/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L33: let first: u64 = select(&left, &right, 0, &mut calls);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_recast_parent_res_1e4e1bba7d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L30: inspect(&mut first);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_arrival_0598aff530/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L7: let original: &mut i64 = &mut backing;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_recast__e0f6b7b130/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L8: let original: addr = address_of(&mut bytes[START]);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reflection_declar_4472bb05a6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: machine apply(counts: &mut Counts) { reflect::inspect_record_fields<Subject, describe>(once(&mut counts)); }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_replaced_field_lo_bb9cd4ba47/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L18: let mut message: Message = Message { body: &mut original };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_call_result_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__5d257d2dca/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Expected exit: 0`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/resident_backing_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__5d257d2dca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L15: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__79c27ed7a9/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Expected exit: 0`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__79c27ed7a9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L15: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__68be774286/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Expected exit: 0`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__68be774286/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L17: entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_returned_write_de_8a2853b2eb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L7: let destination: &write i32 = lend_write(&write value);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_044182ae7d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: let mut indexed:Index<(origin+1u32)> = Index{value:payload(&mut origin)};`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_25f8563d61/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: preserve(limit,&mut indexed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_3d541ed68b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L25: preserve(limit,&mut indexed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_0871120062/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L12: let indexed:Index<((&mut flag)+total)> = Index{value:8};`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_f330775b74/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: preserve(limit,&mut indexed);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_ea53703bda/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: let indexed:Index<(count(total,&mut flag))> = Index{value:8};`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_61438fb02f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L27: let mut indexed:Index<(origin+1u32)> = Index{value:payload(&mut origin)};`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_case_predic_7d637b25fe/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: let saved: bool = capture<&mut Choice>(&mut choice, &mut other) in Choice::Full;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_c3694cff95/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: let saved: u64 = read_kept(&mut items, &mut marker);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_7ecef72bbf/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L63: transition SelectedObserve::observe(&mut items, &mut marker) == 7 && items[1] == 4 && marker == 11 { true -> done() _ -> bad() }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_01110f92b3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L11: copied[0] = 4;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_da6710ccc2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L11: copied[0] = 4;`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_3901b17bd7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L15: let saved: u64 = capture<&u64>(&items[0], &mut marker, 11);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_b3c838eb10/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_8f04e91d18/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.`
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_provider_256ac0e37f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L102: Main::main(&mut application);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_provider_256ac0e37f/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L102: Main::main(&mut application);`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_skipped_short_cir_9827983b15/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L23: let first: bool = ready && bump(&mut counter);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stack_frame_slot__c0385332c3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L37: transition slot_a.left == 10 && slot_a.right == 20 && slot_b.left == 15 && slot_b.right == 25 && slot_c.left == 10 && slot_c.right == 20 {`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_static_callable_t_b36160d8ea/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L21: apply(&mut value);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stored_exclusive__5c82c6a2cc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L6: let windows: [&mut [u8]; 1] = [&mut backing[..]];`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_target_layout_der_a47db4ab79/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L26: true -> mark_size(OBSERVED_SIZE, &mut route, 17)`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__d2ea29438f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L15: mutate(&mut value, &mut count);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__a4403c0e32/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L123: visit(&subject, &mut totals);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/value_location_alignment_contract_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/value_location_alignment_contract_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_value_location_al_843a07033d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/value_location_alignment_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/value_location_alignment_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_value_location_al_97f7a69914/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/view_extent_after_write_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/view_extent_after_write_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_view_extent_after_0200b81fb1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L19: preserve_suffix(&mut bytes[..]);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/wildcard_structural_return_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/wildcard_structural_return_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_wildcard_structur_40c1da8b8a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L29: let first: u64 = evaluate(select(Choice::First, &mut counter), true);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_borrow_initialization_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_borrow_initialization_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_borrow_d8701f8251/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L25: initialize_slot(&write slot);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_borrow_initialization_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/write_only_borrow_initialization_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_borrow_d8701f8251/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: a write-only borrow (&mut write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/write_only_borrow_struct_destination_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_borrow_struct_destination_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_borrow_b8c85f93c5/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L22: initialize_destination(&write slot);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_borrow_struct_destination_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/write_only_borrow_struct_destination_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_borrow_b8c85f93c5/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: Initializing a destination storage slot through a write-only borrow (&mut write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/write_only_ieee_store_readback_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_ieee_store_readback_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_ieee_s_fdd16cdc1b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L6: machine fetch() -> f64 { let slot: f64; store(&write slot); slot }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_ieee_store_readback_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/write_only_ieee_store_readback_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_ieee_s_fdd16cdc1b/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: an IEEE f64 computed and stored through a write-only borrow (&mut write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/write_only_ieee_store_readback_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_ieee_store_readback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_ieee_s_8530ed5cad/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L52: store(&write slot);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_ieee_store_readback_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/write_only_ieee_store_readback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_ieee_s_8530ed5cad/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: an IEEE f64 computed and stored through a write-only borrow (&mut write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

### tests/omega/pass/facts/write_only_indexed_reborrow_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_indexed_reborrow_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_indexe_e1ee4357ce/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L33: exercise(&write parent, &mut storage.position);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_match_join_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_match_join_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_match__71a028ebdd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L17: transition { _ -> pick(&mut chosen, &mut other, flag) }`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_recast_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_recast_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_recast_26a0f1d0e0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L10: overwrite(&write original);`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_reference_store_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/write_only_reference_store_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_refere_c80dee1610/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **known-gap-q7-mut**
- Broken obligation: `L20: true -> lend(&write held, &write marker),`
- Spec (wiki/spec/language/ownership.md):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/write_only_reference_store_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/write_only_reference_store_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_write_only_refere_c80dee1610/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Broken obligation: `L1: // Fact: a `&mut write` store into a record's reference leaf installs the incoming`
- Spec (wiki/spec/language/ownership.md):

> Reborrowing retains exact immediate parent lineage. Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch.

## Hand verification: confirmed-invalid (5)

### tests/omega/pass/arrays/runtime_index_recast/main.omg — remove_mut

- current=`accepted` route=`no-entry` verdict=`confirmed-invalid`
- obligation: `L9: entries[0] = FieldEntry {`
- route_reason: ``
- review_reason: `current accepts illegal write on non-mut local`

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/remove_mut/main.omg	current=accepted	current_us=2549749	root=compilation	route=no-entry

```

### tests/omega/pass/arrays/runtime_index_recast/main.omg — flip_borrow_mutability

- current=`accepted` route=`no-entry` verdict=`confirmed-invalid`
- obligation: `L3: // `&main.bytes[main.position] as &mut RawView` views the byte at a`
- route_reason: ``
- review_reason: `current accepted borrow mutability change; route=no-entry`

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/flip_borrow_mutability/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/flip_borrow_mutability/main.omg	current=accepted	current_us=2444148	root=compilation	route=no-entry

```

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — widen_requires

- current=`accepted` route=`outside` verdict=`confirmed-invalid`
- obligation: `L1: // Expected exit: 0.`
- route_reason: ``
- review_reason: `weakened guard with body use; route=outside`

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/widen_requires/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/widen_requires/main.omg	current=accepted	current_us=3351857	root=compilation	route=outside	Unsupported: type fixed array

```

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — flip_borrow_mutability

- current=`accepted` route=`outside` verdict=`confirmed-invalid`
- obligation: `L5: let independent_address: addr = address_of(&mut independent[1]);`
- route_reason: ``
- review_reason: `current accepted borrow mutability change; route=outside`

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/flip_borrow_mutability/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/flip_borrow_mutability/main.omg	current=accepted	current_us=3302720	root=compilation	route=outside	Unsupported: type fixed array

```

### tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`confirmed-invalid`
- obligation: `L15: x.value = 5;`
- route_reason: ``
- review_reason: `current accepts illegal write on non-mut local`

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_call_replaced_l_b5c60db84c/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_call_replaced_l_b5c60db84c/remove_mut/main.omg	current=accepted	current_us=3299587	root=compilation	route=outside	Unsupported: machine generic parameters

```

## Hand verification: known-gap-q7 remove_mut (5)

### tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`known-gap-q7-mut`
- obligation: `L4: located(&mut result, 5);`
- route_reason: ``
- review_reason: `Q7 gap: writable loan/store on non-mut `result``

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_runtime_unit_dc1f1a502f/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_runtime_unit_dc1f1a502f/remove_mut/main.omg	current=accepted	current_us=3090667	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`known-gap-q7-mut`
- obligation: `L20: Completion::complete(ack, &mut word);`
- route_reason: ``
- review_reason: `Q7 gap: writable loan/store on non-mut `word``

**omega** exit=0
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_atomic_release_disp_b91482c80d/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_atomic_release_disp_b91482c80d/remove_mut/main.omg	current=accepted	current_us=2997158	root=compilation	route=outside	Unsupported: machine service reach

```

### tests/omega/pass/asm/checked_label_edges_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`known-gap-q7-mut`
- obligation: `L23: Main::main(&mut entry_storage);`
- route_reason: ``
- review_reason: `Q7 gap: writable loan/store on non-mut `entry_storage``

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_checked_label_edges_35e4663f16/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_checked_label_edges_35e4663f16/remove_mut/main.omg	current=accepted	current_us=3206158	root=compilation	route=outside	Unsupported: machine attached data

```

### tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`known-gap-q7-mut`
- obligation: `L54: Main::main(&mut entry_storage);`
- route_reason: ``
- review_reason: `Q7 gap: writable loan/store on non-mut `entry_storage``

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_97b8432341/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_97b8432341/remove_mut/main.omg	current=accepted	current_us=3278464	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg — remove_mut

- current=`accepted` route=`outside` verdict=`known-gap-q7-mut`
- obligation: `L12: let slot: &write Held = &write held;`
- route_reason: ``
- review_reason: `Q7 gap: writable loan/store on non-mut `held``

**omega** exit=0
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_ffa91e3d99/remove_mut/main.omg; wrote_output=false

```
**route** exit=0
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_ffa91e3d99/remove_mut/main.omg	current=accepted	current_us=3487456	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

## Hand verification: discarded drop_requires (5)

_No rows in this shard._

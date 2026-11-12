# Soundness mutations shard 5 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
Host: cursor
Shard selection: fixture index `i % 8 == 5` over pass/ and run/ (1130 fixtures).

## Bucket table (every mutant exactly one row)

| Bucket | Count |
| --- | ---: |
| `known-gap-q7-mut` | 69 |
| `discarded-valid` | 128 |
| `discarded-both-reject` | 868 |
| `timeout` | 2 |
| **Total mutants** | **1067** |

Notes: `discarded-valid` includes former `confirmed-invalid` rows where `drop_requires` left the body/transition still establishing the removed fact, `remove_mut` had no writable use of that binding, or the mutation kind was not a concrete spec violation. Downgraded `remove_mut` re-checks from the prior postprocess are counted inside `discarded-valid`, not as an extra bucket.

**Confirmed-invalid holes (current accepts):** 0
**Known gap Q7 mut (current accepts; tracked migration):** 69

| Mutation kind | confirmed-invalid | known-gap-q7-mut |
| --- | ---: | ---: |
| `remove_mut` | 0 | 69 |

## Commands
```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg
timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg
python3 .agents/four-jobs-2026-11-12/soundness/reconcile_fix_shard5.py
```

## Hand verification (5 samples, re-run)

### `remove_mut` — `tests/omega/pass/atomics/placed_atomic_load/main.omg`
- Obligation: line 6: `entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut`
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut/main.omg; wrote_output=false
```
```text
(empty stderr)
```
- route verdict: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut/main.omg	current=accepted	current_us=4241794	root=compilation	route=no-entry
```

### `remove_mut` — `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg`
- Obligation: line 10: `let flag: bool = Integer::touch(&mut other, 9);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut`
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut/main.omg; wrote_output=false
```
```text
(empty stderr)
```
- route verdict: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut/main.omg	current=accepted	current_us=1414598	root=compilation	route=no-entry
```

### `remove_mut` — `tests/omega/pass/borrow/write_only_whole_scalar_replace/main.omg`
- Obligation: line 14: `forward(&write destination, 42);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut`
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut/main.omg; wrote_output=false
```
```text
(empty stderr)
```
- route verdict: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut/main.omg	current=accepted	current_us=1413581	root=compilation	route=no-entry
```

### `remove_mut` — `tests/omega/pass/borrows/copied_image_descriptor_unrelated_write/main.omg`
- Obligation: line 8: `let length: u64 = receive(span, tick(&mut counter));`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut`
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut/main.omg; wrote_output=false
```
```text
warning: unreachable branch: the `true` arm of a transition in machine `main` state `entry` can never run; its guard contradicts the facts in force here: 0 >= counter
```
- route verdict: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut/main.omg	current=accepted	current_us=2944607	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `remove_mut` — `tests/omega/pass/borrows/pending_registration_write_after_completion/main.omg`
- Obligation: line 41: `buffer.value = 99;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_pending_registration_write_after_completion_main_omg_55e4dda937__remove_mut`
- `omega --check` exit 0
```text
compiled 30 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_pending_registration_write_after_completion_main_omg_55e4dda937__remove_mut/main.omg; wrote_output=false
```
```text
(empty stderr)
```
- route verdict: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_pending_registration_write_after_completion_main_omg_55e4dda937__remove_mut/main.omg	current=accepted	current_us=3697004	root=compilation	route=outside	Unsupported: machine service reach
```

## Known gap: Q7 mut (sample)

- `tests/omega/pass/atomics/placed_atomic_load/main.omg` (`remove_mut`): line 6: `entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg` (`remove_mut`): line 10: `let flag: bool = Integer::touch(&mut other, 9);`
- `tests/omega/pass/borrow/write_only_whole_scalar_replace/main.omg` (`remove_mut`): line 14: `forward(&write destination, 42);`
- `tests/omega/pass/borrows/copied_image_descriptor_unrelated_write/main.omg` (`remove_mut`): line 8: `let length: u64 = receive(span, tick(&mut counter));`
- `tests/omega/pass/borrows/pending_registration_write_after_completion/main.omg` (`remove_mut`): line 41: `buffer.value = 99;`
- `tests/omega/pass/borrows/runtime_held_descriptor_replacement_exit/main.omg` (`remove_mut`): line 17: `let mut message: Message = Message { body: &mut original };`
- `tests/omega/pass/borrows/runtime_qualified_returned_descriptor_exit/main.omg` (`remove_mut`): line 18: `let mut message: Message = Message { body: &mut backing };`
- `tests/omega/pass/borrows/runtime_shared_result_attenuates_exclusive_origin_exit/main.omg` (`remove_mut`): line 26: `Main::main(&mut entry_storage);`
- `tests/omega/pass/borrows/suspension_projected_call_view_whole_input_restore_exit/main.omg` (`remove_mut`): line 11: `let source: &mut Backing = &mut backing;`
- `tests/omega/pass/calls/runtime_guarded_reference_return_crash/main.omg` (`remove_mut`): line 11: `let result: &mut Node = choose(&mut node, false);`
- …and 59 more (see TSV).

## Confirmed-invalid entries

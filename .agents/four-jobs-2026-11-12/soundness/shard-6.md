# Soundness mutations shard 6 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Status: in progress

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison
export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD
python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
```

Harness: `target/debug/omega --check` (current checker), `timeout 120 target/debug/examples/route_one_comparison` (route). Shard index rule: fixture line `i` in sorted `pass/`+`run/` list with `i % 8 == 6`.

## Counts

- fixtures in shard: 1130
- originals skipped (current does not accept): 92
- mutants generated: 229
- mutants run: 229
- candidates (either accepts or disagree): 229
- confirmed holes kept: 229
- disagreements kept: 153
- discarded valid (both reject, no disagreement): 0
- discarded no rule: 0
- timeouts: 0

## Confirmed holes and disagreements

### tests/omega/pass/arithmetic/checked_flag_proof_signed_mask/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__checked_flag_proof_signed_mask__main.omg__flip_to_mut_loan__33dddbda8b7a4228/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut values -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__10ff42d0ca60c80a/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__drop_requires__615e4fcebcc33af4/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__weaken_requires__b3ab713df4f44c56/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__weaken_requires__86d52d74da640c01/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__flip_to_mut_loan__812f268293dcee22/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut guard -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__d18d099dc0c88dd0/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__flip_to_mut_loan__69f736f1c415262e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut condition -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__remove_mut__567fa9d8227fb733/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__18dd7b3809419645/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__weaken_requires__7c30a3cbffb7776d/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__flip_to_mut_loan__47e90131bf30f090/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arithmetic/runtime_signed_minimum_remainder_postcondition_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_signed_minimum_remainder_postcondition_exit__main.omg__flip_to_mut_loan__eda5fa6d085b7246/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut output -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__4f809f02a1eeb3a1/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__flip_to_mut_loan__d32f635847a60b02/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arrays/computed_fixed_array_length_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__computed_fixed_array_length_exit__main.omg__flip_to_mut_loan__d0e2ec6c3919791a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cell -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__remove_mut__099e906ffa223a0b/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__drop_requires__d46b7126fb2c2352/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__weaken_requires__9cceba158a498c99/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__flip_to_mut_loan__475d55cb4142c497/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/asm/atomic_release_disposition_pair_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut word -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__atomic_release_disposition_pair_exit__main.omg__remove_mut__63e9f4a5ebd222cc/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cursor -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__2081fee7512be4b2/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__drop_requires__e00ecd5b72dd7ab6/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__weaken_requires__abdf4dfe7445f47b/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__flip_to_mut_loan__2be550a83c25960a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__0a6ba8abea0556a8/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__flip_to_mut_loan__1b76f003d32679d8/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/asm/scratch_snapshot_survives_input_write_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__scratch_snapshot_survives_input_write_exit__main.omg__drop_requires__0ec11805cf655378/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/atomics/placed_atomic_shared_store/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entries -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__placed_atomic_shared_store__main.omg__remove_mut__1ed660626c86d733/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut trace -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_atomic_computed_updates_exit__main.omg__remove_mut__b93beb17b8690622/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_atomic_computed_updates_exit__main.omg__flip_to_mut_loan__d6f8e239e09efc1e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/atomics/runtime_mixed_common_atomic_cell_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_mixed_common_atomic_cell_exit__main.omg__flip_to_mut_loan__60c31da6ee990c4e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cells -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_indexed_write_only_local_receiver_exit__main.omg__remove_mut__95274c9293d876e6/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_indexed_write_only_local_receiver_exit__main.omg__drop_requires__f0d442609b9b7d02/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_indexed_write_only_local_receiver_exit__main.omg__flip_to_mut_loan__925a9b6f1de6321f/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrow/runtime_static_callable_preserves_readonly_subject_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_static_callable_preserves_readonly_subject_exit__main.omg__drop_requires__a78b06456e3d16aa/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrow/runtime_static_callable_preserves_readonly_subject_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_static_callable_preserves_readonly_subject_exit__main.omg__flip_to_mut_loan__9e99acf0f612cd35/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_aggregate_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut held -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_aggregate_exit__main.omg__remove_mut__e82708131068d7cd/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_aggregate_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_aggregate_exit__main.omg__flip_to_mut_loan__10f106d031b56e3d/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_returned_view_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut held -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_returned_view_exit__main.omg__remove_mut__a6c77f14c3c1d21a/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_returned_view_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_returned_view_exit__main.omg__flip_to_mut_loan__6920e273b648f706/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrow/storage_address_byte_window_identity_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrow__storage_address_byte_window_identity_exit__main.omg__flip_to_mut_loan__de434952217f75d9/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/aliased_descriptor_slot_after_last_use_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut holder -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__aliased_descriptor_slot_after_last_use_exit__main.omg__remove_mut__f97a392cb244e20e/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/aliased_descriptor_slot_after_last_use_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__aliased_descriptor_slot_after_last_use_exit__main.omg__flip_to_mut_loan__b0f1286650d25361/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/borrowed_literal_backing_releases_input_loan_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut second -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__borrowed_literal_backing_releases_input_loan_exit__main.omg__remove_mut__5041045f507f6952/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/borrowed_literal_backing_releases_input_loan_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__borrowed_literal_backing_releases_input_loan_exit__main.omg__flip_to_mut_loan__ab61e5ba0cfe0662/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut original -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__conditional_copied_slot_restores_parent_exit__main.omg__remove_mut__a63e4651d95ed547/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__conditional_copied_slot_restores_parent_exit__main.omg__flip_to_mut_loan__9df7fe4a9bd3f4ad/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/copied_input_descriptor_independent_mutation_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut counter -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__copied_input_descriptor_independent_mutation_exit__main.omg__remove_mut__cc68f5139f2705c7/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/copied_input_descriptor_independent_mutation_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__copied_input_descriptor_independent_mutation_exit__main.omg__flip_to_mut_loan__c696a2d82c41ab8d/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/foreign_output_copied_lifetime_last_use/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut first -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__foreign_output_copied_lifetime_last_use__main.omg__remove_mut__65b2786fbf4de8cb/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/initialized_empty_array_snapshot_replacement_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut packet -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__initialized_empty_array_snapshot_replacement_exit__main.omg__remove_mut__71e9c5c23b431503/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/initialized_empty_array_snapshot_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__initialized_empty_array_snapshot_replacement_exit__main.omg__flip_to_mut_loan__fcf9c7f2e2436992/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/literal_index_reference_copy_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut bytes -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__literal_index_reference_copy_exit__main.omg__remove_mut__0a146431972975a2/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/literal_index_reference_copy_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__literal_index_reference_copy_exit__main.omg__flip_to_mut_loan__4cdb720e36a0e9e9/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut bytes -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__remove_mut__dd3658b87dd60904/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__drop_requires__dbbe1f4f316b9bc2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__weaken_requires__1f65f214778af6b4/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__local_subslice_parent_lent_after_last_use_exit__main.omg__flip_to_mut_loan__3130a453b00027a0/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut backing -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__remove_mut__48eeb031629177b3/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__drop_requires__6aa3768683e3aeb8/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/opaque_incoming_nested_projected_array_result_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__opaque_incoming_nested_projected_array_result_exit__main.omg__flip_to_mut_loan__bb5e9b3f18750a3e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/ordinary_select_provider_reference_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut holder -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__ordinary_select_provider_reference_exit__main.omg__remove_mut__26047f8c04b2d3b5/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/ordinary_select_provider_reference_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__ordinary_select_provider_reference_exit__main.omg__flip_to_mut_loan__79a060ea91b68810/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut items -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__remove_mut__1b4a313b962e5590/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__drop_requires__ad844d400c944eba/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__weaken_requires__3398e68f2ec52a15/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__plain_read_after_exclusive_loan_last_use_exit__main.omg__flip_to_mut_loan__714c2300683318ea/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/qualified_reference_field_retarget_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut protected -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__qualified_reference_field_retarget_exit__main.omg__remove_mut__b8600a335fa69fba/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entries -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__resident_view_borrowed_helper_exit__main.omg__remove_mut__c870ab4901bf19f6/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__resident_view_borrowed_helper_exit__main.omg__drop_requires__008de9f6ea9be31c/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/resident_view_borrowed_helper_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__resident_view_borrowed_helper_exit__main.omg__flip_to_mut_loan__2bc5275d2be6cdb1/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_byte_case_partial_cleanup_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_byte_case_partial_cleanup_exit__main.omg__flip_to_mut_loan__5cba6bea6f0a763e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_computed_backing_reference_copy_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_computed_backing_reference_copy_exit__main.omg__drop_requires__d5791e2cb3911aef/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_computed_backing_reference_copy_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_computed_backing_reference_copy_exit__main.omg__weaken_requires__8395ce35635cf3f0/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/runtime_computed_backing_reference_copy_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_computed_backing_reference_copy_exit__main.omg__flip_to_mut_loan__19aa09fcad22dc2c/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_copied_byte_distinct_epochs_exit__main.omg__remove_mut__632e49cf38084812/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_copied_byte_distinct_epochs_exit__main.omg__drop_requires__cec963ad87c42e85/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_copied_byte_distinct_epochs_exit__main.omg__weaken_requires__0943f022ae4c92a4/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/runtime_copied_byte_distinct_epochs_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_copied_byte_distinct_epochs_exit__main.omg__flip_to_mut_loan__9e05fc11429b9e9e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut direct_counter -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_dependent_forwarded_window_exit__main.omg__remove_mut__576e2aa49077bc69/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_dependent_forwarded_window_exit__main.omg__drop_requires__7b4cc37c0d901593/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_dependent_forwarded_window_exit__main.omg__weaken_requires__be6e09230d107238/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/runtime_dependent_forwarded_window_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_dependent_forwarded_window_exit__main.omg__flip_to_mut_loan__72a607ff7479f54b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut original -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_formed_child_descriptor_replacement_exit__main.omg__remove_mut__083128b817a7eb4d/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_formed_child_descriptor_replacement_exit__main.omg__drop_requires__0721a3f0b41bc533/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_formed_child_descriptor_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_formed_child_descriptor_replacement_exit__main.omg__flip_to_mut_loan__cc2c0d52a81f966a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_independent_qualified_owner_reference_exit__main.omg__drop_requires__82688e3a43b2fd6b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_independent_qualified_owner_reference_exit__main.omg__weaken_requires__3ec61dd94f7bd7d9/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_independent_qualified_owner_reference_exit__main.omg__flip_to_mut_loan__7db1496c31ba3d3c/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_independent_qualified_owner_reference_exit/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_independent_qualified_owner_reference_exit__main.omg__borrow_mut_on_immutable__cd00b2c54f063eec/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/borrows/runtime_match_payload_parent_disjoint_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut backing -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_match_payload_parent_disjoint_exit__main.omg__remove_mut__4e4004f3e3d071ee/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_match_payload_parent_disjoint_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_match_payload_parent_disjoint_exit__main.omg__flip_to_mut_loan__a14691c798065f2e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cells -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_mutable_reference_captures_selector_exit__main.omg__remove_mut__3bcca1c547b3268d/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_mutable_reference_captures_selector_exit__main.omg__drop_requires__2b8c7d18ea4f853a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_mutable_reference_captures_selector_exit__main.omg__flip_to_mut_loan__69ae47969c83a478/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_owned_input_reference_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_owned_input_reference_replacement_exit__main.omg__flip_to_mut_loan__35cf2265d19a638a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__remove_mut__3fc6040ed61fc15d/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__drop_requires__54c79d1055ec11f3/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__weaken_requires__02db558d2d236820/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/borrows/runtime_projected_record_region_loop_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_projected_record_region_loop_exit__main.omg__flip_to_mut_loan__b361edeb44af3549/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut original -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_qualified_statement_descriptor_replacement_exit__main.omg__remove_mut__854753cfda970395/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_qualified_statement_descriptor_replacement_exit__main.omg__drop_requires__3549d06c79bbb2cd/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_qualified_statement_descriptor_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_qualified_statement_descriptor_replacement_exit__main.omg__flip_to_mut_loan__6fe5000f3fb1bb79/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_reference_call_scalar_snapshot_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_reference_call_scalar_snapshot_exit__main.omg__remove_mut__85a84c8cc49b90fe/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_reference_call_scalar_snapshot_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_reference_call_scalar_snapshot_exit__main.omg__flip_to_mut_loan__b1327a715d6ede89/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_selected_stored_carrier_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut left -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_selected_stored_carrier_exit__main.omg__remove_mut__bc9fea0372e5430a/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_selected_stored_carrier_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_selected_stored_carrier_exit__main.omg__flip_to_mut_loan__0ed6737f8b1be341/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut visits -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_static_callback_snapshot_exit__main.omg__remove_mut__d9631aafa28fb1a9/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_static_callback_snapshot_exit__main.omg__drop_requires__a96cb9fc17b979b1/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/borrows/runtime_static_callback_snapshot_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_static_callback_snapshot_exit__main.omg__flip_to_mut_loan__5f272ea23bc7ee64/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/runtime_writeonly_nested_reference_array_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut first -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_writeonly_nested_reference_array_exit__main.omg__remove_mut__db3c44bf1d197478/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_writeonly_nested_reference_array_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__runtime_writeonly_nested_reference_array_exit__main.omg__flip_to_mut_loan__382ac80e52c11036/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/stored_view_return_consumers/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__stored_view_return_consumers__main.omg__borrow_mut_on_immutable__26399af817bbcc20/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/borrows/suspension_saved_reference_view_all_suppliers_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__suspension_saved_reference_view_all_suppliers_exit__main.omg__flip_to_mut_loan__45e15b58fa2f3936/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/borrows/tagged_scalar_field_live_copy_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__borrows__tagged_scalar_field_live_copy_exit__main.omg__flip_to_mut_loan__78b1a8bd60384417/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/boundary/requirement_float_precondition_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__boundary__requirement_float_precondition_exit__main.omg__drop_requires__3b9f83e14c48ce87/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/build/activation_descriptor_write_observed_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut carrier -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__activation_descriptor_write_observed_exit__main.omg__remove_mut__6d653e431b2173c9/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/build/activation_descriptor_write_observed_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__activation_descriptor_write_observed_exit__main.omg__flip_to_mut_loan__673eb72d02701bc0/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/build/copied_domain_source_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__copied_domain_source_exit__main.omg__flip_to_mut_loan__a522a85883948278/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/build/installed_entry_ram_two_occurrences_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entries -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__installed_entry_ram_two_occurrences_exit__main.omg__remove_mut__2a4b119f7891f908/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/build/installed_entry_ram_two_occurrences_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__installed_entry_ram_two_occurrences_exit__main.omg__flip_to_mut_loan__8b4e70e3b31b7596/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/build/runtime_retained_const_application_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__runtime_retained_const_application_exit__main.omg__flip_to_mut_loan__4eb8cf278ef14f46/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/build/sibling_module_service_exclusion_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__build__sibling_module_service_exclusion_exit__main.omg__remove_mut__5f5e94a94677fdb7/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__drop_requires__e3ff1b90665705f2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__weaken_requires__2f8c72858e96d25c/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__weaken_requires__419acebd5b763b4b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__converted_call_value_private_arrival_exit__main.omg__flip_to_mut_loan__139255d9f85ec1c0/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/native_code_layout_frame_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__native_code_layout_frame_exit__main.omg__drop_requires__cff93fb9ddfdb02f/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/calls/native_code_layout_frame_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__native_code_layout_frame_exit__main.omg__flip_to_mut_loan__556a72490cde13e5/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/nested_binding_call/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__nested_binding_call__main.omg__borrow_mut_on_immutable__37fd371b902a30e6/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/calls/nested_scalar_argument_evaluation_order_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut value -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__nested_scalar_argument_evaluation_order_exit__main.omg__remove_mut__eedaedc5eae0e765/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/nested_scalar_argument_evaluation_order_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__nested_scalar_argument_evaluation_order_exit__main.omg__flip_to_mut_loan__3039b3a0ab7b5291/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/owned_overaligned_local_call/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entries -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__owned_overaligned_local_call__main.omg__remove_mut__583e8d17dae0de9f/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/owned_overaligned_local_call/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__owned_overaligned_local_call__main.omg__flip_to_mut_loan__e685616a840ad6b2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/recursive_terminal_block_acknowledgement_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__recursive_terminal_block_acknowledgement_exit__main.omg__remove_mut__f9640f609ac052e1/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/recursive_terminal_block_acknowledgement_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__recursive_terminal_block_acknowledgement_exit__main.omg__drop_requires__8f41c19a1535c76a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/calls/recursive_terminal_block_acknowledgement_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__recursive_terminal_block_acknowledgement_exit__main.omg__weaken_requires__851f32b7e24a7a8e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/calls/recursive_terminal_block_acknowledgement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__recursive_terminal_block_acknowledgement_exit__main.omg__flip_to_mut_loan__f6d820df7856c893/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_constructor_child_membership_snapshot_exit__main.omg__remove_mut__2fb2e17500d37181/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_constructor_child_membership_snapshot_exit__main.omg__drop_requires__b13ddb3f9acd4430/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_constructor_child_membership_snapshot_exit__main.omg__flip_to_mut_loan__09d58f071594d639/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/runtime_guarded_reference_return_machine_crash/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut node -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_guarded_reference_return_machine_crash__main.omg__remove_mut__056bb843fb6fa461/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_guarded_terminal_structural_result_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_guarded_terminal_structural_result_exit__main.omg__flip_to_mut_loan__05b6179f77919b94/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/runtime_nested_constructor_child_snapshots_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_nested_constructor_child_snapshots_exit__main.omg__remove_mut__52c0fa294d859efa/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_nested_constructor_child_snapshots_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_nested_constructor_child_snapshots_exit__main.omg__drop_requires__41f7611087a1961b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/calls/runtime_nested_constructor_child_snapshots_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_nested_constructor_child_snapshots_exit__main.omg__flip_to_mut_loan__2db18e6e154f07d3/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/runtime_qualified_owned_result_argument_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_qualified_owned_result_argument_exit__main.omg__weaken_requires__7cd50011cb51c669/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/calls/runtime_qualified_owned_result_argument_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_qualified_owned_result_argument_exit__main.omg__flip_to_mut_loan__b6786f67a3b15bd2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/runtime_selected_call_argument_from_fieldwise_local_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut pair -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_selected_call_argument_from_fieldwise_local_exit__main.omg__remove_mut__5513444bd7f377eb/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_selected_call_argument_from_fieldwise_local_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__runtime_selected_call_argument_from_fieldwise_local_exit__main.omg__flip_to_mut_loan__12e088ba83010b3f/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/calls/terminal_call_acknowledgements/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__calls__terminal_call_acknowledgements__main.omg__flip_to_mut_loan__1c31e2922f5dfec0/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/capabilities/alternative_synchronous_provider_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__capabilities__alternative_synchronous_provider_exit__main.omg__flip_to_mut_loan__38e3d212ac83aeda/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/cases/arm_field_placement/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__arm_field_placement__main.omg__borrow_mut_on_immutable__4535568bd30fa1a3/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/cases/case_where_common/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__case_where_common__main.omg__drop_requires__5f9c67f91716135b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/cases/case_where_common/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__case_where_common__main.omg__weaken_requires__21cb2da1150e2206/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/cases/case_where_common/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__case_where_common__main.omg__flip_to_mut_loan__eebe219fc7c23443/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/cases/match_reference_local/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__match_reference_local__main.omg__flip_to_mut_loan__ec479bfb2032ed80/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/cases/runtime_case_payload_policy_cast_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut entry_storage -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__runtime_case_payload_policy_cast_exit__main.omg__remove_mut__a49c337d6fcfaf9d/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/cases/runtime_case_payload_policy_cast_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__runtime_case_payload_policy_cast_exit__main.omg__flip_to_mut_loan__a39a8eae076be5ec/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/cases/unit_empty_arm/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__unit_empty_arm__main.omg__flip_to_mut_loan__bf1d62bb0496b1c2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/cases/unit_empty_arm/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__cases__unit_empty_arm__main.omg__borrow_mut_on_immutable__c6198ea88a720295/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/collections/runtime_captured_structural_copy_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut matrix -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_captured_structural_copy_exit__main.omg__remove_mut__4d483fa55645bae3/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_captured_structural_copy_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_captured_structural_copy_exit__main.omg__flip_to_mut_loan__c41ee680ab7fcd2a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/runtime_float_array_projection_return_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_float_array_projection_return_exit__main.omg__drop_requires__22096277e1185226/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/collections/runtime_float_array_projection_return_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_float_array_projection_return_exit__main.omg__flip_to_mut_loan__05c96f83240beaad/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/runtime_nested_selected_element_extents_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_nested_selected_element_extents_exit__main.omg__flip_to_mut_loan__7f07006795c324b5/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/runtime_projected_element_descriptor_read_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cells -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_projected_element_descriptor_read_exit__main.omg__remove_mut__72791647f6d4c727/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_projected_element_descriptor_read_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_projected_element_descriptor_read_exit__main.omg__drop_requires__80d2c52c7b6d7552/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/collections/runtime_projected_element_descriptor_read_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_projected_element_descriptor_read_exit__main.omg__flip_to_mut_loan__28ddbacecb854dd3/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/runtime_sequential_index_observations_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cursor -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_sequential_index_observations_exit__main.omg__remove_mut__00f4b4e819c41736/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_sequential_index_observations_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_sequential_index_observations_exit__main.omg__drop_requires__46c9321ee1f41ebb/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/collections/runtime_sequential_index_observations_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_sequential_index_observations_exit__main.omg__weaken_requires__7636eff739257bbd/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/collections/runtime_sequential_index_observations_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_sequential_index_observations_exit__main.omg__flip_to_mut_loan__02ac8f7a99abeb83/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/runtime_stored_reference_array_replacement_contract_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut backing -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_stored_reference_array_replacement_contract_exit__main.omg__remove_mut__285f6664dc2858c2/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_stored_reference_array_replacement_contract_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_stored_reference_array_replacement_contract_exit__main.omg__drop_requires__80f205ebbc35cafc/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/collections/runtime_stored_reference_array_replacement_contract_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__runtime_stored_reference_array_replacement_contract_exit__main.omg__flip_to_mut_loan__2cdae37c3d6fd831/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/collections/typed_run_relocation_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut cell -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__typed_run_relocation_exit__main.omg__remove_mut__eb0ae99506122ee8/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/typed_run_relocation_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__typed_run_relocation_exit__main.omg__drop_requires__78dc336f78360d4f/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/collections/typed_run_relocation_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__collections__typed_run_relocation_exit__main.omg__flip_to_mut_loan__9cf418083ee82178/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/comptime/runtime_const_array_cast_expression_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__comptime__runtime_const_array_cast_expression_exit__main.omg__drop_requires__2e4e13648ab728f5/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/comptime/runtime_const_array_cast_expression_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__comptime__runtime_const_array_cast_expression_exit__main.omg__weaken_requires__5db86b575e42f3ea/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/concurrency/incoming_loan_after_suspend/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut value -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__concurrency__incoming_loan_after_suspend__main.omg__remove_mut__83c8d4f9acee3dbf/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/concurrency/incoming_loan_after_suspend/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__concurrency__incoming_loan_after_suspend__main.omg__flip_to_mut_loan__48fa52b4d1094317/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/concurrency/suspend_carry_task/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__concurrency__suspend_carry_task__main.omg__borrow_mut_on_immutable__849fd49bf4ec6060/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/constants/predicate_constant_static_index/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__constants__predicate_constant_static_index__main.omg__drop_requires__1ec9cb9085c1bd66/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/constants/predicate_constant_static_index/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__constants__predicate_constant_static_index__main.omg__weaken_requires__facb4308b4dbe266/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/constants/target_layout_derived_helper_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__constants__target_layout_derived_helper_exit__main.omg__flip_to_mut_loan__412a6029adf82ec8/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/constraints/proof_machine_order_fact/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__constraints__proof_machine_order_fact__main.omg__drop_requires__66f87ae51234a47a/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/boolean_result_all_normal_returns_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__boolean_result_all_normal_returns_exit__main.omg__drop_requires__a5f80ee7266a5fec/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/completed_anonymous_cast_preserves_prefix_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__completed_anonymous_cast_preserves_prefix_exit__main.omg__drop_requires__3a55201bde546699/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/completed_anonymous_cast_preserves_prefix_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__completed_anonymous_cast_preserves_prefix_exit__main.omg__weaken_requires__1ecd5549c1817dc7/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/contracts/completed_anonymous_cast_preserves_prefix_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__completed_anonymous_cast_preserves_prefix_exit__main.omg__flip_to_mut_loan__318d9b8173402f27/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/contracts/contradictory_match_predecessor_join_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__contradictory_match_predecessor_join_exit__main.omg__drop_requires__788fc1f2351064f5/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/contradictory_match_predecessor_join_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires >= to >
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__contradictory_match_predecessor_join_exit__main.omg__weaken_requires__9d1720a0582166af/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/contracts/contradictory_match_predecessor_join_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__contradictory_match_predecessor_join_exit__main.omg__weaken_requires__b51526e54b1c22fe/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/contracts/contradictory_match_predecessor_join_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__contradictory_match_predecessor_join_exit__main.omg__flip_to_mut_loan__1c7dab8281c78828/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/contracts/folded_actual_requires/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__folded_actual_requires__main.omg__drop_requires__f30170fe4fdb777b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/folded_actual_requires/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__folded_actual_requires__main.omg__weaken_requires__664c507ef29623b4/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/contracts/folded_actual_requires/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__folded_actual_requires__main.omg__borrow_mut_on_immutable__b02c4f9efc1110b2/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/contracts/policy_field_ensures/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__policy_field_ensures__main.omg__drop_requires__15e353860f5cc45d/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/policy_field_ensures/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__policy_field_ensures__main.omg__borrow_mut_on_immutable__a7cb25698ec3b7b0/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/contracts/runtime_recursive_result_bound_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__runtime_recursive_result_bound_exit__main.omg__drop_requires__38691068984b3474/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/runtime_recursive_result_bound_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__runtime_recursive_result_bound_exit__main.omg__weaken_requires__82ce34cc7b42ac05/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/contracts/runtime_recursive_result_bound_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__runtime_recursive_result_bound_exit__main.omg__flip_to_mut_loan__c407a46eb3c285ff/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/contracts/static_type_requires_prior_call_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__static_type_requires_prior_call_exit__main.omg__drop_requires__ba5ecc3bc49ed399/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/contracts/static_type_requires_prior_call_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__contracts__static_type_requires_prior_call_exit__main.omg__flip_to_mut_loan__4cfc4f3062c13d69/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/bool_subject_dispatch_compile/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__bool_subject_dispatch_compile__main.omg__borrow_mut_on_immutable__961d667a53db0b16/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/control_flow/constant_aggregate_dispatch_complete/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__constant_aggregate_dispatch_complete__main.omg__drop_requires__579b03c373840eb4/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/control_flow/constant_aggregate_dispatch_complete/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__constant_aggregate_dispatch_complete__main.omg__flip_to_mut_loan__645aa88d0556d882/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/guarded_leaf_branch_expansion/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__guarded_leaf_branch_expansion__main.omg__borrow_mut_on_immutable__3d31b8fbe034d4fc/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.

### tests/omega/pass/control_flow/runtime_branch_parameter_cleanup_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_branch_parameter_cleanup_exit__main.omg__flip_to_mut_loan__d348b338932dae91/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/runtime_completed_readonly_home_replacement_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_completed_readonly_home_replacement_exit__main.omg__drop_requires__4b9c2d6bda38ee23/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/control_flow/runtime_completed_readonly_home_replacement_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_completed_readonly_home_replacement_exit__main.omg__weaken_requires__2301f31fe0000dff/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/control_flow/runtime_completed_readonly_home_replacement_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_completed_readonly_home_replacement_exit__main.omg__flip_to_mut_loan__d2cb836196af9a9f/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/runtime_narrow_signed_case_payload_state_guard_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_narrow_signed_case_payload_state_guard_exit__main.omg__flip_to_mut_loan__b34456c658ffef4e/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/runtime_saved_transition_subject_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut calls -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_saved_transition_subject_exit__main.omg__remove_mut__50b199ff869c869b/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_saved_transition_subject_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_saved_transition_subject_exit__main.omg__flip_to_mut_loan__a7a35d421741af53/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/runtime_static_type_terminal_unit_cleanup_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut frame -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_static_type_terminal_unit_cleanup_exit__main.omg__remove_mut__f2b92f9403f62407/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_static_type_terminal_unit_cleanup_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_static_type_terminal_unit_cleanup_exit__main.omg__drop_requires__f15b341a5edf6092/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/control_flow/runtime_static_type_terminal_unit_cleanup_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__runtime_static_type_terminal_unit_cleanup_exit__main.omg__flip_to_mut_loan__44febebcdae3c244/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/saved_case_predicate_disjoint_effects_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut marker -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__saved_case_predicate_disjoint_effects_exit__main.omg__remove_mut__3bb79c4b04ad1c49/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/saved_case_predicate_disjoint_effects_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__saved_case_predicate_disjoint_effects_exit__main.omg__flip_to_mut_loan__496dec2815afd072/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/control_flow/shared_view_selected_disposition_exit/main.omg — remove_mut

- Original tier: `pass`
- Mutation: let mut holder -> let (written binding)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__shared_view_selected_disposition_exit__main.omg__remove_mut__f2f729b15260c24f/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/shared_view_selected_disposition_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__control_flow__shared_view_selected_disposition_exit__main.omg__flip_to_mut_loan__d169753a3a8d8686/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/core/address_interval_partition_reverse_exit/main.omg — drop_requires

- Original tier: `pass`
- Mutation: delete first requires clause
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__core__address_interval_partition_reverse_exit__main.omg__drop_requires__55cf90897a803189/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. A written `requires` is complete under entry inference; omitting it admits invocations when required premises do not hold.

### tests/omega/pass/core/address_interval_partition_reverse_exit/main.omg — weaken_requires

- Original tier: `pass`
- Mutation: requires <= to <
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__core__address_interval_partition_reverse_exit__main.omg__weaken_requires__ee5160cdc0e422a6/main.omg`
- Current checker: `accepted`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/machines.md`):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution. This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use.

### tests/omega/pass/core/address_interval_partition_reverse_exit/main.omg — flip_to_mut_loan

- Original tier: `pass`
- Mutation: shared & -> &mut (first)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__core__address_interval_partition_reverse_exit__main.omg__flip_to_mut_loan__1bfdd557b6dfd264/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> `&mut T` is an exclusive observation and mutation loan; forming `&mut` where only shared `&T` authority exists violates exclusivity and mutability rules.

### tests/omega/pass/core/address_interval_partition_reverse_exit/main.omg — borrow_mut_on_immutable

- Original tier: `pass`
- Mutation: first &mut -> &write on non-mut local context
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__core__address_interval_partition_reverse_exit__main.omg__borrow_mut_on_immutable__341a8c725795e035/main.omg`
- Current checker: `rejected`; route: `accepted`
- Hand check: `confirmed-invalid`
- Spec rule (`wiki/spec/language/ownership.md`):

> Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans to owned storage requires a `mut` binding on that local.


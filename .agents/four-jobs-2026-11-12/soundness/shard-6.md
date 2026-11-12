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
- originals skipped (current does not accept): 8
- mutants generated: 32
- mutants run: 32
- candidates (either accepts or disagree): 32
- confirmed holes kept: 32
- disagreements kept: 18
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


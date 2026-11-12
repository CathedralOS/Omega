# Soundness mutations shard 3

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).
Shard selection: `(i % 8) == 3` over sorted `tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.

## Mutant buckets (each row is exactly one bucket)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 21 |
| `known-gap-q7-mut` | 118 |
| `disagreement-valid` | 36 |
| `discarded-valid` | 33 |
| `rejected-both` | 675 |
| `timeout` | 0 |
| `failed-run` | 0 |
| `discarded-no-rule` | 0 |
| **Total (mutants run)** | **883** |

Bucket sum check: 883 == 883 (OK).

- Fixtures in shard: 1130
- Fixtures processed: 470
- Mutants generated: 840

## Mutant total accounting

Primary pass recorded 840 mutants before `drop_requires_line` was re-enabled; backfill adds only missing `drop_requires_line` rows without deleting prior mutants.

### Confirmed-hole by mutation kind

- `drop_requires_line`: 10
- `promote_shared_ref_to_mut`: 6
- `demote_mut_ref_to_shared`: 3
- `corrupt_literal_callee_argument`: 2

### Confirmed-hole by accepted_by

- `both`: 9
- `route`: 12

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null
SHARD=3 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
SHARD=3 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --revalidate
```

Harness build commit: `0835b3aaf803ed97f905be42d94727ff58459263`. Host: `cursor`.
Omega: `/agent/repos/omega/target/debug/omega`. Route: `/agent/repos/omega/target/debug/examples/route_one_comparison`. Per-mutant timeout: 120s.

## Hand verification (5 confirmed-hole)

### ## Hand verification (5 confirmed-hole) 1: `tests/omega/pass/arithmetic/match_operand_versions/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `machine change(cell: &mut Cell) -> bool { cell.value = 2; true }`
- Route reason (hint only): ``
- Validity note: `write through `cell` after promoting & to &mut`
- Accepted by: `both`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg	current=accepted	current_us=1700443	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-hole) 2: `tests/omega/pass/borrows/borrowed_array_loop/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `out[position] = value;`
- Route reason (hint only): ``
- Validity note: `write through parameter `out` after demoting &mut to &`
- Accepted by: `both`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg	current=accepted	current_us=1634353	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-hole) 3: `tests/omega/pass/borrows/runtime_state_boolean_reference_exit/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `main.ready = true;`
- Route reason (hint only): ``
- Validity note: `write through `main` after promoting & to &mut`
- Accepted by: `route`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_state_boolean_reference_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_state_boolean_reference_exit__main.omg__promote_shared_ref_to_mut__312788ebc8/main.omg	current=accepted	current_us=12607950	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (5 confirmed-hole) 4: `tests/omega/pass/borrows/suspension_projected_call_view_shared_cohort_restore_exit/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: `source.right = 19;`
- Route reason (hint only): ``
- Validity note: `write through parameter `source` after demoting &mut to &`
- Accepted by: `both`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg	current=accepted	current_us=3692438	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (5 confirmed-hole) 5: `tests/omega/pass/calls/private_callback_call_closure/main.omg` / `corrupt_literal_callee_argument`

- Obligation / note: `emit(0);`
- Route reason (hint only): ``
- Validity note: `literal argument disagrees with callee requires`
- Accepted by: `route`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls; every arrival establishes the target's requirements under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `private_callback_call_closure` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__private_callback_call_closure__main.omg__corrupt_literal_callee_argument__063671b558/main.omg	current=accepted	current_us=14893805	root=compilation	route=outside	Unsupported: machine service reach
```

## Hand verification (disagreement-valid sample)

### ## Hand verification (disagreement-valid sample) 1: `tests/omega/pass/arithmetic/runtime_dispatch_cast_decisions_exit/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no write through affected reference parameter`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> 

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_dispatch_cast_decisions_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_dispatch_cast_decisions_exit__main.omg__demote_mut_ref_to_shared__a93dc59de0/main.omg	current=accepted	current_us=12164997	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (disagreement-valid sample) 2: `tests/omega/pass/arithmetic/runtime_trapping_mutable_local_traps/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no write through affected reference parameter`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> 

**omega --check** (stdout/stderr head):

```text
(empty)
warning: trapping arithmetic in machine `Main::main` state `main` assignment ALWAYS overflows `i32` -- this computation traps unconditionally at runtime (the trap is an effect and will fire even if the result is never used)
cannot compile admitted project: checked compilation failed for package `runtime_trapping_mutable_local_traps` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_trapping_mutable_local_traps__main.omg__demote_mut_ref_to_shared__39ceac4305/main.omg	current=accepted	current_us=12822172	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (disagreement-valid sample) 3: `tests/omega/pass/build/segment_selector_reload/main.omg` / `remove_mut_binding`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no write on demoted binding`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> 

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `segment_selector_probe` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__build__segment_selector_reload__main.omg__remove_mut_binding__72027e613e/main.omg	current=accepted	current_us=12170122	root=compilation	route=outside	Unsupported: declaration Build selection that is not a machine or a data type
```

## Hand verification (5 discarded drop_requires)

_None yet._
## Hand verification (5 known-gap-q7-mut)

### ## Hand verification (5 known-gap-q7-mut) 1: `tests/omega/pass/arithmetic/offset_integer_at/main.omg` / `remove_mut_binding`

- Obligation / note: `entries[0] = FieldEntry {`
- Route reason (hint only): ``
- Validity note: `write or &mut/&write on non-mut local`
- Accepted by: `both`
- Disposition: `known-gap-q7-mut` (known-gap-q7-mut)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Owned local mutability):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__offset_integer_at__main.omg__remove_mut_binding__bf6499144d/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__offset_integer_at__main.omg__remove_mut_binding__bf6499144d/main.omg	current=accepted	current_us=2352369	root=compilation	route=no-entry
```

### ## Hand verification (5 known-gap-q7-mut) 2: `tests/omega/pass/arithmetic/runtime_explicit_call_result_policy_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `values.exact = produce(9);`
- Route reason (hint only): ``
- Validity note: `write or &mut/&write on non-mut local`
- Accepted by: `both`
- Disposition: `known-gap-q7-mut` (known-gap-q7-mut)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Owned local mutability):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_explicit_call_result_policy_exit__main.omg__remove_mut_binding__3ec05424e1/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_explicit_call_result_policy_exit__main.omg__remove_mut_binding__3ec05424e1/main.omg	current=accepted	current_us=3422727	root=compilation	route=outside	Unsupported: type constrained
```

### ## Hand verification (5 known-gap-q7-mut) 3: `tests/omega/pass/asm/checked_countdown_literal_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `transition output == 0 { true -> done() false -> bad() }`
- Route reason (hint only): ``
- Validity note: `write or &mut/&write on non-mut local`
- Accepted by: `both`
- Disposition: `known-gap-q7-mut` (known-gap-q7-mut)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Owned local mutability):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__asm__checked_countdown_literal_exit__main.omg__remove_mut_binding__8a61b2d28e/main.omg; wrote_output=false
warning: unreachable branch: the `true` arm of a transition in machine `main` state `entry` can never run; its guard contradicts the facts in force here: output >= 99
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__asm__checked_countdown_literal_exit__main.omg__remove_mut_binding__8a61b2d28e/main.omg	current=accepted	current_us=3184381	root=compilation	route=outside	Unsupported: machine assembly target
```

### ## Hand verification (5 known-gap-q7-mut) 4: `tests/omega/pass/atomics/runtime_atomic_operand_order_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `record(&mut trace, 1, 10), record(&mut trace, 2, 99), NoOrdering, NoOrdering);`
- Route reason (hint only): ``
- Validity note: `write or &mut/&write on non-mut local`
- Accepted by: `both`
- Disposition: `known-gap-q7-mut` (known-gap-q7-mut)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Owned local mutability):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__runtime_atomic_operand_order_exit__main.omg__remove_mut_binding__8536509326/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__runtime_atomic_operand_order_exit__main.omg__remove_mut_binding__8536509326/main.omg	current=accepted	current_us=3509863	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### ## Hand verification (5 known-gap-q7-mut) 5: `tests/omega/pass/borrow/mutable_selector_rhs_snapshot_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `let marker: u64 in Wrapping = overwrite(choose(&mut chosen, &mut other, true));`
- Route reason (hint only): ``
- Validity note: `write or &mut/&write on non-mut local`
- Accepted by: `both`
- Disposition: `known-gap-q7-mut` (known-gap-q7-mut)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Owned local mutability):

> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__mutable_selector_rhs_snapshot_exit__main.omg__remove_mut_binding__1e4d43208f/main.omg; wrote_output=false
warning: unreachable branch: the `true` arm of a transition in machine `main` state `entry` can never run; its guard contradicts the facts in force here: 1 >= other
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__mutable_selector_rhs_snapshot_exit__main.omg__remove_mut_binding__1e4d43208f/main.omg	current=accepted	current_us=3579695	root=compilation	route=outside	Unsupported: machine generic parameters
```

## Confirmed-hole entries

### 1. `tests/omega/pass/arithmetic/match_operand_versions/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine change(cell: &mut Cell) -> bool { cell.value = 2; true }`
- Accepted by: **both**
- Validity note: `write through `cell` after promoting & to &mut`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 2. `tests/omega/pass/borrows/borrowed_array_loop/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `out[position] = value;`
- Accepted by: **both**
- Validity note: `write through parameter `out` after demoting &mut to &`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 3. `tests/omega/pass/borrows/runtime_state_boolean_reference_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_state_boolean_reference_exit__main.omg__promote_shared_ref_to_mut__312788ebc8/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.ready = true;`
- Accepted by: **route**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 4. `tests/omega/pass/borrows/suspension_projected_call_view_shared_cohort_restore_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `source.right = 19;`
- Accepted by: **both**
- Validity note: `write through parameter `source` after demoting &mut to &`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 5. `tests/omega/pass/calls/private_callback_call_closure/main.omg` — `corrupt_literal_callee_argument`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__private_callback_call_closure__main.omg__corrupt_literal_callee_argument__063671b558/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `emit(0);`
- Accepted by: **route**
- Validity note: `literal argument disagrees with callee requires`
- Route reason (hint): ``

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls; every arrival establishes the target's requirements under actual argument substitution.

### 6. `tests/omega/pass/calls/runtime_dispatch_result_enum_case_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_dispatch_result_enum_case_exit__main.omg__demote_mut_ref_to_shared__67855d45da/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.result = count(main, 3, 0, 0);`
- Accepted by: **route**
- Validity note: `write through parameter `main` after demoting &mut to &`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 7. `tests/omega/pass/calls/runtime_wrapping_division_call_span_exit/main.omg` — `corrupt_literal_callee_argument`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_wrapping_division_call_span_exit__main.omg__corrupt_literal_callee_argument__ccc76a6d01/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `Console::write_byte((value / divisor % 10 + 48) as i32);`
- Accepted by: **route**
- Validity note: `literal argument disagrees with callee requires`
- Route reason (hint): ``

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls; every arrival establishes the target's requirements under actual argument substitution.

### 8. `tests/omega/pass/collections/mutable_slice_view_establishment/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__collections__mutable_slice_view_establishment__main.omg__promote_shared_ref_to_mut__aeaf8f6fe5/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `transition main.bag.cells[2] == 7 {`
- Accepted by: **route**
- Validity note: `write through parameter `cells` after demoting &mut to &`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 9. `tests/omega/pass/domains/domain_subslice_carry/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.end = 4;`
- Accepted by: **both**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 10. `tests/omega/pass/facts/darwin_stat_abi_through_parameter_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__darwin_stat_abi_through_parameter_exit__main.omg__demote_mut_ref_to_shared__f34fac3343/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `value.dev == -2147483647i32 && value.mode == 33188u16 && value.nlink == 3u16 && value.ino == 4294967298u64 && value.uid == 123u32 && value.gid == 456u32 && value.rdev == -123456789i32 && value.atime.seconds == -17i64 && value.atime.nanoseconds == 123456789i64 && value.mtime.seconds == 42i64 && value.mtime.nanoseconds == 987654321i64 && value.ctime.seconds == -9i64 && value.ctime.nanoseconds == 111111111i64 && value.btime.seconds == 77i64 && value.btime.nanoseconds == 222222222i64 && value.size == 8589934593i64 && value.blocks == 123456i64 && value.blksize == 4096i32 && value.flags == 2147483649u32 && value.generation == 2882400001u32 && value.reserved == -314159i32 && value.spare[0] == -4294967297i64 && value.spare[1] == 68719476737i64 && StatExchange::check(&value) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.rdev)) == 4171510507u64`
- Accepted by: **route**
- Validity note: `write through parameter `value` after demoting &mut to &`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 11. `tests/omega/pass/facts/field_where_index_through_match_join_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__field_where_index_through_match_join_exit__main.omg__promote_shared_ref_to_mut__8cfa8adca8/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.arr = [10, 20, 30, 40, 50];`
- Accepted by: **both**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 12. `tests/omega/pass/arithmetic/anonymous_rational_arguments/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__anonymous_rational_arguments__main.omg__drop_requires_line__69e0e6d4ab/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `ensures result == value`
- Accepted by: **both**
- Validity note: `ensures still references facts from removed requires`
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 13. `tests/omega/pass/borrows/nested_machine_unit_output_ordinary_forward_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__nested_machine_unit_output_ordinary_forward_exit__main.omg__drop_requires_line__87d0d08312/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `transition remaining > 0 {`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires pending > 0``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 14. `tests/omega/pass/borrows/runtime_captured_record_nested_field_snapshot_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_captured_record_nested_field_snapshot_exit__main.omg__drop_requires_line__18000eab8a/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.items[0].prefix = 3;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 15. `tests/omega/pass/borrows/runtime_reference_record_array_field_read_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_reference_record_array_field_read_exit__main.omg__drop_requires_line__df3571292d/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.first.value = 70;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 16. `tests/omega/pass/borrows/runtime_shared_reference_array_cohort_restoration_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_shared_reference_array_cohort_restoration_exit__main.omg__drop_requires_line__c9b39e4ce8/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.cells.first.value = 11;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 17. `tests/omega/pass/borrows/runtime_stored_reference_case_residual_retirement_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_stored_reference_case_residual_retirement_exit__main.omg__drop_requires_line__ab28cd8fe6/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.first.value = 70;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires observed >= 0 && observed <= 100``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 18. `tests/omega/pass/borrows/state_element_clone/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__state_element_clone__main.omg__drop_requires_line__8b28b3a947/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `transition position < 2 {`
- Accepted by: **both**
- Validity note: `body uses facts from removed `requires position < 2``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 19. `tests/omega/pass/calls/native_code_layout_uncalled_boundary_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__native_code_layout_uncalled_boundary_exit__main.omg__drop_requires_line__bab23fb377/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `transition layout.regions.len > 0 { true -> region(layout) _ -> false }`
- Accepted by: **both**
- Validity note: `body uses facts from removed `requires layout.regions.len > 0``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 20. `tests/omega/pass/calls/runtime_constructor_projected_membership_snapshot_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_constructor_projected_membership_snapshot_exit__main.omg__drop_requires_line__71b2f598e3/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `ensures result in Ticket::Issued {`
- Accepted by: **both**
- Validity note: `ensures still references facts from removed requires`
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 21. `tests/omega/pass/calls/runtime_mutable_call_empty_input_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_mutable_call_empty_input_exit__main.omg__drop_requires_line__6757e042ea/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `loader.tag = loader.tags[index] as u8;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires index <= 16383``
- Route reason (hint): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

## Known gap: Q7 mut

Count: 118 (tracked migration; not counted as confirmed holes).

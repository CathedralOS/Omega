# Soundness mutations shard 3

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).
Shard selection: `(i % 8) == 3` over sorted `tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.

## Mutant buckets (each row is exactly one bucket)

| Bucket | Count |
| --- | ---: |
| `confirmed-invalid` | 6 |
| `checker-disagreement` | 21 |
| `known-gap-q7-mut` | 103 |
| `discarded-valid` | 303 |
| `discarded-no-rule` | 0 |
| `discarded-both-reject` | 407 |
| `discarded-timeout` | 0 |
| `discarded-run-failed` | 0 |
| `discarded-generate-skipped` | 0 |
| **Total (mutants run)** | **840** |

Bucket sum check: 840 == 840 (OK).

- Fixtures in shard: 1130
- Fixtures processed: 470
- Mutants generated: 840

## Mutant total accounting

Primary pass recorded 840 mutants before `drop_requires_line` was re-enabled; backfill adds only missing `drop_requires_line` rows without deleting prior mutants.

### Confirmed-invalid by mutation kind

- `promote_shared_ref_to_mut`: 4
- `demote_mut_ref_to_shared`: 1
- `tighten_requires_equality`: 1

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null
SHARD=3 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
SHARD=3 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --revalidate
```

Harness build commit: `0835b3aaf803ed97f905be42d94727ff58459263`. Host: `cursor`.
Omega: `/agent/repos/omega/target/debug/omega`. Route: `/agent/repos/omega/target/debug/examples/route_one_comparison`. Per-mutant timeout: 120s.

## Hand verification (5 confirmed-invalid)

### ## Hand verification (5 confirmed-invalid) 1: `tests/omega/pass/arithmetic/match_operand_versions/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `machine change(cell: &mut Cell) -> bool { cell.value = 2; true }`
- Route reason: ``
- Disposition: `confirmed-invalid` (confirmed-invalid)
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg	current=accepted	current_us=1467542	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-invalid) 2: `tests/omega/pass/borrows/borrowed_array_loop/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `out[position] = value;`
- Route reason: ``
- Disposition: `confirmed-invalid` (confirmed-invalid)
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg	current=accepted	current_us=1643267	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-invalid) 3: `tests/omega/pass/borrows/suspension_projected_call_view_shared_cohort_restore_exit/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: `source.right = 19;`
- Route reason: ``
- Disposition: `confirmed-invalid` (confirmed-invalid)
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg	current=accepted	current_us=3093157	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (5 confirmed-invalid) 4: `tests/omega/pass/domains/domain_subslice_carry/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: `main.end = 4;`
- Route reason: ``
- Disposition: `confirmed-invalid` (confirmed-invalid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg	current=accepted	current_us=1522687	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-invalid) 5: `tests/omega/pass/facts/captured_window_slot_retirement_through_other_machine_exit/main.omg` / `tighten_requires_equality`

- Obligation / note: `requires windows.len == 3 && slot == 0 {`
- Route reason: ``
- Disposition: `confirmed-invalid` (confirmed-invalid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__captured_window_slot_retirement_through_other_machine_exit__main.omg__tighten_requires_equality__0c82318547/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__captured_window_slot_retirement_through_other_machine_exit__main.omg__tighten_requires_equality__0c82318547/main.omg	current=accepted	current_us=2877538	root=compilation	route=outside	Unsupported: machine generic parameters
```

## Hand verification (5 discarded drop_requires)

_None yet._
## Hand verification (5 known-gap-q7-mut)

### ## Hand verification (5 known-gap-q7-mut) 1: `tests/omega/pass/arithmetic/offset_integer_at/main.omg` / `remove_mut_binding`

- Obligation / note: `entries[0] = FieldEntry {`
- Route reason: ``
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__offset_integer_at__main.omg__remove_mut_binding__bf6499144d/main.omg	current=accepted	current_us=2227263	root=compilation	route=no-entry
```

### ## Hand verification (5 known-gap-q7-mut) 2: `tests/omega/pass/arithmetic/runtime_explicit_call_result_policy_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `values.exact = produce(9);`
- Route reason: ``
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_explicit_call_result_policy_exit__main.omg__remove_mut_binding__3ec05424e1/main.omg	current=accepted	current_us=2952146	root=compilation	route=outside	Unsupported: type constrained
```

### ## Hand verification (5 known-gap-q7-mut) 3: `tests/omega/pass/asm/checked_countdown_literal_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `transition output == 0 { true -> done() false -> bad() }`
- Route reason: ``
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__asm__checked_countdown_literal_exit__main.omg__remove_mut_binding__8a61b2d28e/main.omg	current=accepted	current_us=2777400	root=compilation	route=outside	Unsupported: machine assembly target
```

### ## Hand verification (5 known-gap-q7-mut) 4: `tests/omega/pass/atomics/runtime_atomic_operand_order_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `record(&mut trace, 1, 10), record(&mut trace, 2, 99), NoOrdering, NoOrdering);`
- Route reason: ``
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__runtime_atomic_operand_order_exit__main.omg__remove_mut_binding__8536509326/main.omg	current=accepted	current_us=3022622	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### ## Hand verification (5 known-gap-q7-mut) 5: `tests/omega/pass/borrow/mutable_selector_rhs_snapshot_exit/main.omg` / `remove_mut_binding`

- Obligation / note: `let marker: u64 in Wrapping = overwrite(choose(&mut chosen, &mut other, true));`
- Route reason: ``
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__mutable_selector_rhs_snapshot_exit__main.omg__remove_mut_binding__1e4d43208f/main.omg	current=accepted	current_us=2911916	root=compilation	route=outside	Unsupported: machine generic parameters
```

## Confirmed-invalid entries

### 1. `tests/omega/pass/arithmetic/match_operand_versions/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine change(cell: &mut Cell) -> bool { cell.value = 2; true }`
- Route reason: ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 2. `tests/omega/pass/borrows/borrowed_array_loop/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `out[position] = value;`
- Route reason: ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 3. `tests/omega/pass/borrows/suspension_projected_call_view_shared_cohort_restore_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `source.right = 19;`
- Route reason: ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 4. `tests/omega/pass/domains/domain_subslice_carry/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.end = 4;`
- Route reason: ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 5. `tests/omega/pass/facts/captured_window_slot_retirement_through_other_machine_exit/main.omg` — `tighten_requires_equality`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__captured_window_slot_retirement_through_other_machine_exit__main.omg__tighten_requires_equality__0c82318547/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `requires windows.len == 3 && slot == 0 {`
- Route reason: ``

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 6. `tests/omega/pass/facts/field_where_index_through_match_join_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__field_where_index_through_match_join_exit__main.omg__promote_shared_ref_to_mut__8cfa8adca8/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.arr = [10, 20, 30, 40, 50];`
- Route reason: ``

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

## Known gap: Q7 mut

Count: 103 (tracked migration; not counted as confirmed holes).

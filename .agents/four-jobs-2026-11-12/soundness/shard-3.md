# Soundness mutations shard 3

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).
Shard selection: `(i % 8) == 3` over sorted `tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.

## Mutant buckets (each row is exactly one bucket)

| Bucket | Count |
| --- | ---: |
| `confirmed-invalid` | 14 |
| `checker-disagreement` | 19 |
| `known-gap-q7-mut` | 103 |
| `discarded-valid` | 297 |
| `discarded-no-rule` | 0 |
| `discarded-both-reject` | 407 |
| `discarded-timeout` | 0 |
| **Total (mutants run)** | **840** |

Bucket sum check: 840 == 840 (OK).

- Fixtures in shard: 1130
- Fixtures processed: 470
- Mutants generated: 840

### Confirmed-invalid by mutation kind

- `promote_shared_ref_to_mut`: 8
- `demote_mut_ref_to_shared`: 5
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

### Sample 1: `tests/omega/pass/arithmetic/match_operand_versions/main.omg` / `promote_shared_ref_to_mut`

- Obligation line: `machine stable(cell: &mut Cell, flag: bool) -> u32 {`
- Disposition: `confirmed-invalid`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg	current=accepted	current_us=1588988	root=compilation	route=no-entry
```

### Sample 2: `tests/omega/pass/arrays/qualified_array_literal/main.omg` / `demote_mut_ref_to_shared`

- Obligation line: `// member literals carry the same policy.`
- Disposition: `confirmed-invalid`
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arrays__qualified_array_literal__main.omg__demote_mut_ref_to_shared__aa661bf03f/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arrays__qualified_array_literal__main.omg__demote_mut_ref_to_shared__aa661bf03f/main.omg	current=accepted	current_us=1659066	root=compilation	route=no-entry
```

### Sample 3: `tests/omega/pass/atomics/selected_reference_atomic_provider/main.omg` / `promote_shared_ref_to_mut`

- Obligation line: `data Holder<T> { word: &mut T; }`
- Disposition: `confirmed-invalid`
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__selected_reference_atomic_provider__main.omg__promote_shared_ref_to_mut__22b525796a/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__selected_reference_atomic_provider__main.omg__promote_shared_ref_to_mut__22b525796a/main.omg	current=accepted	current_us=3090664	root=compilation	route=outside	Unsupported: declaration Build selection that is not a machine or a data type
```

### Sample 4: `tests/omega/pass/borrow/runtime_write_only_boundary_satisfier_exit/main.omg` / `promote_shared_ref_to_mut`

- Obligation line: `// `&mut write` calls routed through declared-foreign boundary requirements are`
- Disposition: `confirmed-invalid`
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__runtime_write_only_boundary_satisfier_exit__main.omg__promote_shared_ref_to_mut__70bc59a1a2/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__runtime_write_only_boundary_satisfier_exit__main.omg__promote_shared_ref_to_mut__70bc59a1a2/main.omg	current=accepted	current_us=3284981	root=compilation	route=outside	Unsupported: type data with custody
```

### Sample 5: `tests/omega/pass/calls/nested_where_binding/main.omg` / `demote_mut_ref_to_shared`

- Obligation line: `// record.`
- Disposition: `confirmed-invalid`
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__nested_where_binding__main.omg__demote_mut_ref_to_shared__1a3184cf72/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__nested_where_binding__main.omg__demote_mut_ref_to_shared__1a3184cf72/main.omg	current=accepted	current_us=1738951	root=compilation	route=no-entry
```

## Confirmed-invalid entries

### 1. `tests/omega/pass/arithmetic/match_operand_versions/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine stable(cell: &mut Cell, flag: bool) -> u32 {`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 2. `tests/omega/pass/arrays/qualified_array_literal/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arrays__qualified_array_literal__main.omg__demote_mut_ref_to_shared__aa661bf03f/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `// member literals carry the same policy.`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 3. `tests/omega/pass/atomics/selected_reference_atomic_provider/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__selected_reference_atomic_provider__main.omg__promote_shared_ref_to_mut__22b525796a/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `data Holder<T> { word: &mut T; }`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 4. `tests/omega/pass/borrow/runtime_write_only_boundary_satisfier_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__runtime_write_only_boundary_satisfier_exit__main.omg__promote_shared_ref_to_mut__70bc59a1a2/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `// `&mut write` calls routed through declared-foreign boundary requirements are`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 5. `tests/omega/pass/calls/nested_where_binding/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__nested_where_binding__main.omg__demote_mut_ref_to_shared__1a3184cf72/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `// record.`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 6. `tests/omega/pass/cases/runtime_borrowed_case_subject_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__cases__runtime_borrowed_case_subject_exit__main.omg__promote_shared_ref_to_mut__d2052bf471/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine authorized_read(message: &mut Message) -> u32 {`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 7. `tests/omega/pass/collections/private_cycle_guarded_arrivals/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__collections__private_cycle_guarded_arrivals__main.omg__promote_shared_ref_to_mut__73bd60374c/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `let found: u8 = scan(&mut bytes, 0);`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 8. `tests/omega/pass/contracts/arith_ensures_selection/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__contracts__arith_ensures_selection__main.omg__demote_mut_ref_to_shared__0d45ace92d/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `// policy into the ensures evaluation.`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 9. `tests/omega/pass/contracts/distance_conjunction/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__contracts__distance_conjunction__main.omg__demote_mut_ref_to_shared__611c9c9c51/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `// length alike.`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 10. `tests/omega/pass/domains/call_requires_scalar_member_expression_from_domain_fact/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__call_requires_scalar_member_expression_from_domain_fact__main.omg__demote_mut_ref_to_shared__61e37b9dd6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `self.length > 0;`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&mut T` | Exclusive observation and mutation.

### 11. `tests/omega/pass/domains/domain_subslice_carry/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine Main::relay(main: &mut Main, bytes: &[u8] in Ascii) {`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 12. `tests/omega/pass/domains/zero_array_inactive_payload_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__zero_array_inactive_payload_exit__main.omg__promote_shared_ref_to_mut__b3ea9fe1b6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `transition consume(&mut frames) == 0 { true -> done() false -> bad() }`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

### 13. `tests/omega/pass/facts/captured_window_slot_retirement_through_other_machine_exit/main.omg` — `tighten_requires_equality`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__captured_window_slot_retirement_through_other_machine_exit__main.omg__tighten_requires_equality__0c82318547/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `requires windows.len == 3 && slot == 0 {`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation*:

> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### 14. `tests/omega/pass/facts/field_where_index_through_match_join_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__field_where_index_through_match_join_exit__main.omg__promote_shared_ref_to_mut__8cfa8adca8/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine joined(main: &mut Main, flag: bool) -> i32 in Wrapping {`

**wiki/spec/language/ownership.md** — *Borrows and aliases*:

> `&T` | Shared observation under its loan contract.

## Known gap: Q7 mut

Count: 103 (tracked migration; not counted as confirmed holes).

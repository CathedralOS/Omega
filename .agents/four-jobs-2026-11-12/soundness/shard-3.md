# Soundness mutations shard 3

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).
Shard selection: `(i % 8) == 3` over sorted `tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.

## Mutant buckets (each row is exactly one bucket)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 16 |
| `known-gap-q7-mut` | 141 |
| `disagreement-valid` | 38 |
| `valid-route-outside` | 110 |
| `discarded-valid` | 59 |
| `rejected-both` | 681 |
| `timeout` | 0 |
| `failed-run` | 0 |
| `discarded-no-rule` | 0 |
| **Total (mutants run)** | **1045** |

Bucket sum check: 1045 == 1045 (OK).

- Fixtures in shard: 1130
- Fixtures processed: 550
- Mutants generated: 1045

## Fixture reconcile

- Fixtures in shard: 1130
- Fixtures with ≥1 mutant row: 441
- Fixtures with no mutant rows (yet): 689
- Mutant rows total: 1045
- Avg mutants / mutated fixture: 2.37

### Fixtures with more than one mutant row

- `tests/omega/pass/arithmetic/anonymous_rational_arguments/main.omg`: 3 rows
- `tests/omega/pass/arithmetic/const_binder_bound_scales_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/match_operand_versions/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/offset_integer_at/main.omg`: 3 rows
- `tests/omega/pass/arithmetic/runtime_bitwise_high_ops_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_bounded_unsigned_sum_chain_exit/main.omg`: 3 rows
- `tests/omega/pass/arithmetic/runtime_dispatch_cast_decisions_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_explicit_call_result_policy_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_immutable_unsigned_bitwise_bounds_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_pinned_divide_input_across_call_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_shl_saturating_exit/main.omg`: 3 rows
- `tests/omega/pass/arithmetic/runtime_trapping_mutable_local_traps/main.omg`: 3 rows
- `tests/omega/pass/arithmetic/runtime_wrapping_operand_truncation_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/unsigned_computed_index/main.omg`: 2 rows
- `tests/omega/pass/arrays/qualified_array_literal/main.omg`: 2 rows
- `tests/omega/pass/asm/atomic_release_disposition_exit/main.omg`: 2 rows
- `tests/omega/pass/asm/checked_label_transition_exit/main.omg`: 2 rows
- `tests/omega/pass/atomics/runtime_atomic_case_field_state_exit/main.omg`: 2 rows
- `tests/omega/pass/atomics/runtime_atomic_operand_order_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/mutable_selector_rhs_snapshot_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_boundary_receiver_disjoint_loan_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_signed_element_empty_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_write_only_boundary_satisfier_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_alias_exit/main.omg`: 3 rows
- `tests/omega/pass/borrow/runtime_write_only_sum_copy_exit/main.omg`: 2 rows
- `tests/omega/pass/borrow/write_only_constrained_record_field/main.omg`: 4 rows
- `tests/omega/pass/borrows/borrowed_array_loop/main.omg`: 2 rows
- `tests/omega/pass/borrows/borrowed_sum_dispatch/main.omg`: 2 rows
- `tests/omega/pass/borrows/carried_loan_replacement/main.omg`: 3 rows
- `tests/omega/pass/borrows/completed_reference_result_preserves_input_holds_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/copied_empty_case_before_active_replacement_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/data_copy_property/main.omg`: 2 rows
- `tests/omega/pass/borrows/fixed_array_reference_backing_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/guard_subject_ends_before_returning_call_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/linear_call_result_shared_observation_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/local_reference_record_transfer_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/nested_array_call_restores_memberships_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/nested_machine_unit_output_ordinary_forward_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/owned_reference_arrival_projection_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/projected_indexed_member_loan/main.omg`: 2 rows
- `tests/omega/pass/borrows/qualified_byte_owner_shared_call_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/reference_parameter_field_store_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/returned_record_byte_field_store_exit/main.omg`: 2 rows
- `tests/omega/pass/borrows/returned_slice_record_reference_custody_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_assignment_rhs_carrier_replacement_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_captured_record_nested_field_snapshot_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_case_reference_reordered_state_returns_exit/main.omg`: 3 rows
- `tests/omega/pass/borrows/runtime_computed_window_operand_order_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/runtime_copyable_carrier_retirement_exit/main.omg`: 3 rows
- … and 320 more

## Mutant total accounting

Primary pass recorded 840 mutants before `drop_requires_line` was re-enabled; backfill adds only missing `drop_requires_line` rows without deleting prior mutants.

### Confirmed-hole by mutation kind

- `drop_requires_line`: 7
- `promote_shared_ref_to_mut`: 6
- `demote_mut_ref_to_shared`: 3

### Confirmed-hole by accepted_by

- `both`: 7
- `route`: 9

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg	current=accepted	current_us=1468154	root=compilation	route=no-entry
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg	current=accepted	current_us=1571747	root=compilation	route=no-entry
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_state_boolean_reference_exit__main.omg__promote_shared_ref_to_mut__312788ebc8/main.omg	current=accepted	current_us=11321174	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg	current=accepted	current_us=3056745	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (5 confirmed-hole) 5: `tests/omega/pass/calls/runtime_dispatch_result_enum_case_exit/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: `main.result = count(main, 3, 0, 0);`
- Route reason (hint only): ``
- Validity note: `write through parameter `main` after demoting &mut to &`
- Accepted by: `route`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/ownership.md` (Borrows and aliases):

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_dispatch_result_enum_case_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_dispatch_result_enum_case_exit__main.omg__demote_mut_ref_to_shared__67855d45da/main.omg	current=accepted	current_us=11233407	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_dispatch_cast_decisions_exit__main.omg__demote_mut_ref_to_shared__a93dc59de0/main.omg	current=accepted	current_us=11309312	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_trapping_mutable_local_traps__main.omg__demote_mut_ref_to_shared__39ceac4305/main.omg	current=accepted	current_us=10959125	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__build__segment_selector_reload__main.omg__remove_mut_binding__72027e613e/main.omg	current=accepted	current_us=10553647	root=compilation	route=outside	Unsupported: declaration Build selection that is not a machine or a data type
```

## Hand verification (5 discarded drop_requires)

### ## Hand verification (5 discarded drop_requires) 1: `tests/omega/pass/calls/native_code_layout_uncalled_boundary_exit/main.omg` / `drop_requires_line`

- Obligation / note: `transition layout.regions.len > 0 { true -> region(layout) _ -> false }`
- Route reason (hint only): `route=unknown`
- Validity note: `mutant retains explicit crash/bad route`
- Accepted by: `both`
- Disposition: `discarded-valid` (runtime-trap-present)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__native_code_layout_uncalled_boundary_exit__main.omg__drop_requires_line__bab23fb377/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__native_code_layout_uncalled_boundary_exit__main.omg__drop_requires_line__bab23fb377/main.omg	current=accepted	current_us=3133472	root=compilation	route=outside	Unsupported: declaration Build selection that is not a machine or a data type
```

### ## Hand verification (5 discarded drop_requires) 2: `tests/omega/pass/calls/runtime_constructor_projected_membership_snapshot_exit/main.omg` / `drop_requires_line`

- Obligation / note: `ensures result in Ticket::Issued {`
- Route reason (hint only): `route=unknown`
- Validity note: `mutant retains explicit crash/bad route`
- Accepted by: `both`
- Disposition: `discarded-valid` (runtime-trap-present)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_constructor_projected_membership_snapshot_exit__main.omg__drop_requires_line__71b2f598e3/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_constructor_projected_membership_snapshot_exit__main.omg__drop_requires_line__71b2f598e3/main.omg	current=accepted	current_us=3061110	root=compilation	route=outside	Unsupported: machine attached data
```

### ## Hand verification (5 discarded drop_requires) 3: `tests/omega/pass/facts/float_range_delivery_through_other_machine_exit/main.omg` / `drop_requires_line`

- Obligation / note: `let low: Reading = Reading { value: 0.125 };`
- Route reason (hint only): `route=unknown`
- Validity note: `mutant retains explicit crash/bad route`
- Accepted by: `both`
- Disposition: `discarded-valid` (runtime-trap-present)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__float_range_delivery_through_other_machine_exit__main.omg__drop_requires_line__08afd9167e/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__float_range_delivery_through_other_machine_exit__main.omg__drop_requires_line__08afd9167e/main.omg	current=accepted	current_us=2979026	root=compilation	route=outside	Unsupported: type float
```

### ## Hand verification (5 discarded drop_requires) 4: `tests/omega/pass/facts/generic_comparison_body_through_other_state_exit/main.omg` / `drop_requires_line`

- Obligation / note: `let direct: bool = compare<u64>(left, right);`
- Route reason (hint only): `route=unknown`
- Validity note: `mutant retains explicit crash/bad route`
- Accepted by: `both`
- Disposition: `discarded-valid` (runtime-trap-present)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__generic_comparison_body_through_other_state_exit__main.omg__drop_requires_line__6212920103/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__generic_comparison_body_through_other_state_exit__main.omg__drop_requires_line__6212920103/main.omg	current=accepted	current_us=3056877	root=compilation	route=outside	Unsupported: machine generic parameters
```

### ## Hand verification (5 discarded drop_requires) 5: `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_parameter_exit/main.omg` / `drop_requires_line`

- Obligation / note: `transition reading >= 1.25f32 && reading <= 2.75f32 {`
- Route reason (hint only): `route=unknown`
- Validity note: `mutant retains explicit crash/bad route`
- Accepted by: `both`
- Disposition: `discarded-valid` (runtime-trap-present)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/state_contracts.md` (Machine requirements at invocation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__ieee_float_32bit_ordered_bound_through_parameter_exit__main.omg__drop_requires_line__3e2f08941b/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__ieee_float_32bit_ordered_bound_through_parameter_exit__main.omg__drop_requires_line__3e2f08941b/main.omg	current=accepted	current_us=2968370	root=compilation	route=outside	Unsupported: type float
```

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__offset_integer_at__main.omg__remove_mut_binding__bf6499144d/main.omg	current=accepted	current_us=2255973	root=compilation	route=no-entry
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_explicit_call_result_policy_exit__main.omg__remove_mut_binding__3ec05424e1/main.omg	current=accepted	current_us=2902890	root=compilation	route=outside	Unsupported: type constrained
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__asm__checked_countdown_literal_exit__main.omg__remove_mut_binding__8a61b2d28e/main.omg	current=accepted	current_us=2751367	root=compilation	route=outside	Unsupported: machine assembly target
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__runtime_atomic_operand_order_exit__main.omg__remove_mut_binding__8536509326/main.omg	current=accepted	current_us=3038514	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__mutable_selector_rhs_snapshot_exit__main.omg__remove_mut_binding__1e4d43208f/main.omg	current=accepted	current_us=3041898	root=compilation	route=outside	Unsupported: machine generic parameters
```

## Confirmed-hole entries

### 1. `tests/omega/pass/arithmetic/match_operand_versions/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__match_operand_versions__main.omg__promote_shared_ref_to_mut__ccb75225c6/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `machine change(cell: &mut Cell) -> bool { cell.value = 2; true }`
- Accepted by: **both**
- Validity note: `write through `cell` after promoting & to &mut`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- machine stable(cell: &Cell, flag: bool) -> u32 {
+ machine stable(cell: &mut Cell, flag: bool) -> u32 {
```

### 2. `tests/omega/pass/borrows/borrowed_array_loop/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__borrowed_array_loop__main.omg__promote_shared_ref_to_mut__f4cc0f530c/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `out[position] = value;`
- Accepted by: **both**
- Validity note: `write through parameter `out` after demoting &mut to &`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- // `out: &mut [u8; 17]` forwards through the fill/write_digit loop and
+ // `out: &mut mut [u8; 17]` forwards through the fill/write_digit loop and
```

### 3. `tests/omega/pass/borrows/runtime_state_boolean_reference_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_state_boolean_reference_exit__main.omg__promote_shared_ref_to_mut__312788ebc8/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.ready = true;`
- Accepted by: **route**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- machine Main::selected(main: &Main) -> &bool {
+ machine Main::selected(main: &mut Main) -> &bool {
```

### 4. `tests/omega/pass/borrows/suspension_projected_call_view_shared_cohort_restore_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__suspension_projected_call_view_shared_cohort_restore_exit__main.omg__demote_mut_ref_to_shared__15b1f74b6d/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `source.right = 19;`
- Accepted by: **both**
- Validity note: `write through parameter `source` after demoting &mut to &`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&mut T` | Exclusive observation and mutation.

**Minimal mutant diff:**

```diff
- machine project(source: &mut Backing) -> &[i32; 1] { &source.values }
+ machine project(source: &Backing) -> &[i32; 1] { &source.values }
```

### 5. `tests/omega/pass/calls/runtime_dispatch_result_enum_case_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_dispatch_result_enum_case_exit__main.omg__demote_mut_ref_to_shared__67855d45da/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.result = count(main, 3, 0, 0);`
- Accepted by: **route**
- Validity note: `write through parameter `main` after demoting &mut to &`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&mut T` | Exclusive observation and mutation.

**Minimal mutant diff:**

```diff
- machine count(main: &mut Main, i: u64, j: u64, acc: i32 in Wrapping) -> Verdict
+ machine count(main: &Main, i: u64, j: u64, acc: i32 in Wrapping) -> Verdict
```

### 6. `tests/omega/pass/collections/mutable_slice_view_establishment/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__collections__mutable_slice_view_establishment__main.omg__promote_shared_ref_to_mut__aeaf8f6fe5/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `transition main.bag.cells[2] == 7 {`
- Accepted by: **route**
- Validity note: `write through parameter `cells` after demoting &mut to &`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- // Mutable element-view establishment: `let cells: &mut [i32] =
+ // Mutable element-view establishment: `let cells: &mut mut [i32] =
```

### 7. `tests/omega/pass/domains/domain_subslice_carry/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__domains__domain_subslice_carry__main.omg__promote_shared_ref_to_mut__f35ddb9404/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.end = 4;`
- Accepted by: **both**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- machine Main::relay(main: &Main, bytes: &[u8] in Ascii) {
+ machine Main::relay(main: &mut Main, bytes: &[u8] in Ascii) {
```

### 8. `tests/omega/pass/facts/darwin_stat_abi_through_parameter_exit/main.omg` — `demote_mut_ref_to_shared`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__darwin_stat_abi_through_parameter_exit__main.omg__demote_mut_ref_to_shared__f34fac3343/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `value.dev == -2147483647i32 && value.mode == 33188u16 && value.nlink == 3u16 && value.ino == 4294967298u64 && value.uid == 123u32 && value.gid == 456u32 && value.rdev == -123456789i32 && value.atime.seconds == -17i64 && value.atime.nanoseconds == 123456789i64 && value.mtime.seconds == 42i64 && value.mtime.nanoseconds == 987654321i64 && value.ctime.seconds == -9i64 && value.ctime.nanoseconds == 111111111i64 && value.btime.seconds == 77i64 && value.btime.nanoseconds == 222222222i64 && value.size == 8589934593i64 && value.blocks == 123456i64 && value.blksize == 4096i32 && value.flags == 2147483649u32 && value.generation == 2882400001u32 && value.reserved == -314159i32 && value.spare[0] == -4294967297i64 && value.spare[1] == 68719476737i64 && StatExchange::check(&value) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.rdev)) == 4171510507u64`
- Accepted by: **route**
- Validity note: `write through parameter `value` after demoting &mut to &`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&mut T` | Exclusive observation and mutation.

**Minimal mutant diff:**

```diff
-     machine fill(value: &mut DarwinStat) reaches;
+     machine fill(value: &DarwinStat) reaches;
```

### 9. `tests/omega/pass/facts/field_where_index_through_match_join_exit/main.omg` — `promote_shared_ref_to_mut`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__field_where_index_through_match_join_exit__main.omg__promote_shared_ref_to_mut__8cfa8adca8/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `main.arr = [10, 20, 30, 40, 50];`
- Accepted by: **both**
- Validity note: `write through `main` after promoting & to &mut`
- Route reason (hint only): ``

**wiki/spec/language/ownership.md** — *Borrows and aliases* (check-time proof obligation):

> `&T` | Shared observation under its loan contract.

**Minimal mutant diff:**

```diff
- machine joined(main: &Main, flag: bool) -> i32 in Wrapping {
+ machine joined(main: &mut Main, flag: bool) -> i32 in Wrapping {
```

### 10. `tests/omega/pass/arithmetic/anonymous_rational_arguments/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__anonymous_rational_arguments__main.omg__drop_requires_line__69e0e6d4ab/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `ensures result == value`
- Accepted by: **both**
- Validity note: `ensures still references facts from removed requires`
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
- requires value == 7
+ {}
- {}
+ 
- 
+ machine bounded_argument()
- machine bounded_argument()
+ {
- {
+     bounded((7 / 2 * 2) as i32);
-     bounded((7 / 2 * 2) as i32);
+     bounded((18446744073709551615 + 1) - 18446744073709551609);
-     bounded((18446744073709551615 + 1) - 18446744073709551609);
+ }
- }
+ 
- 
+ machine named_argument() -> i32
- machine named_argument() -> i32
+ {
- {
+     transition { _ -> finish((7 / 2 * 2) as i32) }
-     transition { _ -> finish((7 / 2 * 2) as i32) }
+     machine finish(value: i32) -> i32
-     machine finish(value: i32) -> i32
+         requires 7 <= value && value <= 7
-         requires 7 <= value && value <= 7
+     { value }
-     { value }
+ }
- }
```

### 11. `tests/omega/pass/borrows/runtime_captured_record_nested_field_snapshot_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_captured_record_nested_field_snapshot_exit__main.omg__drop_requires_line__18000eab8a/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.items[0].prefix = 3;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
- requires value >= 0 && value <= 100
+ {}
- {}
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() reaches Console {
- machine entry_start() reaches Console {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     Main::main(&mut entry_storage);
-     Main::main(&mut entry_storage);
+ }
- }
```

### 12. `tests/omega/pass/borrows/runtime_reference_record_array_field_read_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_reference_record_array_field_read_exit__main.omg__drop_requires_line__df3571292d/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.first.value = 70;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
- requires value >= 0 && value <= 100
+ {}
- {}
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() reaches Console {
- machine entry_start() reaches Console {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     Main::main(&mut entry_storage);
-     Main::main(&mut entry_storage);
+ }
- }
```

### 13. `tests/omega/pass/borrows/runtime_shared_reference_array_cohort_restoration_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_shared_reference_array_cohort_restoration_exit__main.omg__drop_requires_line__c9b39e4ce8/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.cells.first.value = 11;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires value >= 0 && value <= 100``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
- requires value >= 0 && value <= 100
+ {}
- {}
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() reaches Console {
- machine entry_start() reaches Console {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     Main::main(&mut entry_storage);
-     Main::main(&mut entry_storage);
+ }
- }
```

### 14. `tests/omega/pass/borrows/runtime_stored_reference_case_residual_retirement_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_stored_reference_case_residual_retirement_exit__main.omg__drop_requires_line__ab28cd8fe6/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `main.first.value = 70;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires observed >= 0 && observed <= 100``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
-     requires observed >= 0 && observed <= 100
+     requires second >= 0 && second <= 100
-     requires second >= 0 && second <= 100
+     {
-     {
+         transition observed == 70 && stamp == 29 && payload == 13 && second == 11 && main.first.value == 81 {
-         transition observed == 70 && stamp == 29 && payload == 13 && second == 11 && main.first.value == 81 {
+             true -> good(main)
-             true -> good(main)
+             _ -> bad(main)
-             _ -> bad(main)
+         }
-         }
+     }
-     }
+     machine good(main: &mut Main) { Console::exit_process(70); }
-     machine good(main: &mut Main) { Console::exit_process(70); }
+     machine bad(main: &mut Main) { Console::exit_process(71); }
-     machine bad(main: &mut Main) { Console::exit_process(71); }
+ }
- }
+ 
- 
+ machine require_cell_value(value: i32)
- machine require_cell_value(value: i32)
+ requires value >= 0 && value <= 100
- requires value >= 0 && value <= 100
+ {}
- {}
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() reaches Console {
- machine entry_start() reaches Console {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     Main::main(&mut entry_storage);
-     Main::main(&mut entry_storage);
+ }
```

### 15. `tests/omega/pass/borrows/state_element_clone/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__state_element_clone__main.omg__drop_requires_line__8b28b3a947/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `transition position < 2 {`
- Accepted by: **both**
- Validity note: `body uses facts from removed `requires position < 2``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
-         requires position < 2
+     {
-     {
+         copied.values[position] = source.values[position];
-         copied.values[position] = source.values[position];
+         transition { _ -> each(copied, source, position + 1) }
-         transition { _ -> each(copied, source, position + 1) }
+     }
-     }
+ }
- }
+ machine Worker::copy(source: &Cell) -> Cell {
- machine Worker::copy(source: &Cell) -> Cell {
+     Cell { values: source.values, count: source.count }
-     Cell { values: source.values, count: source.count }
+ }
- }
+ data Main { cell: Cell; }
- data Main { cell: Cell; }
+ machine Main::main(main: &mut Main) {
- machine Main::main(main: &mut Main) {
+     main.cell.count = 2;
-     main.cell.count = 2;
+     main.cell.values[0] = 35;
-     main.cell.values[0] = 35;
+     main.cell.values[1] = 70;
-     main.cell.values[1] = 70;
+     let cloned: Cell = Worker::clone(&main.cell);
-     let cloned: Cell = Worker::clone(&main.cell);
+ }
- }
```

### 16. `tests/omega/pass/calls/runtime_mutable_call_empty_input_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_mutable_call_empty_input_exit__main.omg__drop_requires_line__6757e042ea/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `loader.tag = loader.tags[index] as u8;`
- Accepted by: **route**
- Validity note: `body uses facts from removed `requires index <= 16383``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

**Minimal mutant diff:**

```diff
-     requires index <= 16383
+     {
-     {
+         // `tags`/`firsts` are `[u8; N] in Trapping`, so an element reads as
-         // `tags`/`firsts` are `[u8; N] in Trapping`, so an element reads as
+         // `u8 in Trapping` while `tag`/`first` are bare `u8`. A typed value
-         // `u8 in Trapping` while `tag`/`first` are bare `u8`. A typed value
+         // keeps its policy across a store, so dropping `Trapping` is spelled
-         // keeps its policy across a store, so dropping `Trapping` is spelled
+         // rather than implicit. `starts`/`ends` need no cast: their elements
-         // rather than implicit. `starts`/`ends` need no cast: their elements
+         // are already `u64 in Wrapping`, which is what `start`/`end` hold.
-         // are already `u64 in Wrapping`, which is what `start`/`end` hold.
+         loader.tag = loader.tags[index] as u8;
-         loader.tag = loader.tags[index] as u8;
+         loader.first = loader.firsts[index] as u8;
-         loader.first = loader.firsts[index] as u8;
+         loader.start = loader.starts[index];
-         loader.start = loader.starts[index];
+         loader.end = loader.ends[index];
-         loader.end = loader.ends[index];
+     }
-     }
+ 
- 
+     machine absent(loader: &mut Loader) {
-     machine absent(loader: &mut Loader) {
+         loader.tag = 0;
-         loader.tag = 0;
+         loader.first = 0;
-         loader.first = 0;
+         loader.start = 0;
-         loader.start = 0;
+         loader.end = 0;
-         loader.end = 0;
+     }
-     }
+ }
- }
+ 
```

## Known gap: Q7 mut

Count: 141 (tracked migration; not counted as confirmed holes).

# Soundness mutations shard 3

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).
Shard selection: `(i % 8) == 3` over sorted `tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.

## Mutant buckets (each row is exactly one bucket)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 4 |
| `known-gap-q7-mut` | 261 |
| `disagreement-valid` | 83 |
| `valid-route-outside` | 635 |
| `rejected-current-route-outside` | 23 |
| `discarded-valid` | 246 |
| `rejected-both` | 1078 |
| `timeout` | 0 |
| `discarded-no-rule` | 0 |
| **Total (mutants run)** | **2330** |

Bucket sum check: 2330 == 2330 (OK).

## Bucket integrity

- `rejected-both` rows lacking `current=rejected` AND `route=rejected`: **0** (must be 0; route `stopped` / `outside` / `no-entry` are not rejections).
- `rejected-current-route-outside` (invalid + current rejects + route outside): **23**
- `valid-route-outside` (valid mutants only): **635**
- Machine-header `drop_requires` rows with both checker verdicts: **59** (bucket split: {'discarded-valid': 25, 'rejected-both': 31, 'valid-route-outside': 1, 'disagreement-valid': 2}).

- Fixtures in shard: 1130
- Fixtures processed: 1130
- Mutant rows: 2330
- TSV rows total (mutants + fixture ledger): 2488

## Fixture ledger (1130 fixtures)

- Fixtures in shard: **1130**
- Mutated fixtures (≥1 mutant row): **972**
- `no-mutation` ledger rows: **148**
- `original-not-accepted` ledger rows: **10**
- `failed-run` ledger rows: **0**
- Check: 1130 == 972 + 148 + 10 + 0 (OK)

- `drop_requires` mutant rows: **556**

## Fixture reconcile

- Fixtures in shard: 1130
- Fixtures with ≥1 mutant row: 972
- Fixtures with no mutant rows: 148
- Mutant rows total: 2330
- Avg mutants / mutated fixture: 2.40

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
- `tests/omega/pass/borrows/runtime_case_reference_reordered_state_returns_exit/main.omg`: 4 rows
- `tests/omega/pass/borrows/runtime_computed_window_operand_order_exit/main.omg`: 5 rows
- `tests/omega/pass/borrows/runtime_copyable_carrier_retirement_exit/main.omg`: 3 rows
- … and 669 more

## Mutant total accounting

Primary pass recorded 840 mutants before `drop_requires_line` was re-enabled; backfill adds only missing `drop_requires_line` rows without deleting prior mutants.

### Confirmed-hole by mutation kind

- `drop_requires_line`: 3
- `corrupt_literal_callee_argument`: 1

### Confirmed-hole by canonical `kind`

- `drop_requires`: 3
- `corrupt_literal_callee_argument`: 1

### Confirmed-hole by accepted_by

- `both`: 2
- `route`: 2

### Confirmed-hole by hole_subtype

- `n/a: modular-entry probe not applicable (literal vs callee requires)`: 1
- `probe-not-built`: 3

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

### ## Hand verification (5 confirmed-hole) 1: `tests/omega/pass/calls/runtime_mutable_call_empty_input_exit/main.omg` / `drop_requires_line`

- Obligation / note: `loader.tag = loader.tags[index] as u8;`
- Route reason (hint only): `route=unknown`
- Validity note: `body uses facts from removed `requires index <= 16383``
- Accepted by: `route`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` (Nested machines and identity; Private entry inference):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_mutable_call_empty_input_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_mutable_call_empty_input_exit__main.omg__drop_requires_line__6757e042ea/main.omg	current=accepted	current_us=11278408	root=compilation	route=outside	Unsupported: machine attached data
```

### ## Hand verification (5 confirmed-hole) 2: `tests/omega/pass/wire/library_reflection_delegated_exit/owner/main.omg` / `drop_requires_line`

- Obligation / note: `context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64;`
- Route reason (hint only): `route=unknown`
- Validity note: `body uses facts from removed `requires T == u64``
- Accepted by: `both`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` (Nested machines and identity; Private entry inference):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__wire__library_reflection_delegated_exit__owner__main.omg__drop_requires_line__eb94552a87/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__wire__library_reflection_delegated_exit__owner__main.omg__drop_requires_line__eb94552a87/main.omg	current=accepted	current_us=1677878	root=compilation	route=no-entry
```

### ## Hand verification (5 confirmed-hole) 3: `tests/omega/run/arithmetic/pinned_dividend_across_console_calls/main.omg` / `corrupt_literal_callee_argument`

- Obligation / note: `digits(0);`
- Route reason (hint only): `route=unknown`
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
cannot compile admitted project: checked compilation failed for package `pinned_dividend_across_console_calls` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__run__arithmetic__pinned_dividend_across_console_calls__main.omg__corrupt_literal_callee_argument__9642cd94d0/main.omg	current=accepted	current_us=11216249	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (5 confirmed-hole) 4: `tests/omega/pass/drops/runtime_indexed_nominal_cleanup_exit/main.omg` / `drop_requires_line`

- Obligation / note: `Console::write_byte(token.byte);`
- Route reason (hint only): `route=unknown`
- Validity note: `body uses facts from removed `requires token.ready``
- Accepted by: `both`
- Disposition: `confirmed-hole` (confirmed-hole)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` (Nested machines and identity; Private entry inference):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__drops__runtime_indexed_nominal_cleanup_exit__main.omg__drop_requires_line__91588d3666/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__drops__runtime_indexed_nominal_cleanup_exit__main.omg__drop_requires_line__91588d3666/main.omg	current=accepted	current_us=3250286	root=compilation	route=outside	Unsupported: machine service reach
```

## Hand verification (disagreement-valid sample)

### ## Hand verification (disagreement-valid sample) 1: `tests/omega/pass/arithmetic/runtime_dispatch_cast_decisions_exit/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no shared caller to &mut param and no write through demoted &mut param`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_dispatch_cast_decisions_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_dispatch_cast_decisions_exit__main.omg__demote_mut_ref_to_shared__a93dc59de0/main.omg	current=accepted	current_us=11093512	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (disagreement-valid sample) 2: `tests/omega/pass/arithmetic/runtime_trapping_mutable_local_traps/main.omg` / `demote_mut_ref_to_shared`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no shared caller to &mut param and no write through demoted &mut param`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> `&mut T` | Exclusive observation and mutation.

**omega --check** (stdout/stderr head):

```text
(empty)
warning: trapping arithmetic in machine `Main::main` state `main` assignment ALWAYS overflows `i32` -- this computation traps unconditionally at runtime (the trap is an effect and will fire even if the result is never used)
cannot compile admitted project: checked compilation failed for package `runtime_trapping_mutable_local_traps` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_trapping_mutable_local_traps__main.omg__demote_mut_ref_to_shared__39ceac4305/main.omg	current=accepted	current_us=11286930	root=compilation	route=outside	Unsupported: machine service reach
```

### ## Hand verification (disagreement-valid sample) 3: `tests/omega/pass/borrows/runtime_state_boolean_reference_exit/main.omg` / `promote_shared_ref_to_mut`

- Obligation / note: ``
- Route reason (hint only): ``
- Validity note: `no shared caller to &mut param and no write through demoted &mut param`
- Accepted by: ``
- Disposition: `disagreement-valid` (disagreement-valid)
- Current verdict: **rejected** (exit 1)
- Route verdict: **accepted**

**Spec** — `` ():

> `&T` | Shared observation under its loan contract.

**omega --check** (stdout/stderr head):

```text
(empty)
cannot compile admitted project: checked compilation failed for package `runtime_state_boolean_reference_exit` with 1 diagnostic(s)
  error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding; run `omega update` to review and accept it
```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__runtime_state_boolean_reference_exit__main.omg__promote_shared_ref_to_mut__312788ebc8/main.omg	current=accepted	current_us=11285418	root=compilation	route=outside	Unsupported: machine service reach
```

## Hand verification (5 discarded drop_requires)

### ## Hand verification (5 discarded drop_requires) 1: `tests/omega/pass/arithmetic/anonymous_rational_arguments/main.omg` / `drop_requires_line`

- Obligation / note: `removed `requires value == 7` | restated `machine identity(value: i32) -> i32``
- Route reason (hint only): `route=unknown`
- Validity note: `internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)`
- Accepted by: ``
- Disposition: `discarded-valid` (discarded-valid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `` ():

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__anonymous_rational_arguments__main.omg__drop_requires_line__69e0e6d4ab/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__anonymous_rational_arguments__main.omg__drop_requires_line__69e0e6d4ab/main.omg	current=accepted	current_us=1405450	root=compilation	route=no-entry
```

### ## Hand verification (5 discarded drop_requires) 2: `tests/omega/pass/borrows/state_element_clone/main.omg` / `drop_requires_line`

- Obligation / note: `removed `requires position < 2` | restated `// `-> Cell` result, `requires position < 2` on the storing state,``
- Route reason (hint only): `route=unknown`
- Validity note: `internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)`
- Accepted by: ``
- Disposition: `discarded-valid` (discarded-valid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `` ():

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__state_element_clone__main.omg__drop_requires_line__8b28b3a947/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrows__state_element_clone__main.omg__drop_requires_line__8b28b3a947/main.omg	current=accepted	current_us=1540668	root=compilation	route=no-entry
```

### ## Hand verification (5 discarded drop_requires) 3: `tests/omega/pass/calls/native_code_layout_uncalled_boundary_exit/main.omg` / `drop_requires_line`

- Obligation / note: `removed `requires layout.regions.len > 0` | restated `machine inspect(layout: &CodeLayout) -> bool``
- Route reason (hint only): `route=unknown`
- Validity note: `internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)`
- Accepted by: ``
- Disposition: `discarded-valid` (discarded-valid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `` ():

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__native_code_layout_uncalled_boundary_exit__main.omg__drop_requires_line__bab23fb377/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__native_code_layout_uncalled_boundary_exit__main.omg__drop_requires_line__bab23fb377/main.omg	current=accepted	current_us=3065149	root=compilation	route=outside	Unsupported: declaration Build selection that is not a machine or a data type
```

### ## Hand verification (5 discarded drop_requires) 4: `tests/omega/pass/calls/runtime_constructor_projected_membership_snapshot_exit/main.omg` / `drop_requires_line`

- Obligation / note: `removed `requires value.ticket in Ticket::Issued` | restated `machine Ticket::issue() -> Ticket``
- Route reason (hint only): `route=unknown`
- Validity note: `internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)`
- Accepted by: ``
- Disposition: `discarded-valid` (discarded-valid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `` ():

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_constructor_projected_membership_snapshot_exit__main.omg__drop_requires_line__71b2f598e3/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_constructor_projected_membership_snapshot_exit__main.omg__drop_requires_line__71b2f598e3/main.omg	current=accepted	current_us=3084750	root=compilation	route=outside	Unsupported: machine attached data
```

### ## Hand verification (5 discarded drop_requires) 5: `tests/omega/pass/facts/float_range_delivery_through_other_machine_exit/main.omg` / `drop_requires_line`

- Obligation / note: `removed `requires value >= 0.0 && value <= 0.5` | restated `machine admit(value: f64) -> f64``
- Route reason (hint only): `route=unknown`
- Validity note: `internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)`
- Accepted by: ``
- Disposition: `discarded-valid` (discarded-valid)
- Current verdict: **accepted** (exit 0)
- Route verdict: **accepted**

**Spec** — `` ():

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**omega --check** (stdout/stderr head):

```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__float_range_delivery_through_other_machine_exit__main.omg__drop_requires_line__08afd9167e/main.omg; wrote_output=false

```

**route_one_comparison** (first lines):

```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__facts__float_range_delivery_through_other_machine_exit__main.omg__drop_requires_line__08afd9167e/main.omg	current=accepted	current_us=3064110	root=compilation	route=outside	Unsupported: type float
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__offset_integer_at__main.omg__remove_mut_binding__bf6499144d/main.omg	current=accepted	current_us=2387398	root=compilation	route=no-entry
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__arithmetic__runtime_explicit_call_result_policy_exit__main.omg__remove_mut_binding__3ec05424e1/main.omg	current=accepted	current_us=2988701	root=compilation	route=outside	Unsupported: type constrained
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__asm__checked_countdown_literal_exit__main.omg__remove_mut_binding__8a61b2d28e/main.omg	current=accepted	current_us=2761018	root=compilation	route=outside	Unsupported: machine assembly target
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__atomics__runtime_atomic_operand_order_exit__main.omg__remove_mut_binding__8536509326/main.omg	current=accepted	current_us=2921813	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__borrow__mutable_selector_rhs_snapshot_exit__main.omg__remove_mut_binding__1e4d43208f/main.omg	current=accepted	current_us=3109069	root=compilation	route=outside	Unsupported: machine generic parameters
```

## Confirmed-hole table

| Fixture | kind | accepted_by | subtype | spec quote | minimal diff | no-trap evidence |
| --- | --- | --- | --- | --- | --- | --- |
| `tests/omega/pass/calls/runtime_mutable_call_empty_input_exit/main.omg` | `drop_requires` | `route` | `probe-not-built` | A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. | `-     requires index <= 16383 \n +     { \n -     { \n +         // `tags`/`firsts` are `[u8; N] in Trapping`, so an element reads as \n -         // `tags`/`firsts` are `[u8; N] in Trapping`, so an element reads as \n +         // `u8 in T` | no bad()/crash route; violation is compile-time proof obligation |
| `tests/omega/pass/wire/library_reflection_delegated_exit/owner/main.omg` | `drop_requires` | `both` | `probe-not-built` | A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. | `- requires T == u64 \n + { \n - { \n +     context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64; \n -     context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64; \n +    ` | no bad()/crash route; violation is compile-time proof obligation |
| `tests/omega/run/arithmetic/pinned_dividend_across_console_calls/main.omg` | `corrupt_literal_callee_argument` | `route` | `n/a: modular-entry probe not applicable (literal vs callee requires)` | Machine `requires` applies at every invocation, including terminal and recursive calls; every arrival establishes the target's requirements under actual argument substitution. | `-     digits(53); \n +     digits(0);` | no bad()/crash route; violation is compile-time proof obligation |
| `tests/omega/pass/drops/runtime_indexed_nominal_cleanup_exit/main.omg` | `drop_requires` | `both` | `probe-not-built` | A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check. | `- requires token.ready \n + reaches Console \n - reaches Console \n + { \n - { \n +     Console::write_byte(token.byte); \n -     Console::write_byte(token.byte); \n +     token.ready = false; \n -     token.ready = false; \n + } \n - } \n ` | no bad()/crash route; violation is compile-time proof obligation |

## Confirmed-hole entries

### 1. `tests/omega/pass/calls/runtime_mutable_call_empty_input_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__calls__runtime_mutable_call_empty_input_exit__main.omg__drop_requires_line__6757e042ea/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `loader.tag = loader.tags[index] as u8;`
- Accepted by: **route**
- Hole subtype: **probe-not-built**
- Validity note: `body uses facts from removed `requires index <= 16383``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md** — *Nested machines and identity; Private entry inference* (check-time proof obligation):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

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

### 2. `tests/omega/pass/wire/library_reflection_delegated_exit/owner/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__wire__library_reflection_delegated_exit__owner__main.omg__drop_requires_line__eb94552a87/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64;`
- Accepted by: **both**
- Hole subtype: **probe-not-built**
- Validity note: `body uses facts from removed `requires T == u64``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md** — *Nested machines and identity; Private entry inference* (check-time proof obligation):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**Minimal mutant diff:**

```diff
- requires T == u64
+ {
- {
+     context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64;
-     context.total = ((context.total as u64 in Wrapping) + (value as u64 in Wrapping)) as u64;
+     context.count = ((context.count as u64 in Wrapping) + (1u64 as u64 in Wrapping)) as u64;
-     context.count = ((context.count as u64 in Wrapping) + (1u64 as u64 in Wrapping)) as u64;
+ }
- }
```

### 3. `tests/omega/run/arithmetic/pinned_dividend_across_console_calls/main.omg` — `corrupt_literal_callee_argument`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__run__arithmetic__pinned_dividend_across_console_calls__main.omg__corrupt_literal_callee_argument__9642cd94d0/main.omg`
- Current: **rejected** | Route: **accepted**
- Obligation: `digits(0);`
- Accepted by: **route**
- Hole subtype: **n/a: modular-entry probe not applicable (literal vs callee requires)**
- Validity note: `literal argument disagrees with callee requires`
- Route reason (hint only): `route=unknown`

**wiki/spec/language/state_contracts.md** — *Machine requirements at invocation* (check-time proof obligation):

> Machine `requires` applies at every invocation, including terminal and recursive calls; every arrival establishes the target's requirements under actual argument substitution.

**Minimal mutant diff:**

```diff
-     digits(53);
+     digits(0);
```

### 4. `tests/omega/pass/drops/runtime_indexed_nominal_cleanup_exit/main.omg` — `drop_requires_line`

- Mutant: `/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-3/tests__omega__pass__drops__runtime_indexed_nominal_cleanup_exit__main.omg__drop_requires_line__91588d3666/main.omg`
- Current: **accepted** | Route: **accepted**
- Obligation: `Console::write_byte(token.byte);`
- Accepted by: **both**
- Hole subtype: **probe-not-built**
- Validity note: `body uses facts from removed `requires token.ready``
- Route reason (hint only): `route=unknown`

**wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md** — *Nested machines and identity; Private entry inference* (check-time proof obligation):

> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.

**Minimal mutant diff:**

```diff
- requires token.ready
+ reaches Console
- reaches Console
+ {
- {
+     Console::write_byte(token.byte);
-     Console::write_byte(token.byte);
+     token.ready = false;
-     token.ready = false;
+ }
- }
+ 
- 
+ machine outgoing() -> i32 reaches Console {
- machine outgoing() -> i32 reaches Console {
+     Console::write_byte(88);
-     Console::write_byte(88);
+     69
-     69
+ }
- }
+ 
- 
+ // Capture X before disposing local G, then parameter F.
- // Capture X before disposing local G, then parameter F.
+ // Reverse index-major cleanup visits pair[1].right D, pair[1].left C,
- // Reverse index-major cleanup visits pair[1].right D, pair[1].left C,
+ // then pair[0].right B. Grouping by field would wrongly produce DBC.
- // then pair[0].right B. Grouping by field would wrongly produce DBC.
+ // The successor prints E and only then disposes the transferred child A.
- // The successor prints E and only then disposes the transferred child A.
+ machine select(input: Bundle, marker: Token)
- machine select(input: Bundle, marker: Token)
+ requires input.pairs[0].left.ready, input.pairs[0].right.ready,
- requires input.pairs[0].left.ready, input.pairs[0].right.ready,
+     input.pairs[1].left.ready, input.pairs[1].right.ready, marker.ready
-     input.pairs[1].left.ready, input.pairs[1].right.ready, marker.ready
+ reaches Console
- reaches Console
+ {
```

## Known gap: Q7 mut

Count: 261 (tracked migration; not counted as confirmed holes).

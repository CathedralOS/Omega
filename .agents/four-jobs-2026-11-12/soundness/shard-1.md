# Soundness mutations — shard 1/8

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`. Triage follows **soundness_fix3** + **soundness_fix4**.

## Fixture ledger (sum = 1131 fixtures)

- `no-mutation` fixtures: **88**
- `failed-run` fixtures: **0**
- `mutated` fixtures (≥1 kind): **1043**
- **Total fixtures:** **1131**

## Mutant ledger

- Mutant rows in TSV: **3484**
- Rows with `mutation_kind=none`: **88**
- Classified mutant rows (bucket sum): **3484**

### Mutants generated per kind (row counts)

- `drop_requires`: **540**
- `mut_ref_instead_of_shared`: **747**
- `remove_guard_transition`: **671**
- `remove_mut`: **535**
- `shared_instead_of_mut`: **601**
- `widen_requires`: **302**

- `drop_requires` with **machine-header** site: **392**

### Mutants per fixture (non-`none`)

- Mean: **3.00**
- Fixtures with >1 mutant row: **923**

## Bucket totals (sum = mutant rows)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 35 |
| `known-gap-q7-mut` | 63 |
| `disagreement-valid` | 14 |
| `discarded-valid` | 51 |
| `valid-route-outside` | 1132 |
| `rejected-both` | 2101 |
| `no-mutation` | 88 |
| **Total** | **3484** |

### Confirmed-hole by mutation kind

- `drop_requires`: 13
- `widen_requires`: 22

### Confirmed-hole by accepted_by

- `both`: 1
- `current`: 34

### Confirmed-hole by subtype (drop_requires)

- `(none)`: 22
- `inferred-entry`: 13

## `known-gap-q7-mut` vs PR #2638

- Listed in PR #2638 Q7 audit: **2**
- Not in list (still same-binding write/`&mut`/`&write`): **61**

## Commands
```bash
python3 .agents/four-jobs-2026-11-12/soundness/run_shard1.py
python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard1.py
```

## Hand verification — confirmed-hole (5)

### `widen_requires` — `tests/omega/pass/arithmetic/exact_integer_cast_proven/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): weakened numeric bound in requires
- Route hint (not validity): current=accepted current_us=3200378 root=compilation route=outside Unsupported: expression cast into a non-exact arithmetic domain
- Obligation: `requires 0 <= value && value <= 255 -> requires 0 <= value && value <= 254`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/exact_integer_cast_proven_28e4be0d2f27__widen_requires/main.omg	current=accepted	current_us=3219239	root=compilation	route=outside	Unsupported: expression cast into a non-exact arithmetic domain
```

### `widen_requires` — `tests/omega/pass/arrays/subject_field_where_index/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): weakened numeric bound in requires
- Route hint (not validity): current=accepted current_us=1629763 root=compilation route=no-entry
- Obligation: `requires 0 <= k, k <= 3 { -> requires 0 <= k, k <= 2 {`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/subject_field_where_index_b5954b92f642__widen_requires/main.omg	current=accepted	current_us=1647671	root=compilation	route=no-entry
```

### `widen_requires` — `tests/omega/pass/borrow/stored_write_only_metadata/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): weakened numeric bound in requires
- Route hint (not validity): current=accepted current_us=3185349 root=compilation route=outside Unsupported: type reference
- Obligation: `requires position <= 1 { -> requires position <= 0 {`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/stored_write_only_metadata_6df4301681ef__widen_requires/main.omg	current=accepted	current_us=3135778	root=compilation	route=outside	Unsupported: type reference
```

### `drop_requires` — `tests/omega/pass/facts/affine_discard_root_cleanup_through_parameter_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): ensures remains after dropping requires `resource.valid == 1u64` (modular entry; callers cannot restate)
- Route hint (not validity): current=accepted current_us=3068352 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `ensures result <= 50u64`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/affine_discard_root_cleanup_through_parameter_ex_4dbe16cfb039__drop_requires/main.omg	current=accepted	current_us=3139527	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/applied_requirement_indices_through_local_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): ensures remains after dropping requires `value in Tagged<Value>` (modular entry; callers cannot restate)
- Route hint (not validity): current=accepted current_us=4764879 root=compilation route=outside Unsupported: machine service reach
- Obligation: `ensures result in Tagged<Word>`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/applied_requirement_indices_through_local_exit_dd1f59ad5348__drop_requires/main.omg	current=accepted	current_us=4746396	root=compilation	route=outside	Unsupported: machine service reach
```

## Hand verification — discarded-valid (5)

### `drop_requires` — `tests/omega/pass/calls/runtime_authored_numeric_names_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): valid: no ensures/body line needs dropped requires
- Route hint (not validity): 
- Obligation: ``
- Spec (): > 
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_authored_numeric_names_exit_01523b5da6ce__drop_requires/main.omg	current=accepted	current_us=3160124	root=compilation	route=accepted	route_us=1530	discharge_us=1005	reverify_us=244	reverified=4	obligations=4	judged=4	core_refused=0	unproved=0	refused_other=0	needed_re
```

### `drop_requires` — `tests/omega/pass/drops/runtime_projected_nominal_core_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3018022 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `input.pairs[1].left.ready, input.pairs[1].right.ready, marker.ready`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_projected_nominal_core_exit_d680f7a8ee20__drop_requires/main.omg	current=accepted	current_us=3107182	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_state_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3192193 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `let first: &mut Cell = &mut values[first_index];`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/checked_predicate_disjoint_loans_through_other_s_a97c9dfe2420__drop_requires/main.omg	current=accepted	current_us=3106429	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_parameter_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3012957 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `let selected: &mut [i64] = match choose(visits) {`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/descriptor_range_exclusive_loan_through_paramete_d0e88e63a04d__drop_requires/main.omg	current=accepted	current_us=3068494	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/double_indexed_rmw_through_other_state_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3052366 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `grid[1][2] = 41;`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/double_indexed_rmw_through_other_state_exit_8be82599b175__drop_requires/main.omg	current=accepted	current_us=3123191	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

## Hand verification — discarded-valid drop_requires (5)

### `drop_requires` — `tests/omega/pass/calls/runtime_authored_numeric_names_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): valid: no ensures/body line needs dropped requires
- Route hint (not validity): 
- Obligation: ``
- Spec (): > 
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_authored_numeric_names_exit_01523b5da6ce__drop_requires/main.omg	current=accepted	current_us=3140410	root=compilation	route=accepted	route_us=1239	discharge_us=722	reverify_us=227	reverified=4	obligations=4	judged=4	core_refused=0	unproved=0	refused_other=0	needed_rel
```

### `drop_requires` — `tests/omega/pass/drops/runtime_projected_nominal_core_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3018022 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `input.pairs[1].left.ready, input.pairs[1].right.ready, marker.ready`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_projected_nominal_core_exit_d680f7a8ee20__drop_requires/main.omg	current=accepted	current_us=3156596	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_state_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3192193 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `let first: &mut Cell = &mut values[first_index];`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/checked_predicate_disjoint_loans_through_other_s_a97c9dfe2420__drop_requires/main.omg	current=accepted	current_us=3175019	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_parameter_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3012957 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `let selected: &mut [i64] = match choose(visits) {`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/descriptor_range_exclusive_loan_through_paramete_d0e88e63a04d__drop_requires/main.omg	current=accepted	current_us=3096859	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `drop_requires` — `tests/omega/pass/facts/double_indexed_rmw_through_other_state_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: ``
- Validity (read mutant): body still needs entry fact; modular entry rule (fix5)
- Route hint (not validity): current=accepted current_us=3052366 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `grid[1][2] = 41;`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/double_indexed_rmw_through_other_state_exit_8be82599b175__drop_requires/main.omg	current=accepted	current_us=3245174	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

## Hand verification — known-gap-q7-mut (5)

### `mut_ref_instead_of_shared` — `tests/omega/pass/ownership/shared_view_member_read_compile/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: `current`
- Validity (read mutant): mut_ref_instead_of_shared without writable authority
- Route hint (not validity): current=accepted current_us=1404756 root=compilation route=no-entry
- Obligation: `// A shared borrow lends the caller's claim read-only: `peek(&mutaccount)``
- Spec (wiki/spec/language/expressions.md): > Such a loan requires existing writable authority at the selected place;
- `omega --check` exit 0:
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/shared_view_member_read_compile_2e7a34a81afb__amp_to_ampmut/main.omg; wrote_output=false
```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/shared_view_member_read_compile_2e7a34a81afb__amp_to_ampmut/main.omg	current=accepted	current_us=1475429	root=compilation	route=no-entry
```

### `shared_instead_of_mut` — `tests/omega/pass/borrows/runtime_scalar_reads_shared_result_exit/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: `current`
- Validity (read mutant): shared_instead_of_mut removes writable loan where mutation is required
- Route hint (not validity): current=accepted current_us=3609861 root=compilation route=outside Unsupported: type reference
- Obligation: `machine attenuate(matrix: &mut [[u64 in Wrapping; 2]; 2]) -> &[u64 in Wrapping; 2] {`
- Spec (wiki/spec/language/ownership.md): > A non-`mut` binding does not prevent a permitted move; the recipient may bind its own value mutably.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_scalar_reads_shared_result_exit_91ce5e579d3d__shared_instead_of_mut/main.omg	current=accepted	current_us=3125376	root=compilation	route=outside	Unsupported: type reference
```

### `shared_instead_of_mut` — `tests/omega/pass/build/build_source_gen/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: `current`
- Validity (read mutant): shared_instead_of_mut removes writable loan where mutation is required
- Route hint (not validity): current=accepted current_us=2452081 root=compilation route=no-entry
- Obligation: `machine Main::main(main: &mut Main) { }`
- Spec (wiki/spec/language/ownership.md): > A non-`mut` binding does not prevent a permitted move; the recipient may bind its own value mutably.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/build_source_gen_953d0099675e__shared_instead_of_mut/main.omg	current=accepted	current_us=2305063	root=compilation	route=no-entry
```

### `shared_instead_of_mut` — `tests/omega/pass/calls/computed_binding_argument_exit/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: `current`
- Validity (read mutant): shared_instead_of_mut removes writable loan where mutation is required
- Route hint (not validity): current=accepted current_us=3606119 root=compilation route=outside Unsupported: machine service reach
- Obligation: `machine Main::main(main: &mut Main) reaches Character crashes Trap {`
- Spec (wiki/spec/language/ownership.md): > A non-`mut` binding does not prevent a permitted move; the recipient may bind its own value mutably.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/computed_binding_argument_exit_289ead8db374__shared_instead_of_mut/main.omg	current=accepted	current_us=3115794	root=compilation	route=outside	Unsupported: machine service reach
```

### `shared_instead_of_mut` — `tests/omega/pass/calls/requirement_declared_static_operands/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: `current`
- Validity (read mutant): shared_instead_of_mut removes writable loan where mutation is required
- Route hint (not validity): current=accepted current_us=1662530 root=compilation route=no-entry
- Obligation: `machine fill<const Value: i32>(destination: &mut Observation) reaches;`
- Spec (wiki/spec/language/ownership.md): > A non-`mut` binding does not prevent a permitted move; the recipient may bind its own value mutably.
- `omega --check` exit 0:
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/requirement_declared_static_operands_888b5459a046__shared_instead_of_mut/main.omg; wrote_output=false
```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/requirement_declared_static_operands_888b5459a046__shared_instead_of_mut/main.omg	current=accepted	current_us=1464453	root=compilation	route=no-entry
```

## In-machine restate (`valid-route-outside` / valid mutants): **26** rows

Rule: dropped `requires` is restated on an in-machine `transition`/`if` guard that dominates body access; callers outside the machine cannot restate.

### `tests/omega/pass/borrows/borrow_returned_window_write_and_call/main.omg` — `drop_requires`
- Dropped requires: `2 <= value, value <= 4`
- In-machine guard: `transition value <= 4 {`
- Bucket: `valid-route-outside`

### `tests/omega/pass/borrows/runtime_shared_qualified_view_cycle_exit/main.omg` — `drop_requires`
- Dropped requires: `i < path.len`
- In-machine guard: `transition i < path.len {`
- Bucket: `valid-route-outside`

### `tests/omega/pass/calls/native_code_layout_region_snapshot_exit/main.omg` — `drop_requires`
- Dropped requires: `layout.regions.len > 0`
- In-machine guard: `transition layout.regions.len > 0 {`
- Bucket: `valid-route-outside`

### `tests/omega/pass/collections/runtime_fixed_byte_view_dynamic_state_loan_exit/main.omg` — `drop_requires`
- Dropped requires: `selected < 2`
- In-machine guard: `transition selected < 2 {`
- Bucket: `valid-route-outside`

### `tests/omega/pass/control_flow/field_view_transfer_exit/main.omg` — `drop_requires`
- Dropped requires: `cells.len > 0`
- In-machine guard: `transition store.cells.len > 0 && store.names.len > 0 {`
- Bucket: `valid-route-outside`

## Confirmed-hole entries

### `widen_requires` — `tests/omega/pass/arithmetic/exact_integer_cast_proven/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/exact_integer_cast_proven_28e4be0d2f27__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=3200378 root=compilation route=outside Unsupported: expression cast into a non-exact arithmetic domain
- Obligation: `requires 0 <= value && value <= 255 -> requires 0 <= value && value <= 254`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -4,5 +4,5 @@\n \n machine narrow(value: i32) -> u8\n-requires 0 <= value && value <= 255\n+requires 0 <= value && value <= 254\n {\n     value as u8\n
```

### `widen_requires` — `tests/omega/pass/arrays/subject_field_where_index/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/subject_field_where_index_b5954b92f642__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1629763 root=compilation route=no-entry
- Obligation: `requires 0 <= k, k <= 3 { -> requires 0 <= k, k <= 2 {`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -4,5 +4,5 @@\n // facts — the literal store's exact value does not survive the\n // transition, only the where fact does.\n-data Main requires 0 <= k, k <= 3 {\n+data Main requires 0 <= k, k <= 2 {\n     arr: [i32 in Wrapping; 5];\n     k: u64;\n
```

### `widen_requires` — `tests/omega/pass/borrow/stored_write_only_metadata/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/stored_write_only_metadata_6df4301681ef__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=3185349 root=compilation route=outside Unsupported: type reference
- Obligation: `requires position <= 1 { -> requires position <= 0 {`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -1,3 +1,3 @@\n-machine extent(references: &[&write [i32; 2]; 2], position: u64) -> u64 requires position <= 1 {\n+machine extent(references: &[&write [i32; 2]; 2], position: u64) -> u64 requires position <= 0 {\n     references[position].len\n }\n
```

### `drop_requires` — `tests/omega/pass/facts/affine_discard_root_cleanup_through_parameter_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `drop_resource(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/affine_discard_root_cleanup_through_parameter_ex_4dbe16cfb039__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `resource.valid == 1u64` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3068352 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `ensures result <= 50u64`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -13,5 +13,4 @@\n \n machine drop_resource(resource: &mut Resource)\n-requires resource.valid == 1u64\n {\n     resource.dropped.value = resource.valid;\n
```

### `drop_requires` — `tests/omega/pass/facts/applied_requirement_indices_through_local_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `Identity<Word>::stated(Payload { value: 0 })` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/applied_requirement_indices_through_local_exit_dd1f59ad5348__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `value in Tagged<Value>` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=4764879 root=compilation route=outside Unsupported: machine service reach
- Obligation: `ensures result in Tagged<Word>`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -13,5 +13,4 @@\n     machine stated(value: Payload in Tagged<Value>) -> Payload in Tagged<Value>\n     reaches\n-    requires value in Tagged<Value>\n     ensures result in Tagged<Value>;\n }\n
```

### `drop_requires` — `tests/omega/pass/facts/content_reunion_through_const_name_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `rebuild_region(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/content_reunion_through_const_name_exit_f3c494656658__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `region.base <= 58 && region.length <= 58 - region.base` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3342616 root=compilation route=outside Unsupported: machine type equations, owned data or conformances
- Obligation: `ensures result.base == left.base, result.length == left.length,`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -27,5 +27,4 @@\n \n machine rebuild_region(region: Region in Owned) -> Region in Owned\n-requires region.base <= 58 && region.length <= 58 - region.base\n ensures result.base == region.base, result.length == region.length,\n     core::Identity<IntervalSet<Nat>>(Owned::content(old(&region)), Owned::content(&result))\n
```

### `drop_requires` — `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_call_result_exit/main.omg`
- accepted_by: **both**; subtype: **inferred-entry**
- Entry probe: violating call `step1(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/live_across_two_calls_preservation_bound_through_7ce00d05813d__drop_requires`
- Current: **accepted**; route: **accepted**
- Validity: ensures remains after dropping requires `x == 10` (modular entry; callers cannot restate)
- Route hint: 
- Obligation: `ensures result == 12`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,4 @@\n // Expected exit: 0\n machine step1(x: u64) -> u64\n-    requires x == 10\n     ensures result == 11\n {\n
```

### `drop_requires` — `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_target_body_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `step1(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/live_across_two_calls_preservation_bound_through_5878aa1fe2da__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `x == 10` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3273306 root=compilation route=outside Unsupported: machine service reach
- Obligation: `ensures result == 12`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,4 @@\n // Expected exit: 0\n machine step1(x: u64) -> u64\n-    requires x == 10\n     ensures result == 11\n {\n
```

### `drop_requires` — `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_state_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `need(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/provider_contract_endpoint_pending_call_through__ba2575e0fd2d__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `value <= independent_limit()` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=4372134 root=compilation route=outside Unsupported: machine invokes
- Obligation: `ensures result <= limit()`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -11,5 +11,4 @@\n }\n machine need(value: u64) -> u64 invokes Endpoint\n-requires value <= independent_limit()\n { value }\n machine limit() -> u64 {\n
```

### `drop_requires` — `tests/omega/pass/facts/qualified_field_capacity_through_match_join_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `check_length(0, 0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/qualified_field_capacity_through_match_join_exit_6586bbb4467f__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `tail.length == expected` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3081950 root=compilation route=outside Unsupported: type reference
- Obligation: `ensures result.rest.length == whole.length - length,`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -15,5 +15,4 @@\n data Issued { strategy: Strategy; allocation: Tail; }\n machine check_length(tail: &Tail, expected: u64)\n-requires tail.length == expected\n {}\n machine split(whole: Tail in Tracked & Partitionable, length: u64) -> Parts\n
```

### `drop_requires` — `tests/omega/pass/facts/returned_indexed_linear_residual_through_other_state_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `drop_receipt(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/returned_indexed_linear_residual_through_other_s_b68969c2151b__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `receipt.code == 7` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3043407 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `ensures result == 7`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -5,5 +5,4 @@\n data Receipt [linear] dropped by drop_receipt { code: u64; }\n machine drop_receipt(receipt: &mut Receipt)\n-requires receipt.code == 7\n {\n     receipt.code = 0;\n
```

### `drop_requires` — `tests/omega/pass/facts/returned_linear_residual_through_other_machine_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `drop_receipt(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/returned_linear_residual_through_other_machine_e_2d47059261d0__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `receipt.code == 7` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=3205782 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `ensures result == 7`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -5,5 +5,4 @@\n data Receipt [linear] dropped by drop_receipt { code: u64; }\n machine drop_receipt(receipt: &mut Receipt)\n-requires receipt.code == 7\n {\n     receipt.code = 0;\n
```

### `drop_requires` — `tests/omega/pass/facts/slice_view_element_read_bound_through_parameter_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `pass_through(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/slice_view_element_read_bound_through_parameter__151be8d95163__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: body still needs entry fact; modular entry rule (fix5)
- Route hint: current=accepted current_us=3031091 root=compilation route=outside Unsupported: type reference
- Obligation: `let v: &[u64] = pass_through(view);`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -6,5 +6,4 @@\n \n machine pass_through(param: &[u64]) -> &[u64]\n-    requires param.len >= 3\n     ensures result.len == param.len\n {\n
```

### `drop_requires` — `tests/omega/pass/providers/applied_type_index_forwarding_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `Identity<Word>::stated(Payload { value: 0 })` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/applied_type_index_forwarding_exit_fba26da3e6a6__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `value in Tagged<Value>` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=4638788 root=compilation route=outside Unsupported: machine service reach
- Obligation: `ensures result in Tagged<Word>`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -13,5 +13,4 @@\n     machine stated(value: Payload in Tagged<Value>) -> Payload in Tagged<Value>\n     reaches\n-    requires value in Tagged<Value>\n     ensures result in Tagged<Value>;\n }\n
```

### `widen_requires` — `tests/omega/pass/borrows/same_initializer_disjoint_source_move/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/same_initializer_disjoint_source_move_3420d10176af__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1505598 root=compilation route=no-entry
- Obligation: `requires slots >= 0, slots <= 100 { slots: i32; } -> requires slots >= 0, slots <= 99 { slots: i32; }`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -1,5 +1,5 @@\n // A transfer before a later sibling loan remains a valid authored order.\n // The loan and extraction use disjoint storage; the repair closes the hole.\n-data Inventory requires slots >= 0, slots <= 100 { slots: i32; }\n+data Inventory requires slots >= 0, slots <= 99 { slots: i32; }\n data Bundle { loan: &Inventory; taken: Inventory; }\n data Main { first: Inventory; second: Inventory; }\n
```

### `widen_requires` — `tests/omega/pass/cases/case_arm_pattern_names/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/case_arm_pattern_names_9ca332b2c670__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1694548 root=compilation route=no-entry
- Obligation: `requires 0 <= self && self <= 50; -> requires 0 <= self && self <= 49;`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -1,3 +1,3 @@\n-domain i32::Step requires 0 <= self && self <= 50;\n+domain i32::Step requires 0 <= self && self <= 49;\n \n data Msg \n
```

### `widen_requires` — `tests/omega/pass/facts/affine_discard_root_cleanup_through_parameter_exit/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/affine_discard_root_cleanup_through_parameter_ex_4dbe16cfb039__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=4740101 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 50u64 so 50u64 - result succeeds. -> requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 49u64 so 50u64 - result succeeds.`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -1,3 +1,3 @@\n-// Fact: an unconsumed nominal resource whose drop contract requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 50u64 so 50u64 - result succeeds.\n+// Fact: an unconsumed nominal resource whose drop contract requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 49u64 so 50u64 - result succeeds.\n // Expected exit: 0\n \n
```

### `widen_requires` — `tests/omega/pass/facts/array_element_default_domain/const_name/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/const_name_b7a89bd5a895__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=4967753 root=compilation route=outside Unsupported: type reference
- Obligation: `requires self.values[0] >= 0 && self.values[0] <= 16, -> requires self.values[0] >= 0 && self.values[0] <= 15,`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,5 @@\n \n public data Tallies [copy]\n-requires self.values[0] >= 0 && self.values[0] <= 16,\n+requires self.values[0] >= 0 && self.values[0] <= 15,\n     self.values[1] >= 0 && self.values[1] <= 16,\n     self.values[2] >= 0 && self.values[2] <= 16,\n
```

### `widen_requires` — `tests/omega/pass/facts/atomic_bounded_fetch/parameter/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/parameter_9cc700b2cef7__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=4690818 root=compilation route=outside Unsupported: type reference
- Obligation: `requires value <= 100 { value: AtomicU32; } -> requires value <= 99 { value: AtomicU32; }`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -2,5 +2,5 @@\n // A shared atomic fetch preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.\n use omega::language::core::atomic;\n-data Cell requires value <= 100 { value: AtomicU32; }\n+data Cell requires value <= 99 { value: AtomicU32; }\n machine mutate(cell: &Cell, mask: u32) -> u32 requires mask <= 63 {\n     let prior: u32 = cell.value.fetch_and(mask, NoOrdering);\n
```

### `widen_requires` — `tests/omega/pass/facts/finite_family_owned_field/other_state/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/other_state_7984ba57d5e8__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=6143368 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `requires self >= 100 && self <= 1000; -> requires self >= 100 && self <= 999;`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -5,5 +5,5 @@\n \n data Units {}\n-domain u64::StoredValue requires self >= 100 && self <= 1000;\n+domain u64::StoredValue requires self >= 100 && self <= 999;\n data Token [linear] dropped by drop_token { value: u64 in StoredValue; }\n machine drop_token(token: &mut Token) {}\n
```

### `widen_requires` — `tests/omega/pass/facts/linear_owner_borrowed_projection_local/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/linear_owner_borrowed_projection_local_4540c53a0d90__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=5471680 root=compilation route=outside Unsupported: contract contracts on a state
- Obligation: `requires self >= 0 && self <= 2147483647 && self != 42; -> requires self >= 0 && self <= 2147483646 && self != 42;`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,5 @@\n // Expected exit: 0. Both cleanup branches and both match branches execute.\n use omega::language::core::drop;\n-public domain u64::ResidentValue requires self >= 0 && self <= 2147483647 && self != 42;\n+public domain u64::ResidentValue requires self >= 0 && self <= 2147483646 && self != 42;\n public data Cell { value: u64 in ResidentValue; }\n public domain Cell::Issued established by mint_cell;\n
```

### `widen_requires` — `tests/omega/pass/facts/open_const_implementation_through_call_result_exit/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/open_const_implementation_through_call_result_ex_639d2ef75b14__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=5446537 root=compilation route=outside Unsupported: expression type parameter
- Obligation: `requires Base <= 100 -> requires Base <= 99`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -11,5 +11,5 @@\n { Implementation::read() }\n machine construct<const Base: u64>() -> u64\n-requires Base <= 100\n+requires Base <= 99\n ensures result >= Base + 4\n { selected<Base + 4, Selected<Base + 4>>() }\n
```

### `widen_requires` — `tests/omega/pass/facts/open_const_implementation_through_target_body_exit/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/open_const_implementation_through_target_body_ex_722dfc4cbcd0__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=5866553 root=compilation route=outside Unsupported: declaration Build selection that is not a machine or a data type
- Obligation: `requires Base <= 100 -> requires Base <= 99`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -11,5 +11,5 @@\n { Implementation::read() }\n machine construct<const Base: u64>() -> u64\n-requires Base <= 100\n+requires Base <= 99\n ensures result >= Base + 4\n { selected<Base + 4, Selected<Base + 4>>() }\n
```

### `drop_requires` — `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_other_state_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `need(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/provider_contract_endpoint_pending_call_through__ba2575e0fd2d__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: ensures remains after dropping requires `value <= independent_limit()` (modular entry; callers cannot restate)
- Route hint: current=accepted current_us=6054122 root=compilation route=outside Unsupported: machine invokes
- Obligation: `ensures result <= limit()`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -11,5 +11,4 @@\n }\n machine need(value: u64) -> u64 invokes Endpoint\n-requires value <= independent_limit()\n { value }\n machine limit() -> u64 {\n
```

### `widen_requires` — `tests/omega/pass/facts/skipped_short_circuit_call_through_call_result_exit/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/skipped_short_circuit_call_through_call_result_e_3b6d6e32c6fd__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=4823661 root=compilation route=outside Unsupported: parameter mutable parameter
- Obligation: `requires counter <= 1000u64 -> requires counter <= 999u64`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,5 @@\n // Expected exit: 0\n machine bump(counter: &mut u64) -> bool\n-requires counter <= 1000u64\n+requires counter <= 999u64\n {\n     counter = counter + 1u64;\n
```

### `drop_requires` — `tests/omega/pass/facts/slice_view_element_read_bound_through_parameter_exit/main.omg`
- accepted_by: **current**; subtype: **inferred-entry**
- Entry probe: violating call `pass_through(0)` rejected (exit 1)
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/slice_view_element_read_bound_through_parameter__151be8d95163__drop_requires`
- Current: **accepted**; route: **outside**
- Validity: body still needs entry fact; modular entry rule (fix5)
- Route hint: current=accepted current_us=5329411 root=compilation route=outside Unsupported: type reference
- Obligation: `let v: &[u64] = pass_through(view);`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -6,5 +6,4 @@\n \n machine pass_through(param: &[u64]) -> &[u64]\n-    requires param.len >= 3\n     ensures result.len == param.len\n {\n
```

### `widen_requires` — `tests/omega/pass/facts/skipped_short_circuit_call_through_target_body_exit/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/skipped_short_circuit_call_through_target_body_e_0bcb0dd2989c__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=5354573 root=compilation route=outside Unsupported: machine service reach
- Obligation: `requires counter <= 1000u64 -> requires counter <= 999u64`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -4,5 +4,5 @@\n \n machine bump(counter: &mut u64) -> bool\n-requires counter <= 1000u64\n+requires counter <= 999u64\n {\n     counter = counter + 1u64;\n
```

### `widen_requires` — `tests/omega/pass/generics/open_const_distinct_applications/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/open_const_distinct_applications_ea7f78c2efdc__widen_requires`
- Current: **accepted**; route: **outside**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=5774374 root=compilation route=outside Unsupported: expression type parameter
- Obligation: `requires Base <= 100 -> requires Base <= 99`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -11,5 +11,5 @@\n { Implementation::read() }\n machine construct<const Base: u64>() -> u64\n-requires Base <= 100\n+requires Base <= 99\n ensures result >= Base + 4\n { selected<Base + 4, Selected<Base + 4>>() }\n
```

### `widen_requires` — `tests/omega/pass/machines/hold_state_call/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/hold_state_call_4a05e088466f__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1834129 root=compilation route=no-entry
- Obligation: `requires remaining <= 9 -> requires remaining <= 8`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -10,5 +10,5 @@\n machine Main::count(main: &mut Main, remaining: u32) -> u32\n terminates by remaining in 0..=9\n-requires remaining <= 9\n+requires remaining <= 8\n {\n     transition remaining > 0 {\n
```

### `widen_requires` — `tests/omega/pass/ownership/nested_field_loan_compile/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/nested_field_loan_compile_9375130c5e12__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1830997 root=compilation route=no-entry
- Obligation: `requires inner.count <= 100 -> requires inner.count <= 99`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -12,5 +12,5 @@\n \n machine Main::bump(inner: &mut Inner)\n-requires inner.count <= 100\n+requires inner.count <= 99\n {\n     inner.count = inner.count + 1;\n
```

### `widen_requires` — `tests/omega/pass/termination/identity_measure_rank_range/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/identity_measure_rank_range_d245d8e9234e__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1780660 root=compilation route=no-entry
- Obligation: `requires 0 <= remaining && remaining <= 5 -> requires 0 <= remaining && remaining <= 4`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,5 @@\n \n machine walk(remaining: u64) -> u64\n-requires 0 <= remaining && remaining <= 5\n+requires 0 <= remaining && remaining <= 4\n terminates by remaining -> Countdown::Remaining in 0..=5 {\n     transition remaining > 0 {\n
```

### `widen_requires` — `tests/omega/pass/termination/joint_cycle_finish/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/joint_cycle_finish_6973d100654f__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1715819 root=compilation route=no-entry
- Obligation: `requires remaining <= 9 -> requires remaining <= 8`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -4,5 +4,5 @@\n machine left(remaining: u64) -> u64\n terminates by remaining in 0..=9\n-requires remaining <= 9\n+requires remaining <= 8\n {\n     transition remaining > 0 {\n
```

### `widen_requires` — `tests/omega/pass/termination/measure_field_rank_arithmetic/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/measure_field_rank_arithmetic_c3d2020cacd1__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1889141 root=compilation route=no-entry
- Obligation: `requires 0 <= remaining, remaining <= 5 { remaining: u64; } -> requires 0 <= remaining, remaining <= 4 { remaining: u64; }`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -1,3 +1,3 @@\n-data Countdown requires 0 <= remaining, remaining <= 5 { remaining: u64; }\n+data Countdown requires 0 <= remaining, remaining <= 4 { remaining: u64; }\n measure Countdown::Remaining(countdown: Countdown) -> u64\n { countdown.remaining }\n
```

### `widen_requires` — `tests/omega/pass/termination/measure_field_call_component/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/measure_field_call_component_d02d8f623e96__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1710013 root=compilation route=no-entry
- Obligation: `requires 0 <= remaining, remaining <= 5 { -> requires 0 <= remaining, remaining <= 4 {`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -3,5 +3,5 @@\n // coordinate of each member's record subject. The forward edge preserves the\n // record; the rebuilt literal strictly decreases the coordinate.\n-data Countdown requires 0 <= remaining, remaining <= 5 {\n+data Countdown requires 0 <= remaining, remaining <= 4 {\n     remaining: u64;\n }\n
```

### `widen_requires` — `tests/omega/pass/termination/measure_nested_projection_range_relational/main.omg`
- accepted_by: **current**; subtype: ****
- Entry probe: 
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-1/measure_nested_projection_range_relational_3e352c6ed1dc__widen_requires`
- Current: **accepted**; route: **no-entry**
- Validity: weakened numeric bound in requires
- Route hint: current=accepted current_us=1888447 root=compilation route=no-entry
- Obligation: `requires 0 <= remaining, remaining <= 5 { remaining: u64; } -> requires 0 <= remaining, remaining <= 4 { remaining: u64; }`
- Spec (wiki/spec/language/state_contracts.md): > Every arrival establishes the target's requirements and declared domains under actual argument substitution.
- Minimal diff:
```diff
--- original\n+++ mutant\n@@ -4,5 +4,5 @@\n // every self edge rebuilds the literal chain down to the ranked field while\n // the ceiling and the step stay pinned.\n-data Inner requires 0 <= remaining, remaining <= 5 { remaining: u64; }\n+data Inner requires 0 <= remaining, remaining <= 4 { remaining: u64; }\n data Countdown { label: u64; inner: Inner; }\n measure Countdown::Remaining(countdown: Countdown) -> u64\n
```


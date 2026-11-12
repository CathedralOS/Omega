# Soundness mutations — shard 1/8

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`. Triage follows **soundness_fix3** + **soundness_fix4**.

## Coverage reconciliation (fix4)

- Fixtures in shard: **1131**
- Mutant rows in TSV: **1131**
- `no-mutation` fixtures: **88**
- Fixtures with a mutation run: **1043**
- Mutants per fixture: **1** prioritized kind (or `none` row)

- Fixtures with >1 row: **0**

## Bucket totals (sum = mutant rows)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 9 |
| `disagreement-valid` | 2 |
| `discarded-valid` | 14 |
| `valid-route-outside` | 443 |
| `rejected-both` | 575 |
| `no-mutation` | 88 |
| **Total** | **1131** |

### Confirmed-hole by mutation kind

- `drop_requires`: 9

### Confirmed-hole by accepted_by

- `both`: 1
- `current`: 8

### Confirmed-hole by subtype (drop_requires)

- `inferred-entry`: 9

## `known-gap-q7-mut` vs PR #2638

- Listed in PR #2638 Q7 audit: **0**
- Not in list (still same-binding write/`&mut`/`&write`): **0**

## Commands
```bash
python3 .agents/four-jobs-2026-11-12/soundness/run_shard1.py
python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard1.py
```

## Hand verification — confirmed-hole (5)

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/affine_discard_root_cleanup_through_parameter_ex_4dbe16cfb039__drop_requires/main.omg	current=accepted	current_us=3334112	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/applied_requirement_indices_through_local_exit_dd1f59ad5348__drop_requires/main.omg	current=accepted	current_us=4831460	root=compilation	route=outside	Unsupported: machine service reach
```

### `drop_requires` — `tests/omega/pass/facts/content_reunion_through_const_name_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): ensures remains after dropping requires `region.base <= 58 && region.length <= 58 - region.base` (modular entry; callers cannot restate)
- Route hint (not validity): current=accepted current_us=3342616 root=compilation route=outside Unsupported: machine type equations, owned data or conformances
- Obligation: `ensures result.base == left.base, result.length == left.length,`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/content_reunion_through_const_name_exit_f3c494656658__drop_requires/main.omg	current=accepted	current_us=3328801	root=compilation	route=outside	Unsupported: machine type equations, owned data or conformances
```

### `drop_requires` — `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_call_result_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `both`
- Validity (read mutant): ensures remains after dropping requires `x == 10` (modular entry; callers cannot restate)
- Route hint (not validity): 
- Obligation: `ensures result == 12`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/live_across_two_calls_preservation_bound_through_7ce00d05813d__drop_requires/main.omg	current=accepted	current_us=3010246	root=compilation	route=accepted	route_us=12112	discharge_us=11255	reverify_us=8697	reverified=13	obligations=16	judged=13	core_refused=0	unproved=0	refuse
```

### `drop_requires` — `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_target_body_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: `current`
- Validity (read mutant): ensures remains after dropping requires `x == 10` (modular entry; callers cannot restate)
- Route hint (not validity): current=accepted current_us=3273306 root=compilation route=outside Unsupported: machine service reach
- Obligation: `ensures result == 12`
- Spec (wiki/spec/language/machines.md#nested-machines-and-identity): > A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/live_across_two_calls_preservation_bound_through_5878aa1fe2da__drop_requires/main.omg	current=accepted	current_us=3093893	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_authored_numeric_names_exit_01523b5da6ce__drop_requires/main.omg	current=accepted	current_us=2952567	root=compilation	route=accepted	route_us=1206	discharge_us=863	reverify_us=234	reverified=4	obligations=4	judged=4	core_refused=0	unproved=0	refused_other=0	needed_rel
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_projected_nominal_core_exit_d680f7a8ee20__drop_requires/main.omg	current=accepted	current_us=3046489	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/checked_predicate_disjoint_loans_through_other_s_a97c9dfe2420__drop_requires/main.omg	current=accepted	current_us=2969699	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/descriptor_range_exclusive_loan_through_paramete_d0e88e63a04d__drop_requires/main.omg	current=accepted	current_us=2962446	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/double_indexed_rmw_through_other_state_exit_8be82599b175__drop_requires/main.omg	current=accepted	current_us=3047622	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_authored_numeric_names_exit_01523b5da6ce__drop_requires/main.omg	current=accepted	current_us=3004256	root=compilation	route=accepted	route_us=1297	discharge_us=678	reverify_us=230	reverified=4	obligations=4	judged=4	core_refused=0	unproved=0	refused_other=0	needed_rel
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_projected_nominal_core_exit_d680f7a8ee20__drop_requires/main.omg	current=accepted	current_us=3072714	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/checked_predicate_disjoint_loans_through_other_s_a97c9dfe2420__drop_requires/main.omg	current=accepted	current_us=3150267	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/descriptor_range_exclusive_loan_through_paramete_d0e88e63a04d__drop_requires/main.omg	current=accepted	current_us=3377285	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/double_indexed_rmw_through_other_state_exit_8be82599b175__drop_requires/main.omg	current=accepted	current_us=3256751	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

## Hand verification — known-gap-q7-mut (5)

_None._

## Confirmed-hole entries

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


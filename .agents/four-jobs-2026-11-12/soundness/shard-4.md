# Soundness mutations shard 4

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (measured `fa37a2b617dd16bfed921dbdefb0b11e8951c9c5`). Host: linux cloud agent. Generated 2026-11-12 16:06 UTC.

## Bucket table (mutants only)

Total mutants run: **3650** (must equal sum of buckets).

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 80 |
| `known-gap-q7-mut` | 160 |
| `disagreement-valid` | 32 |
| `valid-route-outside` | 1536 |
| `discarded-valid` | 1324 |
| `rejected-both` | 488 |
| `no-mutation` | 30 |
| **Sum** | **3650** |

- Runner stats: generated=?, run=?, logged=?, current_rejected=?, not_applicable=?, failed_run=?. Delta between generated and logged = early rejects + unlogged kinds before drop_requires rule.


## Fixture / mutant reconcile (fix4)

- Fixtures in shard: **1130**
- Fixture-level `no-mutation` rows: **248**
- Fixtures with ≥1 generated mutant dir: **883**
- Mutant attempt rows (`stage=mutant`): **3650**
- Fixtures with >1 mutant row: **854**
- Reconcile: 1130 fixtures ≈ 248 no-output + 883 mutated

| Fixture | Mutant rows |
| --- | ---: |
| `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` | 7 |
| `tests/omega/pass/arithmetic/checked_flag_proof_saturating_cast_fits/main.omg` | 7 |
| `tests/omega/pass/arithmetic/fixed_array_live_length_compound_guard/main.omg` | 7 |
| `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` | 7 |
| `tests/omega/pass/arithmetic/qualified_cast_in_entry_body_exit/main.omg` | 7 |
| `tests/omega/pass/arithmetic/runtime_boolean_alias_shift_count_exit/main.omg` | 7 |
| `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` | 7 |
| `tests/omega/pass/arrays/qualified_array_literal_retains_element_policy_exit/main.omg` | 2 |
| `tests/omega/pass/asm/atomic_release_disposition_generic_exit/main.omg` | 3 |
| `tests/omega/pass/asm/checked_asm_compare_one_block_exit/main.omg` | 3 |
| `tests/omega/pass/asm/checked_fence_aarch64_in_x86_body/main.omg` | 3 |
| `tests/omega/pass/asm/entry_zero_branch_conditional_exit/main.omg` | 3 |
| `tests/omega/pass/asm/scratch_hardware_flag_mix/main.omg` | 2 |
| `tests/omega/pass/atomics/atomic_order_surface/main.omg` | 2 |
| `tests/omega/pass/atomics/runtime_authored_fence_name_exit/main.omg` | 3 |
| `tests/omega/pass/borrow/mutable_selector_unchanged_exit/main.omg` | 5 |
| `tests/omega/pass/borrow/runtime_signed_element_inclusive_exit/main.omg` | 7 |
| `tests/omega/pass/borrow/runtime_write_only_call_result_observation_exit/main.omg` | 5 |
| `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit/main.omg` | 5 |
| `tests/omega/pass/borrow/runtime_write_only_utf8_field_exit/main.omg` | 7 |
| … | (834 more) |

- Confirmed holes by mutation kind: delete_divisor_guard=12, drop_requires=22, ref_to_mut_param=2, remove_transition_guard=25, strip_and_guard=19
- Confirmed holes by accepted_by: both=4, current=76

## Commands

```bash
bash tools/agent_setup.sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null
export OMEGA_LIBRARY_ROOT=$PWD/source/library
python3 .agents/four-jobs-2026-11-12/soundness/shard4_runner.py
python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py
```

## Hand verification — confirmed holes (fix4)

### `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / delete_divisor_guard → confirmed-hole

Route reason: ``
Discard reason: ``
Obligation: `6: (7 / 2 * 2) as i32`

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_delete_d_5e5ba93519/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_delete_d_5e5ba93519/main.omg	current=accepted	current_us=1462120	root=compilation	route=no-entry
```

### `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / strip_and_guard → confirmed-hole

Route reason: ``
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_strip_an_81dc248c0c/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_strip_an_81dc248c0c/main.omg	current=accepted	current_us=1481333	root=compilation	route=no-entry
```

### `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / delete_divisor_guard → confirmed-hole

Route reason: `Unsupported: type constrained`
Discard reason: ``
Obligation: `6: let quotient: i64 in Wrapping = value / divisor;`

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_runtime_fixed_input_policy_reuse_exit_main_omg_delet_6a2f978618/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_runtime_fixed_input_policy_reuse_exit_main_omg_delet_6a2f978618/main.omg	current=accepted	current_us=3540304	root=compilation	route=outside	Unsupported: type constrained
```

## Hand verification — discarded drop_requires (sample)

### `tests/omega/pass/borrow/runtime_signed_element_inclusive_exit/main.omg` / drop_requires → discarded-valid

Route reason: ``
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -7,5 +7,4 @@
     }
     machine update(values: &mut [f64], amount: f64, index: i32 in Wrapping)
-    requires index >= 0 && index < values.len {
         values[index] = values[index] - amount;
         transition { _ -> scan(values, amount, index + 1) }
@@ -18,5 +17,4 @@
     transition context.offset >= 0 && last >= context.offset && last < 8 { true -> run(context, last) _ -> bad() }
     machine run(context: &mut Context, last: i32)
-    requires context.offset >= 0 && last >= context.offset && last < 8 {
         let view: &mut [f64] = &mut context.residuals[context.offset..=last];
         shifted(view, 0.5);
```

Current checker exit=1:

```
cannot compile admitted project: checked compilation failed for package `window_inclusive` with 1 diagnostic(s)
  error: /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_signed_element_inclusive_exit_main_omg_drop_requ_f75bbe1213/main.omg:9:9: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `boundary`, `requires`, `ensures`, `->`, `{`, found identifier `values`

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_signed_element_inclusive_exit_main_omg_drop_requ_f75bbe1213/main.omg	current=stopped	current_us=654298	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_signed_element_inclusive_exit_main_omg_drop_requ_f75bbe1213/main.omg:9:9: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `bound
```

### `tests/omega/pass/borrow/runtime_write_only_utf8_field_exit/main.omg` / drop_requires → discarded-valid

Route reason: ``
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -65,8 +65,6 @@
 
 machine Buffer8::copy(storage: &mut [u8; 8], bytes: &[u8], start: u64)
-requires start <= 8, bytes.len <= 8 - start {
     transition { _ -> copying(storage, bytes, start, 0) }
     machine copying(storage: &mut [u8; 8], bytes: &[u8], start: u64, index: u64)
-    requires start <= 8, index <= 8 {
         transition index < bytes.len && start + index < 8 {
             true -> writing(storage, bytes, start, index)
@@ -75,5 +73,4 @@
     }
     machine writing(storage: &mut [u8; 8], bytes: &[u8], start: u64, index: u64)
-    requires start <= 8, index < 8, index < bytes.len, start + index < 8 {
         storage[start + index] = bytes[index];
         transition { _ -> copying(storage, bytes, start, index + 1) }
@@ -87,5 +84,4 @@
     }
     machine joining(left: &[u8], right: &[u8]) -> Buffer8
-    requires left.len <= 8, right.len <= 8 - left.len {
     let mut result: Buffer8;
     Buffer8::copy(&mut result.storage, left, 0);
@@ -98,5 +94,4 @@
     }
     machine checked(result: Buffer8, total: u64) -> Buffer8
-    requires total <= 8 {
         let mut value: Buffer8 = result;
         value.used = total;
```

Current checker exit=1:

```
cannot compile admitted project: checked compilation failed for package `runtime_write_only_utf8_field_exit` with 1 diagnostic(s)
  error: /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_utf8_field_exit_main_omg_drop_require_04d094f62e/main.omg:67:5: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `boundary`, `requires`, `ensures`, `->`, `{`, found keyword `transition`

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_utf8_field_exit_main_omg_drop_require_04d094f62e/main.omg	current=stopped	current_us=702443	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_utf8_field_exit_main_omg_drop_require_04d094f62e/main.omg:67:5: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `boun
```

### `tests/omega/pass/borrows/fixed_array_reference_results_exit/main.omg` / drop_requires → discarded-valid

Route reason: ``
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -3,14 +3,9 @@
 data Main { weights: [[u64 in Wrapping; 13]; 33]; other: [[u64 in Wrapping; 13]; 33]; }
 machine select_row(matrix: &mut [[u64 in Wrapping; 13]; 33], depth: u64 in Wrapping) -> &mut [u64 in Wrapping; 13]
-requires depth < 33 { &mut matrix[depth] }
 machine forward<T>(value: &mut T) -> &mut T { value }
 machine attenuate(matrix: &mut [[u64 in Wrapping; 13]; 33], depth: u64 in Wrapping) -> &[u64 in Wrapping; 13]
-requires depth < 33 { &matrix[depth] }
 machine forward_row(matrix: &mut [[u64 in Wrapping; 13]; 33], depth: u64 in Wrapping) -> &mut [u64 in Wrapping; 13]
-requires depth < 33 { forward(select_row(matrix, depth)) }
 machine transition_row(matrix: &mut [[u64 in Wrapping; 13]; 33], depth: u64 in Wrapping) -> &mut [u64 in Wrapping; 13]
-requires depth < 33 { transition { _ -> select_row(matrix, depth) } }
 machine shared_row(matrix: &[[u64 in Wrapping; 13]; 33], depth: u64 in Wrapping) -> &[u64 in Wrapping; 13]
-requires depth < 33 { &matrix[depth] }
 machine read_row(row: &[u64 in Wrapping; 13]) -> u64 in Wrapping { row[1] }
 machine pair(left: &mut [u64 in Wrapping; 13], right: &mut [u64 in Wrapping; 13]) {
@@ -18,5 +13,4 @@
 }
 machine accumulate(row: &mut [u64 in Wrapping; 13], bin: u64 in Wrapping, weight: u64 in Wrapping)
-requires bin < 13 {
     row[bin] = row[bin] + weight;
 }
@@ -30,5 +24,4 @@
     }
     machine add(main: &mut Main, depth: u64 in Wrapping, richness: u64 in Wrapping)
-    requires depth < 33 && richness < 13 {
         let row: &mut [u64 in Wrapping; 13] = forward_row(&mut main.weights, depth);
         accumulate(row, richness, 4);
```

Current checker exit=1:

```
cannot compile admitted project: checked compilation failed for package `fixed_array_reference_results` with 1 diagnostic(s)
  error: /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_fixed_array_reference_results_exit_main_omg_drop_requir_b9d2669b98/main.omg:5:1: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `boundary`, `requires`, `ensures`, `->`, `{`, found keyword `machine`

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_fixed_array_reference_results_exit_main_omg_drop_requir_b9d2669b98/main.omg	current=stopped	current_us=660923	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_fixed_array_reference_results_exit_main_omg_drop_requir_b9d2669b98/main.omg:5:1: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `bound
```

### `tests/omega/pass/borrows/runtime_writeonly_checked_static_carrier_exit/main.omg` / drop_requires → discarded-valid

Route reason: ``
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 machine write_slot<'storage>(value: Slot<'storage>) { value.target.value = 61; }
 machine callback<'storage, machine Write>(value: Slot<'storage>)
-requires machine Write(value: Slot<'storage>);
 { Write(value); }
 machine prepare<'storage>(cell: &'storage write Cell) {
```

Current checker exit=1:

```
cannot compile admitted project: checked compilation failed for package `runtime_writeonly_checked_static_carrier_exit` with 1 diagnostic(s)
  error: /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_runtime_writeonly_checked_static_carrier_exit_main_omg__4504c900fc/main.omg:6:1: machine parameter `Write` requires an authored declaration-site contract: write `requires machine Write(...) -> Result;` or `requires machine Write satisfies Trait::requirement;`

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_runtime_writeonly_checked_static_carrier_exit_main_omg__4504c900fc/main.omg	current=stopped	current_us=695314	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_runtime_writeonly_checked_static_carrier_exit_main_omg__4504c900fc/main.omg:6:1: machine parameter `Write` requires an authored declaration-site contract: write `requires machine Write(...) -> Result;` o
```

### `tests/omega/pass/calls/const_boundary_uses_exact_bound/main.omg` / drop_requires → discarded-valid

Route reason: ``
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -1,13 +1,10 @@
 boundary trait BoundedValue {
     machine bound<const Bound: u64>(value: u64) -> u64 reaches
-        requires value <= Bound ensures result <= Bound;
 }
 PortableBoundedValue satisfies BoundedValue {
     machine bound<const Cap: u64>(value: u64) -> u64
-        requires value <= Cap ensures result <= Cap { value }
 }
 machine forward<const Available: u64, const Other: u64>(value: u64) -> u64
     reaches BoundedValue invokes BoundedValue
-    requires value <= Available ensures result <= Available
 { BoundedValue::bound<Available>(value) }
 machine main() {}
```

Current checker exit=1:

```
cannot compile admitted project: checked compilation failed for package `const_boundary_actual_identity` with 1 diagnostic(s)
  error: /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_const_boundary_uses_exact_bound_main_omg_drop_requires_53f5f292ee/main.omg:3:1: expected identifier, found punctuation `}`

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_const_boundary_uses_exact_bound_main_omg_drop_requires_53f5f292ee/main.omg	current=stopped	current_us=717909	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_const_boundary_uses_exact_bound_main_omg_drop_requires_53f5f292ee/main.omg:3:1: expected identifier, found punctuation `}`")
```

## Hand verification — known-gap-q7-mut (sample)

### `tests/omega/pass/asm/atomic_release_disposition_generic_exit/main.omg` / drop_mut_let → known-gap-q7-mut

Route reason: `Unsupported: machine service reach`
Discard reason: ``
Obligation: `8: proposition acknowledged<Value>(ack: Ack<Value>, word: &Cell) = atomic_release_written<AtomicU32, Value>(&word.value, ack.nonce);`

```diff
--- original
+++ mutant
@@ -14,5 +14,5 @@
 CompletionProvider satisfies Completion { machine complete(ack: Ack<u32>, word: &Cell) { finish(ack, word) } }
 machine main() reaches Completion crashes Trap {
- let mut word: Cell;
+ let word: Cell;
  let ack: Ack<u32> = Ack { nonce: 37 };
  Completion::complete(ack, &word);
```

Current checker exit=0:

```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_atomic_release_disposition_generic_exit_main_omg_drop_mut_l_1f9aba8004/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_atomic_release_disposition_generic_exit_main_omg_drop_mut_l_1f9aba8004/main.omg	current=accepted	current_us=3092439	root=compilation	route=outside	Unsupported: machine service reach
```

### `tests/omega/pass/asm/entry_zero_branch_conditional_exit/main.omg` / drop_mut_let → known-gap-q7-mut

Route reason: `Unsupported: machine assembly target`
Discard reason: ``
Obligation: `12: done requires eax == 0 ensures output == 0:`

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // Expected exit: 0.
 machine countdown() -> u32 ensures result == 0 {
-    let mut output: u32 = 99;
+    let output: u32 = 99;
     asm {
     entry:
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_entry_zero_branch_conditional_exit_main_omg_drop_mut_let_c37adf1f5d/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_entry_zero_branch_conditional_exit_main_omg_drop_mut_let_c37adf1f5d/main.omg	current=accepted	current_us=3187780	root=compilation	route=outside	Unsupported: machine assembly target
```

### `tests/omega/pass/borrow/runtime_write_only_call_result_observation_exit/main.omg` / drop_mut_let → known-gap-q7-mut

Route reason: `Unsupported: parameter mutable parameter`
Discard reason: ``
Obligation: `26: let observation: i32 = replace_and_observe(&write backing);; 30: transition backing == 42 && observation == 19`

```diff
--- original
+++ mutant
@@ -23,5 +23,5 @@
 
 machine Main::main(main: &mut Main) crashes Trap {
-    let mut backing: i32 = 7;
+    let backing: i32 = 7;
     let observation: i32 = replace_and_observe(&write backing);
     let mut other_backing: i32 = 11;
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_call_result_observation_exit_main_omg_7b67fba4b0/main.omg; wrote_output=false
warning: unreachable branch: the `true` arm of a transition in machine `Main::main` state `main` can never run; its guard contradicts the facts in force here: 7 >= backing

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_call_result_observation_exit_main_omg_7b67fba4b0/main.omg	current=accepted	current_us=3551469	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit/main.omg` / drop_mut_let → known-gap-q7-mut

Route reason: `Unsupported: parameter mutable parameter`
Discard reason: ``
Obligation: `15: wrapped(Box { p: &mut held }, identity(identity(incoming)));; 16: held.target.value = 89;; 17: let current: &mut Cell = observe(&mut held.target);`

```diff
--- original
+++ mutant
@@ -11,5 +11,5 @@
     main.left.value = 11;
     main.right.value = 85;
-    let mut held: Held = Held { target: &mut main.left, stamp: 17 };
+    let held: Held = Held { target: &mut main.left, stamp: 17 };
     let incoming: Held = Held { target: &mut main.right, stamp: 23 };
     wrapped(Box { p: &mut held }, identity(identity(incoming)));
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_exclusive_reference_owned_wrapper_exi_010ef1bcbf/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_exclusive_reference_owned_wrapper_exi_010ef1bcbf/main.omg	current=accepted	current_us=3557080	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `tests/omega/pass/borrow/write_only_projected_state_receiver_exit/main.omg` / drop_mut_let → known-gap-q7-mut

Route reason: `Unsupported: machine attached data`
Discard reason: ``
Obligation: `35: Main::main(&mut entry_storage);`

```diff
--- original
+++ mutant
@@ -32,5 +32,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() crashes Trap {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_write_only_projected_state_receiver_exit_main_omg_drop_m_b3c3ab4d0d/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_write_only_projected_state_receiver_exit_main_omg_drop_m_b3c3ab4d0d/main.omg	current=accepted	current_us=3446780	root=compilation	route=outside	Unsupported: machine attached data
```


## Confirmed holes and disagreements

### 1. `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_delete_d_5e5ba93519`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `6: (7 / 2 * 2) as i32`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_delete_d_5e5ba93519; wrote_output=false

```

### 2. `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_strip_an_81dc248c0c`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_anonymous_rational_integer_landing_main_omg_strip_an_81dc248c0c; wrote_output=false

```

### 3. `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_runtime_fixed_input_policy_reuse_exit_main_omg_delet_6a2f978618`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type constrained`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `6: let quotient: i64 in Wrapping = value / divisor;`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_arithmetic_runtime_fixed_input_policy_reuse_exit_main_omg_delet_6a2f978618; wrote_output=false

```

### 4. `tests/omega/pass/build/default_component_abi_extents/producer/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_build_default_component_abi_extents_producer_main_omg_remove_tr_8c4e2695fb`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: declaration Build selection that is not a machine or a data type`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `11: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -8,7 +8,7 @@
 }
 public Inspector satisfies Inspect<u8> {
     machine inspect(value: Packet) -> u64 {
-        transition value.bytes.len > 7u64 {
+        transition {
             true -> inspect_fields(value)
             _ -> wrong()
         }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_build_default_component_abi_extents_producer_main_omg_remove_tr_8c4e2695fb; wrote_output=false

```

### 5. `tests/omega/pass/calls/call_local_reassign/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_call_local_reassign_main_omg_remove_transition_guard_f28565bb1c`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `16: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -13,7 +13,7 @@
     let first: i32 = total;
     total = 71;
     total = echo(first);
-    transition first == 70 && total == 70 {
+    transition {
         true -> good(main, total)
         false -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_call_local_reassign_main_omg_remove_transition_guard_f28565bb1c; wrote_output=false

```

### 6. `tests/omega/pass/calls/call_local_reassign/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_call_local_reassign_main_omg_strip_and_guard_8f27ca0d09`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `16: transition first == 70 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -13,7 +13,7 @@
     let first: i32 = total;
     total = 71;
     total = echo(first);
-    transition first == 70 && total == 70 {
+    transition first == 70 {
         true -> good(main, total)
         false -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_call_local_reassign_main_omg_strip_and_guard_8f27ca0d09; wrote_output=false

```

### 7. `tests/omega/pass/calls/runtime_constructor_child_equal_captures_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_runtime_constructor_child_equal_captures_exit_main_omg_dr_bb68ee90ae`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine attached data`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires value.first == value.second
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: data Pair [copy] { first: i32; replaced: bool; second: i32; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -10,7 +10,6 @@
 }
 
 machine observe(value: Pair, expected: i32)
-requires value.first == value.second
 crashes Trap {
     transition value.first == expected && value.second == expected && value.replaced {
         true -> done()
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_runtime_constructor_child_equal_captures_exit_main_omg_dr_bb68ee90ae; wrote_output=false

```

### 8. `tests/omega/pass/cases/case_requires_payload/main.omg` (ref_to_mut_param)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_case_requires_payload_main_omg_ref_to_mut_param_554a67cc79`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `13: Mode::On {} if choice.counter == 3 -> hit()`
- Spec file/heading/quote (fix4a): `wiki/spec/language/ownership.md` — (Owned local mutability) modifying an established ordinary local's owned storage requires a `mut` binding.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -18,7 +18,7 @@
 }
 
 data Main {}
-machine Main::main(main: &mut Main) crashes Trap {
+machine Main::main(main: &Main) crashes Trap {
     let choice: Choice = make();
     let resident: u32 = observe(&choice);
     let routed: u32 = pick(&choice, Mode::On);
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_case_requires_payload_main_omg_ref_to_mut_param_554a67cc79; wrote_output=false

```

### 9. `tests/omega/pass/cases/match_guard_slice_index/main.omg` (ref_to_mut_param)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_match_guard_slice_index_main_omg_ref_to_mut_param_e63dcef219`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: // true arm; `values.len == 0` gates the complement arm of`
- Spec file/heading/quote (fix4a): `wiki/spec/language/ownership.md` — (Owned local mutability) modifying an established ordinary local's owned storage requires a `mut` binding.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -18,7 +18,7 @@
 
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let values: [i32; 2] = [31, 39];
     let empty: [i32; 0] = [];
     let nonempty: i32 = first(&values[..], 2);
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_match_guard_slice_index_main_omg_ref_to_mut_param_e63dcef219; wrote_output=false

```

### 10. `tests/omega/pass/cases/match_guard_slice_index/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_match_guard_slice_index_main_omg_strip_and_guard_4977417c5d`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_match_guard_slice_index_main_omg_strip_and_guard_4977417c5d; wrote_output=false

```

### 11. `tests/omega/pass/cases/self_payload_return/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_self_payload_return_main_omg_strip_and_guard_0c4787ffdd`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `10: transition sum {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -18,7 +18,7 @@
 machine Main::main(main: &mut Main) {
     let left: Sum = Sum::Left { v: 41 };
     let right: Sum = Sum::Right { w: 29 };
-    transition Sum::pick(&left) == 41 && Sum::pick(&right) == 29 {
+    transition Sum::pick(&left) == 41 {
         true -> done()
         _ -> bad()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_cases_self_payload_return_main_omg_strip_and_guard_0c4787ffdd; wrote_output=false

```

### 12. `tests/omega/pass/contracts/carry_contract_decl/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_contracts_carry_contract_decl_main_omg_remove_transition_guard_faf4e4eccc`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `17: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -14,7 +14,7 @@
     let portable: Portable<Empty> = Portable {
         payload: Empty { elements: [], marker: 42 },
     };
-    transition portable.payload.marker == 42 {
+    transition {
         true -> done(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_contracts_carry_contract_decl_main_omg_remove_transition_guard_faf4e4eccc; wrote_output=false

```

### 13. `tests/omega/pass/contracts/shared_field_current_arithmetic/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_contracts_shared_field_current_arithmetic_main_omg_strip_and_gu_3b7d85ae50`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_contracts_shared_field_current_arithmetic_main_omg_strip_and_gu_3b7d85ae50; wrote_output=false

```

### 14. `tests/omega/pass/control_flow/bool_disequality_subject_dispatch_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_bool_disequality_subject_dispatch_compile_main_omg_886b6bfb9d`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `10: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,7 +7,7 @@
 
 machine Main::main(main: &mut Main) {
     main.flag = false;
-    transition main.flag != true {
+    transition {
         true -> clear(main)
         _ -> set(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_bool_disequality_subject_dispatch_compile_main_omg_886b6bfb9d; wrote_output=false

```

### 15. `tests/omega/pass/control_flow/element_vs_element_subject_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_element_vs_element_subject_compile_main_omg_remove_20b2349937`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `11: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -8,7 +8,7 @@
 
 machine Main::main(main: &mut Main) {
     main.cells = [8, 3];
-    transition main.cells[0] > main.cells[1] {
+    transition {
         true -> leads(main)
         _ -> trails(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_element_vs_element_subject_compile_main_omg_remove_20b2349937; wrote_output=false

```

### 16. `tests/omega/pass/control_flow/helper_machine_state_dispatch_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_helper_machine_state_dispatch_compile_main_omg_rem_fe40a25f4c`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `8: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -5,7 +5,7 @@
 data Main { answer: u64; }
 
 machine Main::classify(input: u64) -> u64 {
-    transition input {
+    transition {
         0 -> none()
         _ -> some(input)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_helper_machine_state_dispatch_compile_main_omg_rem_fe40a25f4c; wrote_output=false

```

### 17. `tests/omega/pass/control_flow/member_member_relational_subject_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_member_member_relational_subject_compile_main_omg__cf6cf36b4f`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `13: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -10,7 +10,7 @@
 machine Main::main(main: &mut Main) {
     main.left = 9;
     main.right = 4;
-    transition main.left >= main.right {
+    transition {
         true -> leads(main)
         _ -> trails(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_member_member_relational_subject_compile_main_omg__cf6cf36b4f; wrote_output=false

```

### 18. `tests/omega/pass/control_flow/sequential_transitions_same_state_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_sequential_transitions_same_state_compile_main_omg_9ad55eb170`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `12: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 
 machine Main::main(main: &mut Main) {
     main.flag = 1;
-    transition main.flag {
+    transition {
         0 -> zero(main)
         _ -> further(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_control_flow_sequential_transitions_same_state_compile_main_omg_9ad55eb170; wrote_output=false

```

### 19. `tests/omega/pass/dependent/case_where_match_contributes/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_dependent_case_where_match_contributes_main_omg_strip_and_guard_1e9457f1b5`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `17: transition main.interval {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_dependent_case_where_match_contributes_main_omg_strip_and_guard_1e9457f1b5; wrote_output=false

```

### 20. `tests/omega/pass/domains/predicate_domain_param/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_domains_predicate_domain_param_main_omg_strip_and_guard_85ff761678`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `9: transition { _ -> moved }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_domains_predicate_domain_param_main_omg_strip_and_guard_85ff761678; wrote_output=false

```

### 21. `tests/omega/pass/facts/affine_discard_root_cleanup_through_field_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_affine_discard_root_cleanup_through_field_exit_main_omg_d_0cd6a9543a`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: parameter mutable parameter`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires resource.valid == 1u64
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // Fact: an unconsumed nominal resource whose drop contract requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 50u64 so 50u64 - result succeeds.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -12,7 +12,6 @@
 }
 
 machine drop_resource(resource: &mut Resource)
-requires resource.valid == 1u64
 {
     resource.dropped.value = resource.valid;
     resource.valid = 0u64;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_affine_discard_root_cleanup_through_field_exit_main_omg_d_0cd6a9543a; wrote_output=false

```

### 22. `tests/omega/pass/facts/as_slice_views_through_other_state_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_as_slice_views_through_other_state_exit_main_omg_drop_req_2ef54112b0`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type reference`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires length == 4
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `7: machine extent(entries: &[Entry; 4]) -> u64 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -3,13 +3,11 @@
 use omega::language::core::slice;
 data Entry { value: i32; }
 machine require_four(length: u64) -> u64
-requires length == 4
 { length }
 machine extent(entries: &[Entry; 4]) -> u64 {
     let view: &[Entry] = &entries[..];
     transition { _ -> measure(view) }
     machine measure(view: &[Entry]) -> u64
-    requires view.len == 4
     { require_four(view.len) }
 }
 machine main() crashes Trap {
```

```
compiled 30 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_as_slice_views_through_other_state_exit_main_omg_drop_req_2ef54112b0; wrote_output=false

```

### 23. `tests/omega/pass/facts/body_bearing_comparison_through_const_name_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_body_bearing_comparison_through_const_name_exit_main_omg__d6acf885ce`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine operator spelling`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires direct == false
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // Equal f64-in-Marked carriers. The selected body returns false, the direct comparison is false, and the match arm misses with result 11.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -16,7 +16,6 @@
 }
 
 machine hold(direct: bool) -> bool
-requires direct == false
 {
     true
 }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_body_bearing_comparison_through_const_name_exit_main_omg__d6acf885ce; wrote_output=false

```

### 24. `tests/omega/pass/facts/call_result_bound_discharge_through_const_name_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_call_result_bound_discharge_through_const_name_exit_main__0912062b21`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine attached data`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires scale <= 5
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: // proves `scale <= 5`.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,7 +7,6 @@
 const SCALE: u64 = opaque();
 
 machine echo(scale: u64) -> u64
-requires scale <= 5
 {
     scale
 }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_call_result_bound_discharge_through_const_name_exit_main__0912062b21; wrote_output=false

```

### 25. `tests/omega/pass/facts/captured_storage_bounds_through_parameter_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_captured_storage_bounds_through_parameter_exit_main_omg_d_88722ea3d5`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: parameter mutable parameter`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires self.pumped >= 0 && self.pumped <= 16,
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: public data Counter [copy]`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -1,7 +1,6 @@
 // Counter's declared 0..16 bounds survive a disjoint field write and captured scalar read, so an incoming pumped < 16 guard admits pumped + 1.
 // Expected exit: 0
 public data Counter [copy]
-requires self.pumped >= 0 && self.pumped <= 16,
 {
     before: u32;
     pumped: i32;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_captured_storage_bounds_through_parameter_exit_main_omg_d_88722ea3d5; wrote_output=false

```

### 26. `tests/omega/pass/facts/crash_entry_nan_equality_through_match_join_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_crash_entry_nan_equality_through_match_join_exit_main_omg_1ca6d15bb4`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `12: let nan: f32 = 0.0f32 / 0.0f32;`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_crash_entry_nan_equality_through_match_join_exit_main_omg_1ca6d15bb4; wrote_output=false

```

### 27. `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_call_result_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_dyn_dispatch_selector_payload_bound_through_call_result_e_8bacee39fc`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type dynamic trait`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `1: // Dispatching a trait method through a dynamic trait carrier (dyn / trait conformance) whose contract establishes result <= 88u64 preserves the return bound across the selector and data lanes, discharging exact subtraction 88u64 - result.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_dyn_dispatch_selector_payload_bound_through_call_result_e_8bacee39fc; wrote_output=false

```

### 28. `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_target_body_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_dyn_dispatch_selector_payload_bound_through_target_body_e_87fc227a90`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine invokes`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `1: // Dispatching a trait method through a dynamic trait carrier (dyn / trait conformance) whose contract establishes result <= 88u64 preserves the return bound across the selector and data lanes, discharging exact subtraction 88u64 - result.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_dyn_dispatch_selector_payload_bound_through_target_body_e_87fc227a90; wrote_output=false

```

### 29. `tests/omega/pass/facts/float_range_delivery_through_other_state_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_float_range_delivery_through_other_state_exit_main_omg_dr_45550f4e5f`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires value >= 0.0 && value <= 0.5
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `4: data Reading [copy] requires value >= 0.0 && value <= 0.25 { value: f64; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -3,12 +3,10 @@
 use omega::language::core::float_operations;
 data Reading [copy] requires value >= 0.0 && value <= 0.25 { value: f64; }
 machine admit(value: f64) -> f64
-requires value >= 0.0 && value <= 0.5
 { value }
 machine probe(reading: Reading) -> f64 {
     transition { _ -> deliver(reading.value) }
     machine deliver(value: f64) -> f64
-    requires value >= 0.0 && value <= 0.25
     { admit(value) }
 }
 machine main() crashes Trap {
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_float_range_delivery_through_other_state_exit_main_omg_dr_45550f4e5f; wrote_output=false

```

### 30. `tests/omega/pass/facts/generic_terminated_array_through_match_join_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_generic_terminated_array_through_match_join_exit_main_omg_8dc20c84ce`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine service reach`
- hole_subtype (fix5): **probe-not-built** — [Errno 2] No such file or directory: '/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/entry_probe/main.omg'
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `4: machine length<const Count: u64>(text: &[u8; Count]) -> u64`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -2,12 +2,9 @@
 // Expected exit: 0
 boundary trait CText {
     machine length<const Count: u64>(text: &[u8; Count]) -> u64
-    requires Count > 0
-    requires text[Count - 1] == 0
     reaches;
 }
 data Text [copy]
-requires self.bytes[3] == 0
 { prefix: u8; bytes: [u8; 4]; suffix: u8; }
 machine verify(value: &Text, expected: u64) reaches CText crashes Trap {
     let actual: u64 = CText::length<4>(&value.bytes);
```

```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_generic_terminated_array_through_match_join_exit_main_omg_8dc20c84ce; wrote_output=false

```

### 31. `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_call_result_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_32bit_ordered_bound_through_call_result_exit_m_0eb40a4dcf`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires reading >= 1.0f32 && reading <= 3.0f32
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `5: data Reading [copy] requires value >= 1.25f32 && value <= 2.75f32 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,7 +7,6 @@
 }
 
 machine verify_f32_window(reading: f32) -> i32
-requires reading >= 1.0f32 && reading <= 3.0f32
 {
     transition reading >= 1.25f32 && reading <= 2.75f32 {
         true -> ok()
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_32bit_ordered_bound_through_call_result_exit_m_0eb40a4dcf; wrote_output=false

```

### 32. `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_target_body_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_32bit_ordered_bound_through_target_body_exit_m_89466b32e9`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: declaration Build selection that is not a machine or a data type`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires reading >= 1.0f32 && reading <= 3.0f32
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `5: data Reading [copy] requires value >= 1.25f32 && value <= 2.75f32 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,7 +7,6 @@
 }
 
 machine verify_f32_window(reading: f32) -> i32
-requires reading >= 1.0f32 && reading <= 3.0f32
 {
     transition reading >= 1.25f32 && reading <= 2.75f32 {
         true -> ok()
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_32bit_ordered_bound_through_target_body_exit_m_89466b32e9; wrote_output=false

```

### 33. `tests/omega/pass/facts/ieee_float_ordered_comparison_through_parameter_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_ordered_comparison_through_parameter_exit_main_d19c6966b7`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires reading >= 1.0 && reading <= 3.0
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `5: data Reading [copy] requires value >= 1.5 && value <= 2.5 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,13 +7,11 @@
 }
 
 machine verify_window(reading: f64) -> i32
-requires reading >= 1.0 && reading <= 3.0
 {
     0i32
 }
 
 machine probe(value: f64) -> i32
-requires value >= 1.5 && value <= 2.5
 {
     verify_window(value)
 }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_ieee_float_ordered_comparison_through_parameter_exit_main_d19c6966b7; wrote_output=false

```

### 34. `tests/omega/pass/facts/match_primitive_local_origin_through_local_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_primitive_local_origin_through_local_exit_main_omg__8c2c68b0a3`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: expression match on a scalar subject`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires value == 17
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // A match selects a primitive local, the join records that local as the origin, and the consumer reads the recorded value.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,6 @@
 }
 
 machine read_origin(value: i64) -> i64
-requires value == 17
 {
     value
 }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_primitive_local_origin_through_local_exit_main_omg__8c2c68b0a3; wrote_output=false

```

### 35. `tests/omega/pass/facts/match_runtime_view_extent_through_target_body_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_runtime_view_extent_through_target_body_exit_main_o_9f6bbe5745`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: declaration Build selection that is not a machine or a data type`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires counter == 0
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `33: machine apply(chosen: &mut [i64; 4], other: &mut [i64; 6], visits: &mut u64, flag: bool)`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -4,7 +4,6 @@
 data Outcome [copy] { chosen_last: i64; other_last: i64; }
 
 machine choose(counter: &mut u64, flag: bool) -> bool
-requires counter == 0
 {
     counter = 1;
     flag
@@ -32,13 +31,11 @@
 
 boundary trait SelectedApply {
     machine apply(chosen: &mut [i64; 4], other: &mut [i64; 6], visits: &mut u64, flag: bool)
-        requires visits == 0
         reaches SelectedBodyCheck invokes SelectedBodyCheck crashes Trap;
 }
 
 X86Apply satisfies SelectedApply {
     machine apply(chosen: &mut [i64; 4], other: &mut [i64; 6], visits: &mut u64, flag: bool)
-        requires visits == 0
         reaches SelectedBodyCheck invokes SelectedBodyCheck crashes Trap
     {
         SelectedBodyCheck::require_tag(17u64);
@@ -53,7 +50,6 @@
 
 ArmApply satisfies SelectedApply {
     machine apply(chosen: &mut [i64; 4], other: &mut [i64; 6], visits: &mut u64, flag: bool)
-        requires visits == 0
         reaches SelectedBodyCheck invokes SelectedBodyCheck crashes Trap
     {
         SelectedBodyCheck::require_tag(29u64);
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_runtime_view_extent_through_target_body_exit_main_o_9f6bbe5745; wrote_output=false

```

### 36. `tests/omega/pass/facts/may_write_choice_through_other_state_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_may_write_choice_through_other_state_exit_main_omg_drop_r_bcbd5f79ae`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine generic parameters`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires index < 4
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `24: let items: [u8; 4] = [4, 7, 8, 9];`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -26,7 +26,6 @@
     let marker: u64 in Wrapping = add(overwrite(choose(&mut chosen, &mut other, true)), zero);
     transition { _ -> read(items, other, chosen, marker) }
     machine read(items: [u8; 4], index: u64, chosen: u64, marker: u64 in Wrapping)
-    requires index < 4
     {
         transition (marker as u64) == 1 && chosen == 9 && items[index] == 7 {
             true -> done()
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_may_write_choice_through_other_state_exit_main_omg_drop_r_bcbd5f79ae; wrote_output=false

```

### 37. `tests/omega/pass/facts/provider_contract_endpoint_guarded_crash_through_field_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_provider_contract_endpoint_guarded_crash_through_field_ex_6c8b05a0c8`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine invokes`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `16: false -> (numerator / denominator)`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_provider_contract_endpoint_guarded_crash_through_field_ex_6c8b05a0c8; wrote_output=false

```

### 38. `tests/omega/pass/facts/range_fact_delivery_through_other_machine_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_d_af04234eab`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=1918 discharge_us=1542 reverify_us=744 reverified=7 obligations=7 judged=7 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires index <= 4
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // For u64 index, index < 4 proves index <= 4 at count(index).`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -1,7 +1,6 @@
 // For u64 index, index < 4 proves index <= 4 at count(index).
 // Expected exit: 0.
 machine count(index: u64) -> u64
-requires index <= 4
 { index }
 machine forward(index: u64) -> u64 requires index < 4 { count(index) }
 machine probe(input: u64) -> u64 requires input < 4 { forward(input) }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_d_af04234eab; wrote_output=false

```

### 39. `tests/omega/pass/facts/recast_parent_restoration_through_field_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_recast_parent_restoration_through_field_exit_main_omg_dro_00941441d0`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine generic parameters`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires self.start >= 0 && self.start <= self.end,
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: public data Pair [copy]`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -1,7 +1,6 @@
 // A scalar store of zero through the u32 view restores Pair.start and its mixed parent predicate before returning the parent.
 // Expected exit: 0
 public data Pair [copy]
-requires self.start >= 0 && self.start <= self.end,
 {
     before: u32;
     start: i32;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_recast_parent_restoration_through_field_exit_main_omg_dro_00941441d0; wrote_output=false

```

### 40. `tests/omega/pass/facts/signed_division_quotient_through_match_join_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_division_quotient_through_match_join_exit_main_omg_a6546cacae`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: expression match on a scalar subject`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `9: true -> (-300i64) / (-3i64),`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_division_quotient_through_match_join_exit_main_omg_a6546cacae; wrote_output=false

```

### 41. `tests/omega/pass/facts/signed_exact_multiplication_bound_through_const_name_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_exact_multiplication_bound_through_const_name_exit_3c0ccdb417`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: declaration const`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires product >= 40 && product <= 90
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // Fact: multiplying two bounded signed i64 values (factor >= 10i64 && factor <= 15i64 and scale >= 4i64 && scale <= 6i64) establishes product >= 40i64 && product <= 90i64, discharging a downstream contract requiring product >= 40i64 && product <= 90i64 across all nine places.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -7,7 +7,6 @@
 const PRODUCT: i64 = FACTOR * SCALE;
 
 machine verify_product(product: i64) -> i32
-requires product >= 40 && product <= 90
 {
     0i32
 }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_exact_multiplication_bound_through_const_name_exit_3c0ccdb417; wrote_output=false

```

### 42. `tests/omega/pass/facts/typed_reflection_through_field_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_typed_reflection_through_field_exit_main_omg_drop_require_ea34090673`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine generic parameters`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires Kind == u8 || Kind == u16 || Kind == u32 || Kind == u64 || Kind == bool
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `47: machine observe<Kind [copy]>(context: &mut Totals, field: &reflect::FieldInfo, value: &Kind) crashes Trap`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -45,7 +45,6 @@
 }
 data Totals { calls: u64; sum: u64; order: u64; }
 machine observe<Kind [copy]>(context: &mut Totals, field: &reflect::FieldInfo, value: &Kind) crashes Trap
-requires Kind == u8 || Kind == u16 || Kind == u32 || Kind == u64 || Kind == bool
 {
     let number: u64 = scalar<Kind>(value);
     context.calls = ((context.calls as u64 in Wrapping) + (1u64 as u64 in Wrapping)) as u64;
@@ -58,14 +57,12 @@
     machine done() {}
     machine bad() { crash Trap; }
     machine scalar<Kind [copy]>(value: &Kind) -> u64
-    requires Kind == u8 || Kind == u16 || Kind == u32 || Kind == u64 || Kind == bool
     {
         transition {
             Kind == bool -> boolean(value)
             _ -> unsigned(value)
         }
         machine unsigned(value: &Kind) -> u64
-        requires Kind == u8 || Kind == u16 || Kind == u32 || Kind == u64
         { value as u64 }
         machine boolean(value: &bool) -> u64 { transition value { true -> 1u64 false -> 0u64 } }
     }
```

```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_typed_reflection_through_field_exit_main_omg_drop_require_ea34090673; wrote_output=false

```

### 43. `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_match_join_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_8bit_division_quotient_bound_through_match_join__d41996dda1`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: expression match on a scalar subject`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `10: true -> dividend / divisor,`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_8bit_division_quotient_bound_through_match_join__d41996dda1; wrote_output=false

```

### 44. `tests/omega/pass/facts/unsigned_division_quotient_through_match_join_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_division_quotient_through_match_join_exit_main_o_83ecee7da8`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: expression match on a scalar subject`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `9: true -> 400u64 / 4u64,`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_division_quotient_through_match_join_exit_main_o_83ecee7da8; wrote_output=false

```

### 45. `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__64ddccca7c`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=1474 discharge_us=1042 reverify_us=360 reverified=8 obligations=12 judged=10 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires val >= 400 && val <= 400 && shift >= 2 && shift <= 2
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // Logical right shift on a bounded u64 value by a bounded shift count establishes the exact post-shift interval, discharging exact subtraction shifted - 100u64.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,6 @@
     }
 
     machine compute(val: u64, shift: u64)
-    requires val >= 400 && val <= 400 && shift >= 2 && shift <= 2
     {
         let shifted: u64 = val >> shift;
         let res: u64 = shifted - 100;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__64ddccca7c; wrote_output=false

```

### 46. `tests/omega/pass/facts/write_only_ieee_store_readback_through_match_join_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_write_only_ieee_store_readback_through_match_join_exit_ma_e2cadf079c`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: parameter mutable parameter`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `5: machine store(r: &write f64) { r = 6.0 / 8.0; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_write_only_ieee_store_readback_through_match_join_exit_ma_e2cadf079c; wrote_output=false

```

### 47. `tests/omega/pass/float/float_match_effects/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_float_float_match_effects_main_omg_remove_transition_guard_598a4370df`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `25: transition { true -> 7 false -> 0 }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -22,7 +22,7 @@
         observe32(&mut calls, 3.0f32) -> divide(0),
         _ -> divide(0)
     };
-    transition result == 7 && calls == 3 { true -> 7 false -> 0 }
+    transition { true -> 7 false -> 0 }
 }
 
 data Main {}
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_float_float_match_effects_main_omg_remove_transition_guard_598a4370df; wrote_output=false

```

### 48. `tests/omega/pass/float/float_match_effects/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_float_float_match_effects_main_omg_strip_and_guard_cba28b80a9`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `25: transition result == 7 { true -> 7 false -> 0 }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -22,7 +22,7 @@
         observe32(&mut calls, 3.0f32) -> divide(0),
         _ -> divide(0)
     };
-    transition result == 7 && calls == 3 { true -> 7 false -> 0 }
+    transition result == 7 { true -> 7 false -> 0 }
 }
 
 data Main {}
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_float_float_match_effects_main_omg_strip_and_guard_cba28b80a9; wrote_output=false

```

### 49. `tests/omega/pass/generics/open_value_case_constructor_checked/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_generics_open_value_case_constructor_checked_main_omg_strip_and_f4e3c0d38d`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: machine generic parameters`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_generics_open_value_case_constructor_checked_main_omg_strip_and_f4e3c0d38d; wrote_output=false

```

### 50. `tests/omega/pass/layouts/local_access_declaration_resolves_ordinarily/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_layouts_local_access_declaration_resolves_ordinarily_main_omg_r_66486348d3`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `18: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -15,7 +15,7 @@
 
 machine main() {
     let local: Access = Access { level: 3 };
-    transition granted(&local) == 3 {
+    transition {
         true -> done()
         _ -> bad()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_layouts_local_access_declaration_resolves_ordinarily_main_omg_r_66486348d3; wrote_output=false

```

### 51. `tests/omega/pass/layouts/runtime_constant_layout_contract_exit/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_layouts_runtime_constant_layout_contract_exit_main_omg_strip_an_19e6287191`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: contract term other than a formal or literal`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `19: transition { _ -> later() }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_layouts_runtime_constant_layout_contract_exit_main_omg_strip_an_19e6287191; wrote_output=false

```

### 52. `tests/omega/pass/machines/nonreturning_unit/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_machines_nonreturning_unit_main_omg_remove_transition_guard_ef687cabc1`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `5: transition { _ -> waiting() }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 data Main { wait: bool; }
 
 machine Main::run(main: &mut Main) {
-    transition main.wait {
+    transition {
         true -> wait()
         _ -> finish()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_machines_nonreturning_unit_main_omg_remove_transition_guard_ef687cabc1; wrote_output=false

```

### 53. `tests/omega/pass/modules/boolean_logic_indices/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_modules_boolean_logic_indices_main_omg_strip_and_guard_c0add6faf5`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_modules_boolean_logic_indices_main_omg_strip_and_guard_c0add6faf5; wrote_output=false

```

### 54. `tests/omega/pass/objc/native_gui_loop/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_objc_native_gui_loop_main_omg_remove_transition_guard_aef3e9328b`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `183: transition { true -> frame_loop(main) _ -> bad(main) }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **1** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -180,7 +180,7 @@
     main.mode = ObjectiveC::send_string(main.tmp, main.sel, "kCFRunLoopDefaultMode");
     main.mask = 4294967295;
     main.frame = 0;
-    transition main.win != 0 { true -> frame_loop(main) _ -> bad(main) }
+    transition { true -> frame_loop(main) _ -> bad(main) }
     machine frame_loop(main: &mut Main) {
         transition main.frame < 3 { true -> render(main) _ -> teardown(main) }
     }
```

```
error: Terminal Build `build` failed: dependency requests require coordinator settlement before Product compilation

```

### 55. `tests/omega/pass/optimizer/rollback_control_flow_cleanup_control/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_control_flow_cleanup_control_main_omg_remove_3f2efb158d`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=510 discharge_us=169 reverify_us=0 reverified=0 obligations=0 judged=0 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `12: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 machine main() {
     let decided: bool = true;
 
-    transition decided {
+    transition {
         true -> taken()
         _ -> untaken()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_control_flow_cleanup_control_main_omg_remove_3f2efb158d; wrote_output=false

```

### 56. `tests/omega/pass/optimizer/rollback_proof_check_elision_control/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_proof_check_elision_control_main_omg_remove__6cc0016cdc`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=686 discharge_us=400 reverify_us=32 reverified=1 obligations=1 judged=1 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `17: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -14,7 +14,7 @@
     let produced: i32 = source();
     let folded: i32 = produced - produced;
 
-    transition folded == 0 {
+    transition {
         true -> done()
         _ -> done()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_proof_check_elision_control_main_omg_remove__6cc0016cdc; wrote_output=false

```

### 57. `tests/omega/pass/ownership/linear_branch_consume/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_linear_branch_consume_main_omg_remove_transition_guar_86bc81b850`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `14: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -11,7 +11,7 @@
 data Main { choose_first: bool; }
 machine Main::main(main: &mut Main) -> i32 {
     let receipt: Receipt = Receipt { code: 7 };
-    transition main.choose_first {
+    transition {
         true -> consume_first(receipt)
         _ -> consume_second(receipt)
     }
```

```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_linear_branch_consume_main_omg_remove_transition_guar_86bc81b850; wrote_output=false

```

### 58. `tests/omega/pass/ownership/local_shared_view_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_local_shared_view_compile_main_omg_remove_transition__ec8dcba477`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `12: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 machine Main::main(main: &mut Main) {
     let count: u64 = 6;
     let seen: u64 = Main::read(&count);
-    transition seen {
+    transition {
         6 -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_local_shared_view_compile_main_omg_remove_transition__ec8dcba477; wrote_output=false

```

### 59. `tests/omega/pass/ownership/two_member_loans_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_two_member_loans_compile_main_omg_remove_transition_g_2b096d1e69`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `19: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -16,7 +16,7 @@
     main.x = 0;
     main.y = 0;
     Main::weave(&mut main.x, &mut main.y);
-    transition main.x {
+    transition {
         7 -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_ownership_two_member_loans_compile_main_omg_remove_transition_g_2b096d1e69; wrote_output=false

```

### 60. `tests/omega/pass/packages/indexed_contract_field_review/vendor/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_packages_indexed_contract_field_review_vendor_main_omg_strip_an_a5f0fd918d`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_packages_indexed_contract_field_review_vendor_main_omg_strip_an_a5f0fd918d; wrote_output=false

```

### 61. `tests/omega/pass/proofs/called_conjunction_requires/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_called_conjunction_requires_main_omg_strip_and_guard_e3d8bdb96e`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_called_conjunction_requires_main_omg_strip_and_guard_e3d8bdb96e; wrote_output=false

```

### 62. `tests/omega/pass/proofs/core_identity_ieee_reflexivity_exit/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_core_identity_ieee_reflexivity_exit_main_omg_delete_divi_a180c40323`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `50: let nan: f64 = 0.0f64 / 0.0f64;`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_core_identity_ieee_reflexivity_exit_main_omg_delete_divi_a180c40323; wrote_output=false

```

### 63. `tests/omega/pass/proofs/core_identity_ieee_reflexivity_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_core_identity_ieee_reflexivity_exit_main_omg_drop_requir_442628112f`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: type float`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires both<f64>(left, right)
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `3: data FloatBox [copy] { value: f64; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -25,10 +25,8 @@
 let through_partial<A: core::Type<0>>(left: A, right: A): core::Type<0> = partial<A>(left, right);
 let both<A: core::Type<0>>(left: A, right: A): core::Strict<0> = core::Identity<A>(left, right) && core::Identity<A>(right, left);
 machine need_both(left: f64, right: f64)
-requires both<f64>(left, right)
 { }
 machine require_both(left: f64, right: f64)
-requires core::Identity<f64>(left, right)
 { need_both(left, right); }
 machine keep_constructed(value: IdentityBox<u8>) reaches terminates
 ensures same_box_outer<u8>(value, value)
@@ -44,7 +42,6 @@
 ensures core::Identity<FloatBox>(value, value)
 { }
 machine require_same(left: f64, right: f64)
-requires through_partial<f64>(left, right)
 { }
 machine main() crashes Trap {
     let nan: f64 = 0.0f64 / 0.0f64;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_core_identity_ieee_reflexivity_exit_main_omg_drop_requir_442628112f; wrote_output=false

```

### 64. `tests/omega/pass/proofs/embed_collection_length/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_embed_collection_length_main_omg_drop_requires_2535b51d2f`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires embed(count) == 3
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `4: data Bag { items: [i32; 3]; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -5,13 +5,11 @@
 data Reading { len: i32; }
 
 machine observe(count: u64) -> bool
-requires embed(count) == 3
 {
     transition { _ -> (true) }
 }
 
 machine span(items: &[i32; 3]) -> u32
-requires embed(items.len) == 3
 {
     let count: u64 = items.len;
     let accepted: bool = observe(count);
@@ -23,11 +21,9 @@
 }
 
 machine window(items: &[i32; 4])
-requires embed(items[0..2].len) == 2
 {}
 
 machine same_spelled_field(reading: &Reading)
-requires embed(reading.len) == embed(reading.len)
 {}
 
 machine main() crashes Trap {
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_embed_collection_length_main_omg_drop_requires_2535b51d2f; wrote_output=false

```

### 65. `tests/omega/pass/proofs/numeric_state_contract/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_numeric_state_contract_main_omg_strip_and_guard_329d3ddec6`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `34: transition { _ -> swapped(second, first) }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_numeric_state_contract_main_omg_strip_and_guard_329d3ddec6; wrote_output=false

```

### 66. `tests/omega/pass/proofs/selected_map_law_application_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_selected_map_law_application_exit_main_omg_drop_requires_eaa606fea6`
- Current: **accepted**; route: **outside**
- Route hint (not validity): `Unsupported: contract clause that is not a comparison or conjunction`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires core::Identity<Pair>(Selected::apply(value), value)
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: data Pair [copy] { first: u8; last: u8; }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -2,13 +2,11 @@
 trait Operation<T> { machine apply(value: T) -> T reaches terminates; }
 trait Law<Selected satisfies Operation<Pair>> {
     machine theorem(value: Pair) reaches terminates
-    requires core::Identity<Pair>(Selected::apply(value), value)
     ensures core::Identity<Pair>(Selected::apply(value), value);
 }
 Identity satisfies Operation<Pair> { machine apply(value: Pair) -> Pair reaches terminates { value } }
 Proof satisfies Law<Identity> {
  machine theorem(value: Pair) reaches terminates
- requires core::Identity<Pair>(Identity::apply(value), value)
  ensures core::Identity<Pair>(Identity::apply(value), value) { }
 }
 machine main() { }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_proofs_selected_map_law_application_exit_main_omg_drop_requires_eaa606fea6; wrote_output=false

```

### 67. `tests/omega/pass/recast/float_bits_view_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_float_bits_view_compile_main_omg_remove_transition_guard_29ff4f3445`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `12: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 
 machine Main::main(main: &mut Main) {
     main.rate = 1.5;
-    transition (&main.rate as &u64) == 4609434218613702656 {
+    transition {
         true -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_float_bits_view_compile_main_omg_remove_transition_guard_29ff4f3445; wrote_output=false

```

### 68. `tests/omega/pass/recast/int_float_bits_view_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_int_float_bits_view_compile_main_omg_remove_transition_g_8b3cb30de2`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `11: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -8,7 +8,7 @@
 
 machine Main::main(main: &mut Main) {
     main.bits = 4607182418800017408;
-    transition (&main.bits as &f64) == 1.0 {
+    transition {
         true -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_int_float_bits_view_compile_main_omg_remove_transition_g_8b3cb30de2; wrote_output=false

```

### 69. `tests/omega/pass/recast/mutable_float_bits_write_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_mutable_float_bits_write_compile_main_omg_remove_transit_aca17ed6a7`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `13: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -10,7 +10,7 @@
     main.rate = 1.5;
     let bits: &mut u64 = &mut main.rate as &mut u64;
     bits = 13830554455654793216;
-    transition main.rate == -1.5 {
+    transition {
         true -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_mutable_float_bits_write_compile_main_omg_remove_transit_aca17ed6a7; wrote_output=false

```

### 70. `tests/omega/pass/recast/mutable_u16_signed_write_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_mutable_u16_signed_write_compile_main_omg_remove_transit_8249fe4961`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `13: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -10,7 +10,7 @@
     main.half = 0;
     let view: &mut i16 = &mut main.half as &mut i16;
     view = 0 - 5;
-    transition main.half == 65531 {
+    transition {
         true -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_mutable_u16_signed_write_compile_main_omg_remove_transit_8249fe4961; wrote_output=false

```

### 71. `tests/omega/pass/recast/signed_source_bits_view_compile/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_signed_source_bits_view_compile_main_omg_remove_transiti_aa2ec0c7ec`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `11: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -8,7 +8,7 @@
 
 machine Main::main(main: &mut Main) {
     main.word = 0 - 1;
-    transition (&main.word as &u64) == 18446744073709551615 {
+    transition {
         true -> good(main)
         _ -> bad(main)
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_recast_signed_source_bits_view_compile_main_omg_remove_transiti_aa2ec0c7ec; wrote_output=false

```

### 72. `tests/omega/pass/slices/element_view_write/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_element_view_write_main_omg_remove_transition_guard_50bef0cfbc`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `16: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -13,7 +13,7 @@
     main.items = [1, 2, 3, 4];
     let view: &mut [i32] = &mut main.items[..];
     view[1] = 77;
-    transition main.items[1] == 77 {
+    transition {
         true -> done()
         _ -> bad()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_element_view_write_main_omg_remove_transition_guard_50bef0cfbc; wrote_output=false

```

### 73. `tests/omega/pass/slices/mut_slice_establish/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_mut_slice_establish_main_omg_remove_transition_guard_60a0e21385`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `20: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -17,7 +17,7 @@
 machine Main::main(main: &mut Main) {
     fill(&mut main.bag);
     sow(&mut main.bag);
-    transition main.bag.cells[2] == 7 && main.bag.cells[0] == 1 {
+    transition {
         true -> done()
         _ -> bad()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_mut_slice_establish_main_omg_remove_transition_guard_60a0e21385; wrote_output=false

```

### 74. `tests/omega/pass/slices/mut_slice_establish/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_mut_slice_establish_main_omg_strip_and_guard_e36e0597ce`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `20: transition main.bag.cells[2] == 7 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -17,7 +17,7 @@
 machine Main::main(main: &mut Main) {
     fill(&mut main.bag);
     sow(&mut main.bag);
-    transition main.bag.cells[2] == 7 && main.bag.cells[0] == 1 {
+    transition main.bag.cells[2] == 7 {
         true -> done()
         _ -> bad()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_slices_mut_slice_establish_main_omg_strip_and_guard_e36e0597ce; wrote_output=false

```

### 75. `tests/omega/pass/termination/measure_field_cycle/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_measure_field_cycle_main_omg_remove_transition_guar_3fcd7e8471`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `15: transition { _ -> Main::scan_a(main, Countdown { remaining: 2 }) }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -16,7 +16,7 @@
 }
 machine Main::scan_a(main: &mut Main, countdown: Countdown) -> u64
 terminates by countdown -> Countdown::Remaining in 0..=5 {
-    transition countdown.remaining > 0 {
+    transition {
         true -> Main::scan_b(main, countdown)
         false -> countdown.remaining
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_measure_field_cycle_main_omg_remove_transition_guar_3fcd7e8471; wrote_output=false

```

### 76. `tests/omega/pass/termination/measure_field_rank_step/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_measure_field_rank_step_main_omg_strip_and_guard_e303151fb7`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `8: transition countdown.remaining >= amount {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_measure_field_rank_step_main_omg_strip_and_guard_e303151fb7; wrote_output=false

```

### 77. `tests/omega/pass/termination/rank_range_call_declared_step_copy/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_rank_range_call_declared_step_copy_main_omg_strip_a_39cca6d617`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `10: transition { _ -> Main::count(main, 9, 1) }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_rank_range_call_declared_step_copy_main_omg_strip_a_39cca6d617; wrote_output=false

```

### 78. `tests/omega/pass/termination/remainder_actual_call_component/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_remainder_actual_call_component_main_omg_strip_and__5efdcc3ba5`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `8: transition remaining > 0 {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_remainder_actual_call_component_main_omg_strip_and__5efdcc3ba5; wrote_output=false

```

### 79. `tests/omega/pass/termination/stored_reference_endpoint_arrivals/main.omg` (strip_and_guard)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_stored_reference_endpoint_arrivals_main_omg_strip_a_476a936f78`
- Current: **accepted**; route: **no-entry**
- Route hint (not validity): ``
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `7: transition { _ -> iterate(indirect, remaining) }`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Right-hand-side mutations invalidate their own overlapping facts when the written place overlaps a fact's subject.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_termination_stored_reference_endpoint_arrivals_main_omg_strip_a_476a936f78; wrote_output=false

```

### 80. `tests/omega/run/facts/signed_division_quotient_truncation_not_floor_traps/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_run_facts_signed_division_quotient_truncation_not_floor_traps_main_o_c531e9d86c`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=34271 discharge_us=33963 reverify_us=17115 reverified=5 obligations=5 judged=5 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `2: // -7i64 / 2i64 is -3i64 (truncation), not -4i64 (floor). This program asserts`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_run_facts_signed_division_quotient_truncation_not_floor_traps_main_o_c531e9d86c; wrote_output=false

```


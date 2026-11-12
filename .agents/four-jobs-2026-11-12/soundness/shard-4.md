# Soundness mutations shard 4

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (measured `eaad9b07f9ce834c2d8619331565339c5a5dfcc6`). Host: linux cloud agent. Generated 2026-11-12 21:06 UTC.

## Bucket table (mutant attempts with disk dirs only)

Mutant rows in bucket table: **3620** (equals sum of buckets).
Not-applicable attempt rows (outside table): **149** (kinds with no mutant dir; former per-attempt `no-mutation`).

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 22 |
| `known-gap-q7-mut` | 162 |
| `disagreement-valid` | 59 |
| `valid-route-outside` | 2981 |
| `discarded-valid` | 100 |
| `rejected-both` | 31 |
| `rejected-current-no-route` | 0 |
| `rejected-current-route-outside` | 265 |
| `rejected-current-other` | 0 |
| `timeout` | 0 |
| `failed-run` | 0 |
| **Sum** | **3620** |

- `timeout` among mutants: **0**
- `failed-run` among mutants: **0**

## Fixture reconcile (exact)

**1130** fixtures = **18** no-mutation + **230** original-not-accepted + **882** mutated

`original-not-accepted` means the original program got a normal `current=rejected` verdict at the PIN (not harness failed-run). The runner does not mutate those fixtures.

- Fixture-level companion rows (`stage=no-mutation`): **248** (=230 original-not-accepted + 18 accepted-without-mutators)
- Mutant attempt rows with dirs: **3620**; not-applicable: **149**; attempt rows total: **3769**

## PIN baseline (timeout 300s, all originals)

Shard fixtures at PIN **`0835b3aaf803ed97f905be42d94727ff58459263`**. **230** originals stay `original-not-accepted` (real checker diagnostics at 300s, not harness timeout).

| Category | Count |
| --- | ---: |
| `accepted` | 900 |
| `rejected-with-diagnostic` | 230 |

### Examples: `accepted`

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg`: 
- `tests/omega/pass/arithmetic/checked_flag_proof_saturating_cast_fits/main.omg`: 
- `tests/omega/pass/arithmetic/fixed_array_live_length_compound_guard/main.omg`: 

### Examples: `rejected-with-diagnostic`

- `tests/omega/pass/arithmetic/const_fold_unsigned_landed_ops_exit/main.omg`: error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding;
- `tests/omega/pass/arithmetic/runtime_bounded_unsigned_sum_field_and_call_exit/main.omg`: error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding;
- `tests/omega/pass/arithmetic/runtime_domain_boundaries_exit/main.omg`: error: target root slot `linux_x86_64::ProgramEntry` takes boundary schema `LinuxX86_64Application` from a dependency's `targets/linux_x86_64/entry.omg`, which needs an accepted program-entry binding;

## `valid-route-outside` route verdict audit

Rows in bucket: **2981**. Missing/empty `route_verdict`: **0**.

| `route_verdict` | Count |
| --- | ---: |
| `no-entry` | 260 |
| `outside` | 2566 |
| `unreached` | 155 |

## Mutation kinds (canonical, generated attempts with dirs)

| Canonical kind | Count |
| --- | ---: |
| `delete_divisor_guard` | 825 |
| `drop_requires` | 433 |
| `mut_ref_instead_of_shared` | 0 |
| `remove_mut` | 347 |
| `remove_transition_guard` | 758 |
| `shared_instead_of_mut` | 380 |
| `strip_and_guard` | 577 |
| `widen_requires` | 300 |

- `drop_requires` site split (header vs body): body-or-data=72, machine-header=361
- **known-gap-q7-mut:** 162 (canonical kind `remove_mut` unless noted)
- Confirmed holes by kind: drop_requires=22
- Confirmed holes by accepted_by: both=1, current=21

## Bucket transition (c0a4894 → strict audit)

**rejected-both verification failures:** 0

| Old | New | Count | Rule |
| --- | --- | ---: | --- |
| `discarded-valid` | `valid-route-outside` | 1195 | Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). |
| `rejected-both` | `rejected-current-route-outside` | 265 |  |
| `rejected-both` | `valid-route-outside` | 197 |  |
| `confirmed-hole` | `valid-route-outside` | 53 | Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. |
| `discarded-valid` | `disagreement-valid` | 27 | Checker disagreement after strict validity reassessment. |
| `discarded-valid` | `rejected-both` | 5 | Superseded rename to rejected-current-no-route (same rule). |
| `confirmed-hole` | `discarded-valid` | 3 | Hole demoted: missing spec quote, obligation, or no-trap evidence. |
| `confirmed-hole` | `known-gap-q7-mut` | 2 | Ownership/Q7 mutability gap (remove_mut / shared_instead_of_mut) per lead notes. |

### Spot checks (5 per major flow)

### discarded-valid → valid-route-outside (1195 rows)

- `tests/omega/pass/arithmetic/fixed_array_live_length_compound_guard/main.omg` / `ref_to_mut_param`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=rejected, route=no-entry)
- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `ref_to_mut_param`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=rejected, route=no-entry)
- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `strip_and_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/qualified_cast_in_entry_body_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)

### discarded-valid → rejected-current-no-route (5 rows)

- `tests/omega/pass/domains/runtime_stored_call_inequality_addition_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/callee_saved_across_nested_calls_through_call_result_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_machine_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/signed_shift_left_bound_through_parameter_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/unsigned_division_remainder_through_local_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)

### confirmed-hole → valid-route-outside (53 rows)

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `strip_and_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/build/default_component_abi_extents/producer/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/calls/call_local_reassign/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)

## Kind totals (canonical)

## Kind × bucket cross-table

| Kind | `confirmed-hole` | `disagreement-valid` | `discarded-valid` | `known-gap-q7-mut` | `rejected-both` | `rejected-current-route-outside` | `valid-route-outside` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `delete_divisor_guard` | 0 | 15 | 35 | 0 | 0 | 0 | 775 |
| `drop_requires` | 22 | 0 | 1 | 0 | 31 | 214 | 165 |
| `remove_mut` | 0 | 0 | 0 | 160 | 0 | 15 | 172 |
| `remove_transition_guard` | 0 | 13 | 26 | 0 | 0 | 0 | 719 |
| `shared_instead_of_mut` | 0 | 0 | 0 | 2 | 0 | 36 | 342 |
| `strip_and_guard` | 0 | 16 | 21 | 0 | 0 | 0 | 540 |
| `widen_requires` | 0 | 15 | 17 | 0 | 0 | 0 | 268 |
| **Sum** | — | — | — | — | — | — | — |

Kind total check: **3620** (must equal **3620**).

## Fixtures with multiple mutant rows (sample)

| Fixture | Mutant rows |
| --- | ---: |
| `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` | 4 |
| `tests/omega/pass/arithmetic/fixed_array_live_length_compound_guard/main.omg` | 4 |
| `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` | 4 |
| `tests/omega/pass/arithmetic/runtime_boolean_alias_shift_count_exit/main.omg` | 2 |
| `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` | 4 |
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
| `tests/omega/pass/borrow/write_only_projected_state_receiver_exit/main.omg` | 4 |
| `tests/omega/pass/borrows/borrowed_window_parameter_provenance_exit/main.omg` | 5 |
| … | (832 more) |

## Commands

```bash
bash tools/agent_setup.sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null
export OMEGA_LIBRARY_ROOT=$PWD/source/library
python3 .agents/four-jobs-2026-11-12/soundness/shard4_runner.py
python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py
```

## Confirmed holes (full table)

| # | fixture | kind | accepted_by | hole_subtype | spec quote | obligation | no-trap / check-time | diff |
| ---: | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `tests/omega/pass/calls/runtime_constructor_child_equal_captures_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 3: data Pair [copy] { first: i32; replaced: bool; second: i32; } | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -10,7 +10,6 @@ } machine observe(value: Pair, expected: i32) -requires va… |
| 2 | `tests/omega/pass/facts/affine_discard_root_cleanup_through_field_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // Fact: an unconsumed nominal resource whose drop contract requires valid ==… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -12,7 +12,6 @@ } machine drop_resource(resource: &mut Resource) -requires… |
| 3 | `tests/omega/pass/facts/as_slice_views_through_other_state_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 7: machine extent(entries: &[Entry; 4]) -> u64 { | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -3,13 +3,11 @@ use omega::language::core::slice; data Entry { value: i32;… |
| 4 | `tests/omega/pass/facts/body_bearing_comparison_through_const_name_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // Equal f64-in-Marked carriers. The selected body returns false, the direct … | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -16,7 +16,6 @@ } machine hold(direct: bool) -> bool -requires direct == f… |
| 5 | `tests/omega/pass/facts/call_result_bound_discharge_through_const_name_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 3: // proves `scale <= 5`. | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -7,7 +7,6 @@ const SCALE: u64 = opaque(); machine echo(scale: u64) -> u64… |
| 6 | `tests/omega/pass/facts/captured_storage_bounds_through_parameter_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 3: public data Counter [copy] | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -1,7 +1,6 @@ // Counter's declared 0..16 bounds survive a disjoint field … |
| 7 | `tests/omega/pass/facts/float_range_delivery_through_other_state_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 4: data Reading [copy] requires value >= 0.0 && value <= 0.25 { value: f64; } | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -3,12 +3,10 @@ use omega::language::core::float_operations; data Reading … |
| 8 | `tests/omega/pass/facts/generic_terminated_array_through_match_join_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 4: machine length<const Count: u64>(text: &[u8; Count]) -> u64 | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -2,12 +2,9 @@ // Expected exit: 0 boundary trait CText { machine length<c… |
| 9 | `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_call_result_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 5: data Reading [copy] requires value >= 1.25f32 && value <= 2.75f32 { | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -7,7 +7,6 @@ } machine verify_f32_window(reading: f32) -> i32 -requires r… |
| 10 | `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_target_body_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 5: data Reading [copy] requires value >= 1.25f32 && value <= 2.75f32 { | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -7,7 +7,6 @@ } machine verify_f32_window(reading: f32) -> i32 -requires r… |
| 11 | `tests/omega/pass/facts/ieee_float_ordered_comparison_through_parameter_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 5: data Reading [copy] requires value >= 1.5 && value <= 2.5 { | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -7,13 +7,11 @@ } machine verify_window(reading: f64) -> i32 -requires rea… |
| 12 | `tests/omega/pass/facts/match_primitive_local_origin_through_local_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // A match selects a primitive local, the join records that local as the orig… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -9,7 +9,6 @@ } machine read_origin(value: i64) -> i64 -requires value == … |
| 13 | `tests/omega/pass/facts/match_runtime_view_extent_through_target_body_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 33: machine apply(chosen: &mut [i64; 4], other: &mut [i64; 6], visits: &mut u64,… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -4,7 +4,6 @@ data Outcome [copy] { chosen_last: i64; other_last: i64; } m… |
| 14 | `tests/omega/pass/facts/may_write_choice_through_other_state_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 24: let items: [u8; 4] = [4, 7, 8, 9]; | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -26,7 +26,6 @@ let marker: u64 in Wrapping = add(overwrite(choose(&mut ch… |
| 15 | `tests/omega/pass/facts/range_fact_delivery_through_other_machine_exit/main.omg` | `drop_requires` | both | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // For u64 index, index < 4 proves index <= 4 at count(index). | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -1,7 +1,6 @@ // For u64 index, index < 4 proves index <= 4 at count(index… |
| 16 | `tests/omega/pass/facts/recast_parent_restoration_through_field_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 3: public data Pair [copy] | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -1,7 +1,6 @@ // A scalar store of zero through the u32 view restores Pair… |
| 17 | `tests/omega/pass/facts/signed_exact_multiplication_bound_through_const_name_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // Fact: multiplying two bounded signed i64 values (factor >= 10i64 && factor… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -7,7 +7,6 @@ const PRODUCT: i64 = FACTOR * SCALE; machine verify_product(… |
| 18 | `tests/omega/pass/facts/typed_reflection_through_field_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 47: machine observe<Kind [copy]>(context: &mut Totals, field: &reflect::FieldInf… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -45,7 +45,6 @@ } data Totals { calls: u64; sum: u64; order: u64; } machin… |
| 19 | `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: // Logical right shift on a bounded u64 value by a bounded shift count establ… | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -9,7 +9,6 @@ } machine compute(val: u64, shift: u64) - requires val >= 40… |
| 20 | `tests/omega/pass/proofs/core_identity_ieee_reflexivity_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 3: data FloatBox [copy] { value: f64; } | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -25,10 +25,8 @@ let through_partial<A: core::Type<0>>(left: A, right: A):… |
| 21 | `tests/omega/pass/proofs/embed_collection_length/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 4: data Bag { items: [i32; 3]; } | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -5,13 +5,11 @@ data Reading { len: i32; } machine observe(count: u64) -> … |
| 22 | `tests/omega/pass/proofs/selected_map_law_application_exit/main.omg` | `drop_requires` | current | probe-not-built | (Nested machines and identity) A written entry contract remains complete even when all currently vis… | 1: data Pair [copy] { first: u8; last: u8; } | spec quote targets proof-time establishment (requires/ownership/contracts) | --- original +++ mutant @@ -2,13 +2,11 @@ trait Operation<T> { machine apply(value: T) -> T reaches … |

## Hand verification — confirmed holes (fix4)

### `tests/omega/pass/calls/runtime_constructor_child_equal_captures_exit/main.omg` / drop_requires → confirmed-hole

Route reason: `Unsupported: machine attached data`
Discard reason: ``
Obligation: `3: data Pair [copy] { first: i32; replaced: bool; second: i32; }`

```diff
--- original
+++ mutant
@@ -11,5 +11,4 @@
 
 machine observe(value: Pair, expected: i32)
-requires value.first == value.second
 crashes Trap {
     transition value.first == expected && value.second == expected && value.replaced {
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_runtime_constructor_child_equal_captures_exit_main_omg_dr_bb68ee90ae/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_runtime_constructor_child_equal_captures_exit_main_omg_dr_bb68ee90ae/main.omg	current=accepted	current_us=2984689	root=compilation	route=outside	Unsupported: machine attached data
```

### `tests/omega/pass/facts/affine_discard_root_cleanup_through_field_exit/main.omg` / drop_requires → confirmed-hole

Route reason: `Unsupported: parameter mutable parameter`
Discard reason: ``
Obligation: `1: // Fact: an unconsumed nominal resource whose drop contract requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 50u64 so 50u64 - result succeeds.`

```diff
--- original
+++ mutant
@@ -13,5 +13,4 @@
 
 machine drop_resource(resource: &mut Resource)
-requires resource.valid == 1u64
 {
     resource.dropped.value = resource.valid;
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_affine_discard_root_cleanup_through_field_exit_main_omg_d_0cd6a9543a/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_affine_discard_root_cleanup_through_field_exit_main_omg_d_0cd6a9543a/main.omg	current=accepted	current_us=3123053	root=compilation	route=outside	Unsupported: parameter mutable parameter
```

### `tests/omega/pass/facts/as_slice_views_through_other_state_exit/main.omg` / drop_requires → confirmed-hole

Route reason: `Unsupported: type reference`
Discard reason: ``
Obligation: `7: machine extent(entries: &[Entry; 4]) -> u64 {`

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 data Entry { value: i32; }
 machine require_four(length: u64) -> u64
-requires length == 4
 { length }
 machine extent(entries: &[Entry; 4]) -> u64 {
@@ -10,5 +9,4 @@
     transition { _ -> measure(view) }
     machine measure(view: &[Entry]) -> u64
-    requires view.len == 4
     { require_four(view.len) }
 }
```

Current checker exit=0:

```
compiled 30 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_as_slice_views_through_other_state_exit_main_omg_drop_req_2ef54112b0/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_as_slice_views_through_other_state_exit_main_omg_drop_req_2ef54112b0/main.omg	current=accepted	current_us=2922031	root=compilation	route=outside	Unsupported: type reference
```

## Hand verification — discarded drop_requires (sample)

### `tests/omega/pass/facts/projection_call_premise_through_parameter_exit/main.omg` / drop_requires → discarded-valid

Route reason: `route_us=1273 discharge_us=617 reverify_us=67 reverified=2 obligations=2 judged=2 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=4 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine project(mark: Mark) -> bool
-requires mark.kept
 {
     mark.kept
@@ -12,5 +11,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     true
@@ -18,5 +16,4 @@
 
 machine observe(mark: Mark) -> bool
-requires mark.kept
 {
     project(mark)
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_projection_call_premise_through_parameter_exit_main_omg_d_032ef57012/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_projection_call_premise_through_parameter_exit_main_omg_d_032ef57012/main.omg	current=accepted	current_us=3008719	root=compilation	route=accepted	route_us=1275	discharge_us=707	reverify_us=79	reverified=2	obligations=2	judged=2	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=4	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_atomic_release_disposition_generic_exit_main_omg_drop_mut_l_1f9aba8004/main.omg	current=accepted	current_us=2837826	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_entry_zero_branch_conditional_exit_main_omg_drop_mut_let_c37adf1f5d/main.omg	current=accepted	current_us=2932257	root=compilation	route=outside	Unsupported: machine assembly target
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_call_result_observation_exit_main_omg_7b67fba4b0/main.omg	current=accepted	current_us=3007082	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_exclusive_reference_owned_wrapper_exi_010ef1bcbf/main.omg	current=accepted	current_us=2986945	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_write_only_projected_state_receiver_exit_main_omg_drop_m_b3c3ab4d0d/main.omg	current=accepted	current_us=2995335	root=compilation	route=outside	Unsupported: machine attached data
```


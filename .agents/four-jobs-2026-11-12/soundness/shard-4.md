# Soundness mutations shard 4

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (measured `b1feb1e3441290553fe52ddaf8bd8abd42fe3b2d`). Host: linux cloud agent. Generated 2026-11-12 17:16 UTC.

## Bucket table (mutant attempts with disk dirs only)

Mutant rows in bucket table: **3620** (equals sum of buckets).
Not-applicable attempt rows (outside table): **30** (kinds with no mutant dir; former per-attempt `no-mutation`).

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 22 |
| `known-gap-q7-mut` | 162 |
| `disagreement-valid` | 59 |
| `valid-route-outside` | 2338 |
| `discarded-valid` | 748 |
| `rejected-both` | 0 |
| `rejected-current-no-route` | 291 |
| `rejected-current-route-outside` | 0 |
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
- Mutant attempt rows with dirs: **3620**; not-applicable: **30**; attempt rows total: **3650**

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

- `drop_requires` site split (header vs body): _(re-run shard4_reclassify.py if empty)_
- **known-gap-q7-mut:** 162 (canonical kind `remove_mut` unless noted)
- Confirmed holes by kind: drop_requires=22
- Confirmed holes by accepted_by: both=1, current=21

## Original-not-accepted breakdown (baseline audit)

Fixture-level **failed-run** count: **0**. All **230** are checker rejections (`current=rejected`), not harness failures.

| Category | Count | Examples |
| --- | ---: | --- |
| `current-rejected-other` | 11 | `tests/omega/pass/build/conditional_root_binding/main.omg`, `tests/omega/pass/calls/native_code_layout_discarded_exit/main.omg`, `tests/omega/pass/facts/installed_entry_storage_through_other_machine_exit/main.omg` |
| `current-rejected-proof` | 23 | `tests/omega/pass/borrows/qualified_owner_state_loan_exit/main.omg`, `tests/omega/pass/borrows/incoming_reference_slot_replacement_exit/main.omg`, `tests/omega/pass/borrows/runtime_uniform_projected_reference_selection_exit/main.omg` |
| `current-rejected-signature` | 4 | `tests/omega/pass/facts/recoverable_replacement_window_through_parameter_exit/main.omg`, `tests/omega/pass/providers/inline_provider_owner_lifetime_telescope_unapplied/main.omg`, `tests/omega/pass/traits/implementation_value_own_const_identity/main.omg` |
| `current-rejected-slow` | 192 | `tests/omega/pass/arithmetic/saturating_signed_divide_min_by_neg_one/main.omg`, `tests/omega/pass/arithmetic/runtime_integer_casts_exit/main.omg`, `tests/omega/pass/arithmetic/const_fold_unsigned_landed_ops_exit/main.omg` |

Accepted originals without mutant dirs (gap-fill re-attempted, kept=0 each): **18** — all mutation kinds not applicable on source.

## Bucket transition (c0a4894 → strict audit)

**rejected-both verification failures:** 0

| Old | New | Count | Rule |
| --- | --- | ---: | --- |
| `discarded-valid` | `valid-route-outside` | 749 | Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). |
| `rejected-both` | `rejected-current-no-route` | 291 | Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. |
| `rejected-both` | `discarded-valid` | 197 |  |
| `confirmed-hole` | `valid-route-outside` | 53 | Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. |
| `discarded-valid` | `disagreement-valid` | 27 | Checker disagreement after strict validity reassessment. |
| `confirmed-hole` | `discarded-valid` | 3 | Hole demoted: missing spec quote, obligation, or no-trap evidence. |
| `confirmed-hole` | `known-gap-q7-mut` | 2 | Ownership/Q7 mutability gap (remove_mut / shared_instead_of_mut) per lead notes. |

### Spot checks (5 per major flow)

### discarded-valid → valid-route-outside (749 rows)

- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `strip_and_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/qualified_cast_in_entry_body_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arrays/qualified_array_literal_retains_element_policy_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/asm/checked_asm_compare_one_block_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)

### discarded-valid → rejected-current-no-route (0 rows)

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/checked_asm_compare_one_block_exit/main.omg` / `drop_mut_let`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/checked_fence_aarch64_in_x86_body/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/scratch_hardware_flag_mix/main.omg` / `drop_mut_let`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)

### confirmed-hole → valid-route-outside (53 rows)

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `strip_and_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/build/default_component_abi_extents/producer/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/calls/call_local_reassign/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)

## Kind totals (canonical)

## Kind × bucket cross-table

| Kind | `confirmed-hole` | `disagreement-valid` | `discarded-valid` | `known-gap-q7-mut` | `rejected-current-no-route` | `valid-route-outside` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `delete_divisor_guard` | 0 | 15 | 37 | 0 | 0 | 773 |
| `drop_requires` | 22 | 0 | 130 | 0 | 240 | 41 |
| `remove_mut` | 0 | 0 | 10 | 160 | 15 | 162 |
| `remove_transition_guard` | 0 | 13 | 220 | 0 | 0 | 525 |
| `shared_instead_of_mut` | 0 | 0 | 293 | 2 | 36 | 49 |
| `strip_and_guard` | 0 | 16 | 40 | 0 | 0 | 521 |
| `widen_requires` | 0 | 15 | 18 | 0 | 0 | 267 |
| **Sum** | — | — | — | — | — | — |

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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_runtime_constructor_child_equal_captures_exit_main_omg_dr_bb68ee90ae/main.omg	current=accepted	current_us=3100396	root=compilation	route=outside	Unsupported: machine attached data
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_affine_discard_root_cleanup_through_field_exit_main_omg_d_0cd6a9543a/main.omg	current=accepted	current_us=3389979	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_as_slice_views_through_other_state_exit_main_omg_drop_req_2ef54112b0/main.omg	current=accepted	current_us=3110449	root=compilation	route=outside	Unsupported: type reference
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_signed_element_inclusive_exit_main_omg_drop_requ_f75bbe1213/main.omg	current=stopped	current_us=705182	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_signed_element_inclusive_exit_main_omg_drop_requ_f75bbe1213/main.omg:9:9: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `bound
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_utf8_field_exit_main_omg_drop_require_04d094f62e/main.omg	current=stopped	current_us=717967	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_utf8_field_exit_main_omg_drop_require_04d094f62e/main.omg:67:5: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `boun
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_fixed_array_reference_results_exit_main_omg_drop_requir_b9d2669b98/main.omg	current=stopped	current_us=688075	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_fixed_array_reference_results_exit_main_omg_drop_requir_b9d2669b98/main.omg:5:1: expected one of `terminates`, `decreases`, `reaches`, `invokes <binding>`, `suspends`, `blocks`, `crashes <Cause>`, `bound
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_runtime_writeonly_checked_static_carrier_exit_main_omg__4504c900fc/main.omg	current=stopped	current_us=688997	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrows_runtime_writeonly_checked_static_carrier_exit_main_omg__4504c900fc/main.omg:6:1: machine parameter `Write` requires an authored declaration-site contract: write `requires machine Write(...) -> Result;` o
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_const_boundary_uses_exact_bound_main_omg_drop_requires_53f5f292ee/main.omg	current=stopped	current_us=671394	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_calls_const_boundary_uses_exact_bound_main_omg_drop_requires_53f5f292ee/main.omg:3:1: expected identifier, found punctuation `}`")
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_atomic_release_disposition_generic_exit_main_omg_drop_mut_l_1f9aba8004/main.omg	current=accepted	current_us=2945724	root=compilation	route=outside	Unsupported: machine service reach
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_asm_entry_zero_branch_conditional_exit_main_omg_drop_mut_let_c37adf1f5d/main.omg	current=accepted	current_us=2915464	root=compilation	route=outside	Unsupported: machine assembly target
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_call_result_observation_exit_main_omg_7b67fba4b0/main.omg	current=accepted	current_us=3016787	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_runtime_write_only_exclusive_reference_owned_wrapper_exi_010ef1bcbf/main.omg	current=accepted	current_us=3058970	root=compilation	route=outside	Unsupported: parameter mutable parameter
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_borrow_write_only_projected_state_receiver_exit_main_omg_drop_m_b3c3ab4d0d/main.omg	current=accepted	current_us=3071808	root=compilation	route=outside	Unsupported: machine attached data
```


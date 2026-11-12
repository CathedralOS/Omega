# Soundness mutations shard 7 (job 2)

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

Rules: `soundness_fix` → `fix2` → **`fix3`** → **`fix4`** → **`fix5`** (latest wins).
fix5: only **external** caller re-establishment is ignored; in-machine transition/guard restatement → discarded-valid (both sides quoted).
Confirmed `drop_requires` holes: `hole_subtype` is exactly `unchecked`, `inferred-entry`, or `probe-not-built`.
Validity from reading mutants; route reasons are hints only.

## Coverage reconciliation (fix4)

- Shard fixtures (assigned): **1130**
- Fixtures with any TSV row (mutant or no-mutation): **1130**
- Fixtures finished (progress lines): **1130**
- Fixtures with ≥1 mutant row: **983**
- Fixtures with `no-mutation` placeholder: **147**
- Mutant rows (excl. no-mutation placeholders): **2782**
- Total TSV data rows: **2929**
- Runner `generated_pre_dedupe`: **2782**
- Runner `generated_post_dedupe` (after source-hash collapse): **2782**

### Generated vs TSV row accounting

| Reason | Count |
| --- | ---: |
| `generated_pre_dedupe` (mutation candidates before hash dedupe) | 2782 |
| `generated_post_dedupe` (unique mutation sources, ≤12/fixture) | 2782 |
| `source_hash_collapsed` (pre − post) | 0 |
| `tsv_rows` | 2929 |
| `no_mutation_rows` | 147 |
| `non_candidate_rows` (still recorded) | 2289 |
| Expected rows if every post-dedupe mutant + no-mutation emitted | 2929 |
| Δ vs tsv_rows (pre-backfill historical gap) | 0 |

| `expected_tsv_rows_after_full_emit` | 2929 |
| `fixtures_missing_all_rows` | 0 |
| `source_hash_collapsed` | 0 |

Prior incomplete run: **203** fixtures had progress but **no TSV rows** (~152 mutant rows never emitted, **0** `no-mutation` placeholders). Incremental rebuild closed the gap with **147** `no-mutation` rows and backfilled mutants.

### Retired prior `failed-run` rows (12)

Those 12 were **not** checker failures on distinct mutants. Review had written a multi-line `drop_requires` obligation into `shard-7.tsv`; newline splits produced phantom rows whose `original_fixture` was code (`first: holder.amount,`, etc.). Classification saw missing files → `failed-run`.

Single real fixture: `tests/omega/pass/calls/runtime_constructor_child_named_state_captures_exit/main.omg`.
After rebuild + `_field()` sanitization, the lone `drop_requires` mutant row is **`rejected-both`** (checker rejects; modular entry hole not accepted). Related rows on the same fixture: `widen_requires` rejected-both, `remove_mut` known-gap-q7-mut, `shared_instead_of_mut` rejected-both.

**`disagreement-valid`:** also **0** in the prior reviewed TSV (`d90c7e9`); real accept-vs-reject disagreements were already `rejected-both` or `valid-route-outside` under fix3/fix4.

Fixtures with more than one mutant row (first 20):
- `tests/omega/pass/borrows/runtime_dependent_window_byte_payload_exit/main.omg`: **7** rows
- `tests/omega/pass/borrows/runtime_dyn_image_result_snapshot_exit/main.omg`: **7** rows
- `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_local_exit/main.omg`: **7** rows
- `tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_field_exit/main.omg`: **7** rows
- `tests/omega/pass/ownership/runtime_replacement_call_child_snapshot_exit/main.omg`: **7** rows
- `tests/omega/pass/slices/runtime_indexed_store_operand_effect_order_exit/main.omg`: **7** rows
- `tests/omega/pass/asm/checked_asm_nominal_symbolic_disjunction_exit/main.omg`: **6** rows
- `tests/omega/pass/borrows/runtime_computed_case_copy_order_exit/main.omg`: **6** rows
- `tests/omega/pass/collections/typed_run_sum_array_exit/main.omg`: **6** rows
- `tests/omega/pass/core/address_interval_partition_unrelated_store_exit/main.omg`: **6** rows
- `tests/omega/pass/core/content_conservation_selected_parameter_transport/main.omg`: **6** rows
- `tests/omega/pass/drops/result_cleanup_original_exit/main.omg`: **6** rows
- `tests/omega/pass/drops/runtime_cleanup_helper_normal_guarantee_exit/main.omg`: **6** rows
- `tests/omega/pass/drops/runtime_parent_hook_preserves_linear_descendants_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/checked_asm_nominal_predicate_through_call_result_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/checked_asm_nominal_predicate_through_target_body_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/computed_nested_state_loan_through_match_join_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/computed_record_window_through_local_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/content_reunion_through_parameter_exit/main.omg`: **6** rows
- `tests/omega/pass/facts/copied_call_argument_snapshot_through_other_machine_exit/main.omg`: **6** rows

## Bucket table

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 7 |
| `discarded-valid` | 343 |
| `known-gap-q7-mut` | 230 |
| `no-mutation` | 147 |
| `rejected-both` | 1932 |
| `timeout` | 1 |
| `valid-route-outside` | 269 |
| **sum** | **2929** |

### Confirmed holes by kind
- `drop_requires`: 7

### Confirmed holes by accepted_by
- `current`: 7

### Confirmed drop_requires holes by hole_subtype
- `inferred-entry`: 6
- `probe-not-built`: 1

## discarded-valid breakdown

### By discard_class (why accepted)
- `unclassified`: 135
- `ref-signature-or-call-only`: 99
- `mut-ref-stricter-signature`: 80
- `ensures-weaken`: 20
- `drop-req-body-restate`: 4
- `mut-ref-reborrow-from-mut-place`: 3
- `confirmed-downgrade`: 2

### By mutation kind
- `shared_instead_of_mut`: 99
- `remove_mut`: 96
- `mut_ref_instead_of_shared`: 83
- `drop_requires`: 45
- `drop_ensures`: 20

### Hand-check: 10 discarded-valid across reasons

- **confirmed-downgrade** `drop_requires` `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_other_state_exit/main.omg`
  - note: confirmed downgrade: runtime trap/crash route present (FOUND L30:_ -> bad(); L34:machine bad() -> i32 { 1 }; L41:machine main() crashes Trap {; L45:_ -> bad())
```diff
-     requires x == 10
+     ensures result == 11
-     ensures result == 11
+ {
- {
+     11
-     11
+ }
```
- **confirmed-downgrade** `drop_requires` `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_machine_exit/main.omg`
  - note: confirmed downgrade: runtime trap/crash route present (FOUND L31:machine main() crashes Trap {; L37:_ -> bad(); L40:machine bad() { crash Trap; })
```diff
-     requires raw >= 59 && raw <= 59
+ {
- {
+     raw ^ 15u64
-     raw ^ 15u64
+ }
- }
+ 
```
- **drop-req-body-restate** `drop_requires` `tests/omega/pass/facts/signed_arithmetic_shift_right_through_call_result_exit/main.omg`
  - note: valid: in-machine body/guard restates removed requires | requires: `diff >= 0i64 && diff <= 0i64` | restate: `diff`
```diff
-     requires diff >= 0i64 && diff <= 0i64
+ {
- {
+     diff
-     diff
+ }
- }
+ 
```
- **drop-req-body-restate** `drop_requires` `tests/omega/pass/facts/signed_exact_addition_bound_through_other_machine_exit/main.omg`
  - note: valid: in-machine body/guard restates removed requires | requires: `margin >= 0i64 && margin <= 40i64` | restate: `margin`
```diff
-     requires margin >= 0i64 && margin <= 40i64
+ {
- {
+     margin
-     margin
+ }
- }
+ 
```
- **ensures-weaken** `drop_ensures` `tests/omega/pass/arithmetic/full_width_integer_literals_compose_exit/main.omg`
  - note: ensures removal weakens only
```diff
- ensures result == value - 1
+ { value - 1u64 }
- { value - 1u64 }
+ 
- 
+ machine main() crashes Trap {
- machine main() crashes Trap {
+     let returned: u64 = maximum();
```
- **ensures-weaken** `drop_ensures` `tests/omega/pass/arithmetic/runtime_unbounded_zero_product_exit/main.omg`
  - note: ensures removal weakens only
```diff
-     ensures result == 0
+ {
- {
+     value * 0
-     value * 0
+ }
- }
+ machine left_zero(value: u64) -> u64
```
- **ensures-weaken** `drop_ensures` `tests/omega/pass/facts/bounded_scalar_sums_through_call_result_exit/main.omg`
  - note: ensures removal weakens only
```diff
-     ensures result <= 50
+ {
- {
+     value
-     value
+ }
- }
+ 
```
- **mut-ref-stricter-signature** `mut_ref_instead_of_shared` `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_other_state_exit/main.omg`
  - note: valid: `&mut` at least as strict as `&` without shared-slot violation
```diff
- // Fact: masking an unsigned 64-bit integer to its low byte (val & 255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.
+ // Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.
```
- **mut-ref-stricter-signature** `mut_ref_instead_of_shared` `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_local_exit/main.omg`
  - note: valid: `&mut` at least as strict as `&` without shared-slot violation
```diff
- // Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x & 31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.
+ // Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut  31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.
```
- **mut-ref-stricter-signature** `mut_ref_instead_of_shared` `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_call_result_exit/main.omg`
  - note: valid: `&mut` at least as strict as `&` without shared-slot violation
```diff
- // Fact: 64-bit signed bitwise AND mask raw & 63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.
+ // Fact: 64-bit signed bitwise AND mask raw &mut  63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.
```

## Confirmed holes (hand basis: obligation + spec quote)

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/control_flow/record_result_dispatch_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__control_flow__record_result_dispatch_compile__main.omg/drop_requires_b139efc8d2/main.omg`
- omega **accepted**, current **accepted**, route **no-entry** (route outside is not acceptance)
- Obligation (check-time): `machine `Main::pick(n: u64) -> Pair` missing entry `n <= 9``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `Main::pick(n: u64) -> Pair` missing entry `n <= 9` | fix4 airtight | probe pick(0) rejected (exit 1)
```diff
- requires n <= 9
+ {
- {
+     transition n {
-     transition n {
+         9 -> wrap(n)
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/drops/runtime_nominal_state_loan_nonlending_branch_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__drops__runtime_nominal_state_loan_nonlending_branch_exit__main.omg/drop_requires_eab7e56c6e/main.omg`
- omega **accepted**, current **accepted**, route **outside** (route outside is not acceptance)
- Obligation (check-time): `machine `drop_room(room:` missing entry `room.count == 10``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `drop_room(room:` missing entry `room.count == 10` | fix4 airtight | probe drop_room(0) rejected (exit 1)
```diff
- requires room.count == 10
+ reaches Console
- reaches Console
+ {
- {
+  Console::write_byte(68);
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/generics/mixed_value_nullary_case_checked/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__generics__mixed_value_nullary_case_checked__main.omg/drop_requires_9c6180c1c5/main.omg`
- omega **accepted**, current **accepted**, route **outside** (route outside is not acceptance)
- Obligation (check-time): `removed requires `((self in Tagged::Empty) && (Limit<10))``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): removed requires `((self in Tagged::Empty) && (Limit<10))` | fix4 airtight
```diff
-     requires ((self in Tagged::Empty) && (Limit<10))
+ {case Empty;}
- {case Empty;}
+ machine main(){let item:Tagged<u64,9> = Tagged::Empty;}
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/proofs/embedded_successor_call_requirement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__embedded_successor_call_requirement_exit__main.omg/drop_requires_7e84fe46ee/main.omg`
- omega **accepted**, current **accepted**, route **outside** (route outside is not acceptance)
- Obligation (check-time): `machine `require_length<Length: u64>()` missing entry `Length <= 8``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `require_length<Length: u64>()` missing entry `Length <= 8` | fix4 airtight | probe require_length<Length: u64>(0) rejected (exit 1)
```diff
- requires Length <= 8
+ {}
- {}
+ machine transfer<Length: u64, NextLength: u64>()
- machine transfer<Length: u64, NextLength: u64>()
+ requires Length <= 7 && embed(Length) + 1 == embed(NextLength)
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/proofs/proposition_const_operand_application/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__proposition_const_operand_application__main.omg/drop_requires_9ef71b3696/main.omg`
- omega **accepted**, current **accepted**, route **no-entry** (route outside is not acceptance)
- Obligation (check-time): `machine `inspect(value:` missing entry `valid<3u64>(value)``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `inspect(value:` missing entry `valid<3u64>(value)` | fix4 airtight | probe inspect(0) rejected (exit 1)
```diff
- requires valid<3u64>(value)
+ {}
- {}
+ 
- 
+ machine main() {}
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/providers/boundary_extent_caller/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__providers__boundary_extent_caller__main.omg/drop_requires_3cda6bc2e3/main.omg`
- omega **accepted**, current **accepted**, route **no-entry** (route outside is not acceptance)
- Obligation (check-time): `machine `pump(buffer:` missing entry `64 <= buffer.len``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `pump(buffer:` missing entry `64 <= buffer.len` | fix4 airtight | probe pump(0) rejected (exit 1)
```diff
-         requires 64 <= buffer.len
+     {
-     {
+         let count: i64 = ByteHost::read(0, buffer, 64);
-         let count: i64 = ByteHost::read(0, buffer, 64);
+     }
```

### drop_requires | accepted_by=current
- Fixture: `tests/omega/pass/termination/external_result_ranked_arrival_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__termination__external_result_ranked_arrival_compile__main.omg/drop_requires_560635e41b/main.omg`
- omega **accepted**, current **accepted**, route **no-entry** (route outside is not acceptance)
- Obligation (check-time): `machine `absolute(value: i32) -> i32` missing entry `value >= 0 && value <= 9``
- Spec: `wiki/spec/language/machines.md` — ## Nested machines and identity
> A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.
- fix4b / fix3: runtime trap scan: `none (no crash/trap/bad() in mutant source)`
- not-Q7: (n/a)
- Hand note: invalid (modular entry): machine `absolute(value: i32) -> i32` missing entry `value >= 0 && value <= 9` | fix4 airtight | probe absolute(0) rejected (exit 1)
```diff
-     requires value >= 0 && value <= 9
+     ensures result == value;
-     ensures result == value;
+ }
- }
+ data ForeignProvider {}
```

## Hand verification samples

### 5 confirmed holes

- `drop_requires` `tests/omega/pass/control_flow/record_result_dispatch_compile/main.omg` — invalid (modular entry): machine `Main::pick(n: u64) -> Pair` missing entry `n <= 9` | fix4 airtight | probe pick(0) rejected (exit 1)
```diff
- requires n <= 9
+ {
- {
+     transition n {
-     transition n {
+         9 -> wrap(n)
-         9 -> wrap(n)
+         _ -> flat(n)
-         _ -> flat(n)
+     }
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__control_flow__record_result_dispatch_compile__main.omg/drop_requires_b139efc8d2/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__control_flow__record_result_dispatch_compile__main.omg/drop_requires_b139efc8d2/main.omg	current=accepted	current_us=1496723	root=compilation	route=no-entry

- `drop_requires` `tests/omega/pass/drops/runtime_nominal_state_loan_nonlending_branch_exit/main.omg` — invalid (modular entry): machine `drop_room(room:` missing entry `room.count == 10` | fix4 airtight | probe drop_room(0) rejected (exit 1)
```diff
- requires room.count == 10
+ reaches Console
- reaches Console
+ {
- {
+  Console::write_byte(68);
-  Console::write_byte(68);
+  room.count = 99;
-  room.count = 99;
+ }
```
- omega exit=0
- compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__drops__runtime_nominal_state_loan_nonlending_branch_exit__main.omg/drop_requires_eab7e56c6e/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__drops__runtime_nominal_state_loan_nonlending_branch_exit__main.omg/drop_requires_eab7e56c6e/main.omg	current=accepted	current_us=3399649	root=compilation	route=outside	Unsupported: machine service reach

- `drop_requires` `tests/omega/pass/generics/mixed_value_nullary_case_checked/main.omg` — invalid (modular entry): removed requires `((self in Tagged::Empty) && (Limit<10))` | fix4 airtight
```diff
-     requires ((self in Tagged::Empty) && (Limit<10))
+ {case Empty;}
- {case Empty;}
+ machine main(){let item:Tagged<u64,9> = Tagged::Empty;}
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__generics__mixed_value_nullary_case_checked__main.omg/drop_requires_9c6180c1c5/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__generics__mixed_value_nullary_case_checked__main.omg/drop_requires_9c6180c1c5/main.omg	current=accepted	current_us=3111947	root=compilation	route=outside	Unsupported: type generic data

- `drop_requires` `tests/omega/pass/proofs/embedded_successor_call_requirement_exit/main.omg` — invalid (modular entry): machine `require_length<Length: u64>()` missing entry `Length <= 8` | fix4 airtight | probe require_length<Length: u64>(0) rejected (exit 1)
```diff
- requires Length <= 8
+ {}
- {}
+ machine transfer<Length: u64, NextLength: u64>()
- machine transfer<Length: u64, NextLength: u64>()
+ requires Length <= 7 && embed(Length) + 1 == embed(NextLength)
- requires Length <= 7 && embed(Length) + 1 == embed(NextLength)
+ {
- {
+     require_length<NextLength>();
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__embedded_successor_call_requirement_exit__main.omg/drop_requires_7e84fe46ee/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__embedded_successor_call_requirement_exit__main.omg/drop_requires_7e84fe46ee/main.omg	current=accepted	current_us=3275825	root=compilation	route=outside	Unsupported: machine generic parameters

- `drop_requires` `tests/omega/pass/proofs/proposition_const_operand_application/main.omg` — invalid (modular entry): machine `inspect(value:` missing entry `valid<3u64>(value)` | fix4 airtight | probe inspect(0) rejected (exit 1)
```diff
- requires valid<3u64>(value)
+ {}
- {}
+ 
- 
+ machine main() {}
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__proposition_const_operand_application__main.omg/drop_requires_9ef71b3696/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__proofs__proposition_const_operand_application__main.omg/drop_requires_9ef71b3696/main.omg	current=accepted	current_us=1451200	root=compilation	route=no-entry

### 5 discarded drop_requires (current accepts)

- `drop_requires` `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_other_state_exit/main.omg` — confirmed downgrade: runtime trap/crash route present (FOUND L30:_ -> bad(); L34:machine bad() -> i32 { 1 }; L41:machine main() crashes Trap {; L45:_ -> bad())
```diff
-     requires x == 10
+     ensures result == 11
-     ensures result == 11
+ {
- {
+     11
-     11
+ }
- }
+ 
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__live_across_two_calls_preservation_bound_through_other_state_exit__main.omg/drop_requires_b80f167658/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__live_across_two_calls_preservation_bound_through_other_state_exit__main.omg/drop_requires_b80f167658/main.omg	current=accepted	current_us=3478723	root=compilation	route=accepted	route_us=11941	discharge_us=10517	reverify_us=8771	reverified=12	obligations=14	judged=12	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	publi

- `drop_requires` `tests/omega/pass/facts/projection_call_premise_through_field_exit/main.omg` — no removed requires
```diff
- requires mark.kept
+ {
- {
+     mark.kept
-     mark.kept
+ }
- }
+ 
- 
+ machine hold(direct: bool) -> bool
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__projection_call_premise_through_field_exit__main.omg/drop_requires_9fa9392ad7/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__projection_call_premise_through_field_exit__main.omg/drop_requires_9fa9392ad7/main.omg	current=accepted	current_us=3303523	root=compilation	route=incomplete	route_us=999	discharge_us=429	reverify_us=66	reverified=2	obligations=3	judged=2	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=4	unestablish

- `drop_requires` `tests/omega/pass/facts/range_fact_delivery_through_call_result_exit/main.omg` — no removed requires
```diff
- requires index <= 4
+ { index }
- { index }
+ machine source(input: u64) -> u64 requires input < 4 ensures result < 4 { input }
- machine source(input: u64) -> u64 requires input < 4 ensures result < 4 { input }
+ machine probe(input: u64) -> u64 requires input < 4 { count(source(input)) }
- machine probe(input: u64) -> u64 requires input < 4 { count(source(input)) }
+ machine main() crashes Trap {
- machine main() crashes Trap {
+     let observed: u64 = probe(3);
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__range_fact_delivery_through_call_result_exit__main.omg/drop_requires_bd26803be7/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__range_fact_delivery_through_call_result_exit__main.omg/drop_requires_bd26803be7/main.omg	current=accepted	current_us=3208151	root=compilation	route=accepted	route_us=1985	discharge_us=1347	reverify_us=763	reverified=8	obligations=9	judged=8	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=0	unestabl

- `drop_requires` `tests/omega/pass/facts/signed_arithmetic_shift_right_through_call_result_exit/main.omg` — valid: in-machine body/guard restates removed requires | requires: `diff >= 0i64 && diff <= 0i64` | restate: `diff`
```diff
-     requires diff >= 0i64 && diff <= 0i64
+ {
- {
+     diff
-     diff
+ }
- }
+ 
- 
+ machine supply_val(val: i64) -> i64
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__signed_arithmetic_shift_right_through_call_result_exit__main.omg/drop_requires_be51a89af9/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__signed_arithmetic_shift_right_through_call_result_exit__main.omg/drop_requires_be51a89af9/main.omg	current=accepted	current_us=3000599	root=compilation	route=rejected	route_us=2899	discharge_us=2177	reverify_us=1386	reverified=9	obligations=12	judged=9	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	use

- `drop_requires` `tests/omega/pass/facts/signed_exact_addition_bound_through_other_machine_exit/main.omg` — valid: in-machine body/guard restates removed requires | requires: `margin >= 0i64 && margin <= 40i64` | restate: `margin`
```diff
-     requires margin >= 0i64 && margin <= 40i64
+ {
- {
+     margin
-     margin
+ }
- }
+ 
- 
+ machine forward_sum(mid: i64) -> i64
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__signed_exact_addition_bound_through_other_machine_exit__main.omg/drop_requires_d82f31b8cf/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__signed_exact_addition_bound_through_other_machine_exit__main.omg/drop_requires_d82f31b8cf/main.omg	current=accepted	current_us=3230936	root=compilation	route=accepted	route_us=25909	discharge_us=25272	reverify_us=22897	reverified=6	obligations=6	judged=6	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	u

### 5 known-gap Q7 remove_mut

- `remove_mut` `tests/omega/pass/arithmetic/call_argument_snapshot_survives_write_exit/main.omg` — binding `cell` lacks `mut` but is written or `&mut`-loaned
```diff
-     let mut cell: Cell = Cell { value: 1u32 };
+     let cell: Cell = Cell { value: 1u32 };
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__call_argument_snapshot_survives_write_exit__main.omg/remove_mut_8f93f94ace/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__call_argument_snapshot_survives_write_exit__main.omg/remove_mut_8f93f94ace/main.omg	current=accepted	current_us=3116680	root=compilation	route=outside	Unsupported: parameter mutable parameter

- `remove_mut` `tests/omega/pass/arithmetic/runtime_captured_call_result_subtracts_itself_exit/main.omg` — binding `flag` lacks `mut` but is written or `&mut`-loaned
```diff
-     let mut flag: u64 = 0;
+     let flag: u64 = 0;
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__runtime_captured_call_result_subtracts_itself_exit__main.omg/remove_mut_233016365f/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__runtime_captured_call_result_subtracts_itself_exit__main.omg/remove_mut_233016365f/main.omg	current=accepted	current_us=3345413	root=compilation	route=outside	Unsupported: parameter mutable parameter

- `remove_mut` `tests/omega/pass/borrow/runtime_write_only_affine_sum_exit/main.omg` — binding `entry_storage` lacks `mut` but is written or `&mut`-loaned
```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_affine_sum_exit__main.omg/remove_mut_3842ac4802/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_affine_sum_exit__main.omg/remove_mut_3842ac4802/main.omg	current=accepted	current_us=3187418	root=compilation	route=outside	Unsupported: type data with custody

- `remove_mut` `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_operand_exit/main.omg` — binding `held` lacks `mut` but is written or `&mut`-loaned
```diff
-     let mut held: Held = Held { target: &mut main.left, stamp: 17 };
+     let held: Held = Held { target: &mut main.left, stamp: 17 };
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_call_operand_exit__main.omg/remove_mut_c1fe05d15f/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_call_operand_exit__main.omg/remove_mut_c1fe05d15f/main.omg	current=accepted	current_us=3309559	root=compilation	route=outside	Unsupported: parameter mutable parameter

- `remove_mut` `tests/omega/pass/borrow/runtime_write_only_indexed_record_copy_exit/main.omg` — binding `entry_storage` lacks `mut` but is written or `&mut`-loaned
```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```
- omega exit=0
- compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_indexed_record_copy_exit__main.omg/remove_mut_c3dcdea5f6/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_indexed_record_copy_exit__main.omg/remove_mut_c3dcdea5f6/main.omg	current=accepted	current_us=3134374	root=compilation	route=outside	Unsupported: parameter mutable parameter

### mut_ref `let view: &[T] = &mut ...` (prior confirmed pair)

- `arithmetic/every_width_index`: source is `let values` (**not** `mut`) → **`known-gap-q7-mut`** (Q7: `&mut` of non-mut local); no separate aliasing spec.
- `slices/subslice_folded_bound_facts_compile`: source is `main: &mut Main` → valid shared reborrow from mut place → **`valid-route-outside`** / discard_class `mut-ref-reborrow-from-mut-place` (fix4 routes `no-entry`).

### shared_instead_of_mut confirmed holes (not Q7)


### Hand read: all mut_ref_instead_of_shared rows

- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/arithmetic/every_width_index/main.omg` | `let view: &[u64] = &mut values[..];` | Q7: source `let values` without `mut`; `let view: &[u64] = &mut values[..];`
- **rejected-both** accepted_by=- | `tests/omega/pass/arithmetic/relative_midpoint_values_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/asm/atomic_release_disposition_reference_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/asm/observation_preserves_address_model/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/atomics/runtime_nested_case_payload_atomic_event_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrow/owned_capture_replaced_reference/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrow/runtime_write_only_affine_sum_exit/main.omg` | `machine replace(reading: &mut write Reading, incoming: Reading) { reading = produce(incoming); }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_operand_exit/main.omg` | `held.target.value = 89;` | invalid: callee writes through `&mut` parameter `held`
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrow/runtime_write_only_indexed_record_copy_exit/main.omg` | `machine replace(cells: &mut write [Cell; 2], position: u64, incoming: Cell)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_capture_exit/main.omg` | `data Held { target: &mut Cell; stamp: i64; }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/borrow/storage_address_loop_copy_identity_exit/main.omg` | `let original_address: addr = address_of(&mut original[0]);` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/case_payload_state_loan_exit/main.omg` | `machine read(envelope: &mut Envelope) -> u64 {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/constant_shared_boundary_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/copied_input_descriptor_local_copy_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/foreign_output_exclusive_anchor_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/inline_carried_reference_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/nested_machine_carried_recurrent_selector_exit/main.omg` | `false -> &mut values[index]` | Q7: non-mut `let values` with `false -> &mut values[index]`
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/nested_reference_crossings_exit/main.omg` | `data Message { stamp: i32; body: &mut Cell; }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/opaque_incoming_nested_projected_record_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/overwritten_intermediate_descriptor_releases_backing_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/predicate_reference_field_retarget_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/rebound_parameter_keeps_earlier_copy_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/borrows/retained_envelope_refs/main.omg` | `// a retained reference (`&mut retained` where retained is itself `&i32`) — plus a` | Q7: non-mut `let retained` with `// a retained reference (`&mut retained` where retained is itself `&i32`) — plus a`
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/runtime_byte_view_element_loan_exit/main.omg` | `// A call lends one element of a byte view (`&mut bytes[i]` over `&[u8]`,` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/borrows/runtime_computed_case_copy_order_exit/main.omg` | `let second: u32 in Wrapping = local_copy(&mut cells, &mut cursor);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_copied_byte_predicate_snapshot_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_dependent_window_byte_payload_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_dyn_image_result_snapshot_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_indexed_reference_call_regions/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/borrows/runtime_measured_range_record_domains_exit/main.omg` | `transition visited == 70 && shifted == 70 && TextBuffer::equal(&mut main.rooms[1].label, "vault") {` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/runtime_nested_reference_array_bounded_read_exit/main.omg` | `let messages: [[&mut Cell; 2]; 2] = [[&main.first, &main.second], [&main.second, &main.first]];` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_owned_reference_carrier_scalar_operand_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/borrows/runtime_reference_carrier_state_loan_exit/main.omg` | `machine inspect(envelope: &mut Envelope<'storage>) -> u64 {` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/runtime_reference_result_alias_case_payload_exit/main.omg` | `machine has_value(cell: &mut Cell, expected: u64 in Wrapping) -> bool` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_scalar_call_stored_carrier_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_selected_stored_carrier_retired_parent_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_shared_temporary_call_exit/main.omg` | `machine Main::collect(context: &mut Main, first: &mut Item, second: &Item) -> bool {` | Q7: source `let Main, first: &mut Item, second: &Item) -> bool {` without `mut`; `machine Main::collect(context: &mut Main, first: &mut Item, second: &Item) -> bool {`
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_stored_boolean_reference_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/borrows/runtime_tail_recursive_disjoint_field_fact_exit/main.omg` | `machine require_bound(work: &mut Work) requires work.count <= 8 { }` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/runtime_writeonly_reference_array_state_exit/main.omg` | `targets[index].value = 71;` | invalid: callee writes through `&mut` parameter `value`
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/static_carrier_reference_read_exit/main.omg` | `data Message { stamp: i32; body: &mut Cell; }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/structural_operand_view_catalog_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/suspension_projected_call_view_reborrow_selected_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/suspension_saved_reference_view_earlier_operand_capture_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/borrows/target_family_backing_alternatives_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/borrows/writeonly_record_field_store_variants/main.omg` | `// the referent's own nested record. Every `&mut write` carrier is an exclusive` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/build/native_component_case_loan_cycle/producer/main.omg` | `let result: u64 = block poll(&mut gate, &mut peer, 1u64);` | Q7: non-mut `let gate` with `let result: u64 = block poll(&mut gate, &mut peer, 1u64);`
- **discarded-valid** accepted_by=- | `tests/omega/pass/build/uefi_selected_context_entry/main.omg` | `machine enter(image_handle: EfiImageHandle, system_table: &mut EfiSystemTable) -> EfiStatus` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/ambient_receiver_scalar_return_exit/main.omg` | `// Borrowed-receiver scalar return: `count(&mut main.buf)` calls a` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/local_record_leaf_loan_scalar_calls_exit/main.omg` | `// machine: a primitive field (`&mut pair.high`) and one element of a byte-array` | Q7: non-mut `let pair` with `// machine: a primitive field (`&mut pair.high`) and one element of a byte-array`
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/native_code_layout_lifetime_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/nested_self_call_argument_exit/main.omg` | `// `Sink::probe(&mut main.sink, bytes[..])` — probe is a `&self` method whose whole-view` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/runtime_atomic_case_shared_argument_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/runtime_constructed_borrowed_payload_arguments_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/runtime_cyclic_state_reference_result_exit/main.omg` | `machine shared_identity(node: &mut Node) -> &Node { node }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/runtime_foreign_scalar_indirect_widths_exit/main.omg` | `plan.call.result.shape.class = scalar_class(&mut signature.shapes[result]);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/runtime_heterogeneous_case_argument_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/runtime_lifetime_only_constructor_result_exit/main.omg` | `machine Output::read_value(output: &mut Output) -> u64 { output.value }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/calls/runtime_temporary_lending_order_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/calls/scalar_range_argument_exit/main.omg` | `let observed: u64 = Helper::measure(&mut main.bytes[1..41]);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/calls/structural_mut_to_shared_argument/main.omg` | `machine Counts::clone(source: &mut Counts) -> Counts {` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/cases/mid_arm_value_return/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/cases/runtime_constructor_subject_range_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/cases/write_only_case_postcondition_exit/main.omg` | `machine initialize(destination: &mut write Choice)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/collections/runtime_offset_view_case_observation_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/collections/runtime_projected_range_call_nonempty_requirement_exit/main.omg` | `exercise(&mut matrix, &mut cursor);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/collections/runtime_shared_array_dot_product_exit/main.omg` | `let sum: i32 in Wrapping = compute_dot_product(&mut a, &b);` | Q7: non-mut `let a` with `let sum: i32 in Wrapping = compute_dot_product(&mut a, &b);`
- **discarded-valid** accepted_by=- | `tests/omega/pass/collections/runtime_three_indexed_record_float_read_exit/main.omg` | `machine read(storage: &mut Storage, layer: u64, row: u64, column: u64) -> f64` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/collections/typed_run_sum_array_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/concurrency/ref_array_suspend/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/contracts/borrowed_descriptor_guarded_end_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/contracts/completed_unit_call_post_case/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/contracts/runtime_float_field_crash_guard_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/contracts/runtime_literal_view_extent_calls_exit/main.omg` | `let bounded_length: u64 = two_bytes(&mut bounded);` | Q7: non-mut `let bounded` with `let bounded_length: u64 = two_bytes(&mut bounded);`
- **discarded-valid** accepted_by=- | `tests/omega/pass/control_flow/borrowed_case_dispatch_payload_arguments_exit/main.omg` | `machine Duo::code(duo: &mut Duo) -> u64 in Wrapping {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/control_flow/guarded_mid_arm_return/main.omg` | `machine Probe::classify(probe: &mut Probe, value: u64) -> u64 {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/control_flow/pattern_arm_guarded_index_argument_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/control_flow/runtime_scalar_case_arm_evaluation_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/core/address_interval_partition_unrelated_store_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/core/content_conservation_interval_split_exit/main.omg` | `public machine Owned::content(region: &mut Region) -> IntervalSet<ByteUnit>` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/core/content_conservation_selected_parameter_transport/main.omg` | `public machine Owned::content(region: &mut Region) -> CountedQuantity<ByteUnit>` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/core/content_conservation_wrapper_match_independent_body_exit/main.omg` | `public machine Owned::content(region: &mut Region) -> CountedQuantity<ByteUnit>` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/core/independent_content_indexed_requirement/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/core/numeric_trapping_negative_to_unsigned/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/core/numeric_widening_published_ranges/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/dependent/collection_predicate_element_store_restored/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/dependent/runtime_computed_slice_index_relation_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/domains/carried_runtime_index_payload_qualification/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/domains/domain_intersection/main.omg` | `// `password in Password::Valid &mut  Password::Secure` — each member's` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/domains/domain_pattern_match_bound_generic_equation_same_carrier_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/domains/domain_pattern_match_bound_predicate_helper_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/domains/indexed_domain_fact/main.omg` | `// `ensures result in Granted &mut  Resident<SlotPlacement, Slot>` is` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/domains/linear_routed_erasure_keeps_storage_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/domains/routed_selected_copy_exit/main.omg` | `let observed: u64 = select(&mut frames, 9);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/domains/runtime_qualified_borrowed_gated_input_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/borrowed_projection_moved_aggregate_capture_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/result_cleanup_original_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/runtime_borrowed_temporary_projection_cleanup_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/runtime_generic_cleanup_owner_telescope_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/runtime_mixed_temporary_effects_continuation_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/runtime_parent_hook_preserves_linear_descendants_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/drops/skipped_claim_temporary/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/expressions/owned_match_record_values/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/additional_input_domain_mutation_through_parameter_exit/main.omg` | `cell.value = 73;` | invalid: callee writes through `&mut` parameter `cell`
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/address_count_partition_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/aggregate_storage_address_identity_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/applied_zero_establishment_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/applied_zero_establishment_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/array_element_default_domain/parameter/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/as_slice_views_generic_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/as_slice_views_through_const_name_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_boolean_store/other_state/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_bounded_fetch/other_machine/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_bounded_resident/match_join/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_compare_exchange_once_operand_effect_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_compare_exchange_once_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/atomic_try_exchange_once/field/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/borrowed_case_result_through_local_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/borrowed_projection_cleanup_through_const_name_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/borrowed_store_call_through_parameter_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/c_layout_nested_representation_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/c_layout_scalar_record_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/captured_storage_bounds_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/case_payload_stored_loan_through_match_join_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/checked_asm_add_carry_through_call_result_exit/main.omg` | `machine capture() -> u64 ensures (result &mut  2261u64) == 85u64 {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/checked_asm_add_carry_through_target_body_exit/main.omg` | `observed requires (snapshot &mut  2261u64) == 85u64:` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/checked_asm_fence_contracts_through_match_join_exit/main.omg` | `path = 1;` | invalid: callee writes through `&mut` parameter `path`
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/checked_asm_flag_restore_through_field_exit/main.omg` | `transition (holder.saved &mut  64u64) == 64u64 {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/checked_asm_flag_snapshot_through_const_name_exit/main.omg` | `observed requires (snapshot &mut  ZF_MASK) == ZF_MASK:` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/completed_integer_carrier_bounds_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/component_scalar_lending_through_call_result_exit/producer/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/component_scalar_lending_through_match_join_exit/producer/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/component_scalar_lending_through_target_body_exit/producer/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/computed_constant_array_extent_through_other_machine_exit/main.omg` | `transition Inspector::check(&mut local_arr) { true -> done() _ -> bad() }` | Q7: non-mut `let local_arr` with `transition Inspector::check(&mut local_arr) { true -> done() _ -> bad() }`
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/computed_subslice_extent_through_const_name_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/content_interval_adjacency_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/content_interval_adjacency_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/content_reunion_through_parameter_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/crash_entry_ieee_view_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/crash_entry_ieee_view_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/crash_entry_nonempty_path_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/crash_entry_representation_view_through_other_machine_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/darwin_stat_abi_through_local_exit/main.omg` | `machine check(value: &mut DarwinStat) -> u64 reaches;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_borrowed_field_array_field/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_borrowed_field_disjoint_local_write_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_borrowed_field_through_parameter_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_nested_borrowed_field_call_result/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_nested_borrowed_field_match_join/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_predicate_helper_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/data_where_standing_through_local_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/data_where_standing_write_only_const_name/main.omg` | `public machine fill(counter: &mut write Counter, replacement: i32)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/data_where_standing_write_only_predicate_helper/main.omg` | `public machine fill(counter: &mut write Counter, replacement: i32)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/dependency_type_qualified_call_through_const_name_exit/main.omg` | `let total: u64 = lib::ops::Wrapped::describe(&mut left, &right);` | Q7: non-mut `let left` with `let total: u64 = lib::ops::Wrapped::describe(&mut left, &right);`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/dependent_subtract_through_call_result_compile/main.omg` | `machine used_amount(main: &mut Main) -> u32` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/descriptor_replacement_order_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/domain_pattern_match_bound_through_parameter_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/domain_selected_cleanup_computed_other_machine_exit/main.omg` | `machine observe(keeper: &mut Keeper in Held<7>) {}` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_computed_exit/main.omg` | `machine observe(keeper: &mut Keeper) {}` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/domain_selected_cleanup_through_other_machine_exit/main.omg` | `machine observe(keeper: &mut Keeper) {}` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/erased_reference_contract_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/field_where_index_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/field_where_index_through_target_body_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/finite_family_input_index/other_machine/main.omg` | `check_observations(&mut observed, expected_marker, width, expected_branch);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/finite_family_owned_field/match_join/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/finite_family_returned_descriptor/local/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_layout_bit_observation_through_local_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_layout_reference_readback_through_call_result_exit/main.omg` | `record.pointer = pointer;` | invalid: callee writes through `&mut` parameter `pointer`
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_layout_reference_readback_through_target_body_exit/main.omg` | `record.pointer = pointer;` | invalid: callee writes through `&mut` parameter `pointer`
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_layout_storage_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/fixed_region_byte_roundtrip_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/float_result_equality_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/float_result_equality_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_byte_array_element_backing_through_parameter_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_nested_record_backing_other_machine/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_retained_mutable_borrow_const_name/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_retained_mutable_borrow_reverse_completion/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_retained_shared_borrow_match_join/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_retained_shared_borrow_resident_whole_move/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/foreign_scalar_backing_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/free_entry_local_state_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/generic_content_transfer_through_const_name_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/generic_requirement_statement_call_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/generic_requirement_statement_call_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/generic_terminated_array_through_parameter_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/guarded_line_loan_through_other_machine_exit/main.omg` | `ReadOutcome::Complete { count } -> evaluate(observe(&mut line, count))` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/guarded_record_line_loan_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/ieee_float_ordered_comparison_through_field_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/implementation_region_forwarding_through_const_name_exit/main.omg` | `let result: u64 = exercise(&mut environment, &storage);` | Q7: non-mut `let environment` with `let result: u64 = exercise(&mut environment, &storage);`
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/in_place_view_argument_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/in_place_view_argument_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/independent_content_partition_through_other_machine_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/indexed_crash_guard_through_local_exit/main.omg` | `transition items[0] == 7 { true -> finish(read_seven(&mut items, 0)) _ -> bad() }` | Q7: non-mut `let items` with `transition items[0] == 7 { true -> finish(read_seven(&mut items, 0)) _ -> bad() }`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/indexed_storage_address_identity_through_call_result_exit/main.omg` | `let selected: addr = address_of(&mut items[index]);` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/indexed_storage_address_identity_through_target_body_exit/main.omg` | `let selected: addr = address_of(&mut items[index]);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/inferred_array_equation_call_mixed_binders_through_other_state_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/installed_entry_storage_through_call_result_exit/main.omg` | `let observed: u32 = view.status.read(&mut view.status);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/installed_entry_storage_through_target_body_exit/main.omg` | `let observed: u32 = view.status.read(&mut view.status);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=both | `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_other_state_exit/main.omg` | `// Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/length_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/linear_owner_borrowed_projection_const_name/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/linear_routed_label_through_call_result_exit/main.omg` | `domain Claim::Tagged = Claim::Issued &mut  Claim::Label;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/linear_routed_label_through_target_body_exit/main.omg` | `domain Claim::Tagged = Claim::Issued &mut  Claim::Label;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/match_join_place_read_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/match_join_place_read_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/match_reference_referent_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/may_write_choice_through_const_name_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/may_write_codec_argument_through_call_result_exit/main.omg` | `Blob::encode(&mut sample, &mut buffer, choose(&mut chosen, &mut other, true));` | Q7: non-mut `let sample` with `Blob::encode(&mut sample, &mut buffer, choose(&mut chosen, &mut other, true));`
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/may_write_codec_argument_through_target_body_exit/main.omg` | `Blob::encode(&mut sample, &mut buffer, choose(chosen, other, true));` | Q7: non-mut `let sample` with `Blob::encode(&mut sample, &mut buffer, choose(chosen, other, true));`
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/may_write_codec_through_parameter_exit/main.omg` | `encode_packet(&mut sample, &mut buffer, counter.slot);` | Q7: non-mut `let sample` with `encode_packet(&mut sample, &mut buffer, counter.slot);`
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/may_write_named_state_through_other_machine_exit/main.omg` | `let found: u8 = read_at(&mut items, other);` | Q7: non-mut `let items` with `let found: u8 = read_at(&mut items, other);`
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/may_write_nested_helper_through_match_join_exit/main.omg` | `machine mark_index(other: u64, path: &mut write u64) -> u64` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/method_array_operand_inference_through_field_exit/main.omg` | `let observed: u8 = Bytes::last<3u64>(&mut packet.items);` | Q7: non-mut `let packet` with `let observed: u8 = Bytes::last<3u64>(&mut packet.items);`
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/named_const_callable_family_through_match_join_exit/main.omg` | `let first: u64 = apply<FOUR, measure>(&mut four);` | Q7: non-mut `let four` with `let first: u64 = apply<FOUR, measure>(&mut four);`
- **discarded-valid** accepted_by=both | `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_local_exit/main.omg` | `// Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut  31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_borrow_qualification_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_borrow_qualification_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_record_field_borrow_bound_through_local_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_result_extent_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_result_extent_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nominal_record_representation_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nominal_record_representation_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/nominal_reference_cleanup_through_parameter_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/non_owning_reference_erasure_through_other_state_exit/main.omg` | `machine write(storage: &mut write Storage in Issued) {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/placed_stable_write_through_const_name_exit/main.omg` | `let observed: u64 = read(&mut other.number);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/progress_selector_nested_arrival_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/progress_selector_owned_arrival_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/progress_selector_through_local_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/qualified_field_capacity_through_field_exit/main.omg` | `tail: Tail in Tracked &mut  Partitionable;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/reference_recast_backing_footprint_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/reference_recast_backing_footprint_through_target_body_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/reflection_capacity_through_parameter_exit/main.omg` | `machine accept(bound: &mut ScalarBound) requires bound.valid && bound.bytes == 7u64 {}` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/reflection_declaration_inspection_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/remainder_width_index_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/representation_agreement_through_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/resident_backing_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/resident_backing_runtime_loop_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/facts/result_cleanup_guarantees_through_other_machine_exit/main.omg` | `// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.` | Q7: non-mut `let seed` with `// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.`
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/returned_write_destination_through_field_exit/main.omg` | `data Destination { loan: &mut write i32; }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/runtime_count_index_through_const_name_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_local_exit/main.omg` | `requires kept(&mut holder.items) == true` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_field_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/saved_observation_snapshot_through_const_name_exit/main.omg` | `let saved: u64 = capture<&mut u64>(&items[0], &mut marker, 11);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/scalar_tag_field_loan_through_match_join_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/selected_native_code_layout_through_parameter_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/selected_provider_issuance_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/shared_borrow_field_projection_through_local_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/shared_byte_view_rebinding_through_field_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/shared_qualification_erasure_through_const_name_exit/main.omg` | `domain Ticket::Labelled = Ticket::Issued &mut  Ticket::Label;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=current | `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_call_result_exit/main.omg` | `// Fact: 64-bit signed bitwise AND mask raw &mut  63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_target_body_exit/main.omg` | `// Fact: 64-bit signed bitwise AND mask raw &mut  63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/slice_view_element_read_bound_through_other_machine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/stored_shared_byte_view_element_lend_through_const_name_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/suspension_transported_loan_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/suspension_transported_loan_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/target_family_contract_operand/other_state/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/target_family_invocation_lifetime/other_machine/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/temporary_case_cleanup_through_call_result_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/temporary_case_cleanup_through_target_body_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/typed_reflection_mutation_through_other_state_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/typed_reflection_through_other_machine_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=current | `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_parameter_exit/main.omg` | `// Fact: 64-bit unsigned bitwise AND mask raw &mut  63u64 with bounded interval 40u64 <= masked <= 63u64 establishes 40u64 <= masked <= 63u64 across all nine places so exact subtractions 63u64 - masked and masked - 40u64 are safe.` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/value_location_alignment_contract_exit/main.omg` | `// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/value_location_alignment_through_other_machine_exit/main.omg` | `// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=both | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_field_exit/main.omg` | `// Masking an unsigned scalar with a constant bitmask (val &mut  63u64) establishes the upper bound (masked <= 63u64), discharging exact subtraction 63u64 - masked.` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/write_only_borrow_initialization_through_const_name_exit/main.omg` | `// Fact: a write-only borrow (&mut write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/write_only_borrow_struct_destination_through_call_result_exit/main.omg` | `// Fact: Initializing a destination storage slot through a write-only borrow (&mut write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/write_only_borrow_struct_destination_through_target_body_exit/main.omg` | `// Fact: Initializing a destination storage slot through a write-only borrow (&mut write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.` | valid mutant; route `outside` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/write_only_ieee_store_readback_through_parameter_exit/main.omg` | `// Fact: an IEEE f64 computed and stored through a write-only borrow (&mut write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/write_only_indexed_reborrow_through_other_state_exit/main.omg` | `machine replace(cell: &mut write Cell, value: i64) { cell.value = value; }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/facts/write_only_match_join_through_other_machine_exit/main.omg` | `let selected: &mut write i64 = match choose(visits, flag) {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/facts/write_only_recast_through_match_join_exit/main.omg` | `machine marked<'storage>(storage: &'storage write Original, marker: &mut write u32) -> &'storage write Viewed {` | Q7: source `let write u32) -> &'storage write Viewed {` without `mut`; `machine marked<'storage>(storage: &'storage write Original, marker: &mut write u32) -> &'storage write Viewed {`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/facts/write_only_reference_store_through_local_exit/main.omg` | `// Fact: a `&mut write` store into a record's reference leaf installs the incoming` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/filesystem/native_fs_workflow/main.omg` | `&mut  61440;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/filesystem/windows_raw_file_control_exit/main.omg` | `transition (after &mut  1) != 0 && (before & 4294967166) == (after & 4294967166) {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/float/constructor_requirement_result_policy_exit/main.omg` | `maximum: f32 in Finite &mut  Saturating;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/float/runtime_exact_integer_to_float_coercion_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/float/runtime_guarded_float_capture_exit/main.omg` | `transition select(&mut values, 0) == 0.5 && select(&values, 1) == 0.0 {` | Q7: non-mut `let values` with `transition select(&mut values, 0) == 0.5 && select(&values, 1) == 0.0 {`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/float/specification_index_reference_selector_exit/main.omg` | `// although `position` is `&mut u64` or `&mut u64`, exactly as for a `u64`.` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/float/value_refinement_conjunction_survives_scalar_transport_exit/main.omg` | `machine retain(value: f64 in Finite &mut  Reflexive) -> f64 in Finite & Reflexive { value }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/generics/contextual_static_machine_const_scopes_exit/main.omg` | `let first_result: bool = first<2>(&mut pair);` | Q7: non-mut `let pair` with `let first_result: bool = first<2>(&mut pair);`
- **rejected-both** accepted_by=- | `tests/omega/pass/generics/core_slice_for_each/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/generics/generic_case_subject_sibling_bounds_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/generics/runtime_case_qualified_reference_storage_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/generics/runtime_container_setter_matrix_exit/main.omg` | `machine Cell::get<T [copy]>(cell: &mut Cell<T>) -> T {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/generics/runtime_symbolic_receiver_extent_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/host/runtime_console_bounded_line_lf_exit/main.omg` | `Console::write(&mut main.line);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/host/runtime_descriptor_binding_record_result_exit/main.omg` | `let written: i64 = DescriptorIo::write(1, &mut words, 4);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/host/runtime_descriptor_immutable_primitive_loans_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/host/runtime_descriptor_record_array_input_exit/main.omg` | `let written: i64 = DescriptorIo::write_bytes(1, &mut packet.bytes);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/host/runtime_gui_foreground_window_exit/main.omg` | `main.hwnd = WindowsGuiHost::window_create(&mut main.cls, &main.title, 0, 10, 10, 50, 50);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/host/runtime_windows_bitmap_client_cleanup_exit/main.omg` | `WindowsGuiHost::stretch_dibits(device, 0, 0, 173, 147, 0, 0, 2, 2, &mut pixels[0], &info[0], 0, 13369376)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/fixed_layout_computed_alias_policy_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/fixed_layout_overridden_valid_policy_exit/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/layouts/nested_reference_carrier_geometry_exit/main.omg` | `data Views { pair: &mut Pair; bytes: &[u8]; }` | Q7: source `let Pair; bytes: &` without `mut`; `data Views { pair: &mut Pair; bytes: &[u8]; }`
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/placed_primitive_authored_name/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg` | `read(&mut view.number)` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/layouts/private_callback_descriptor_fragments_exit/main.omg` | `fill_materializations(&mut plan, &mut signature, 0, 33);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/resident_array_field_access/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/layouts/resident_qualified_schema_result/main.omg` | `machine initialize(storage: Extent in Granted &mut  Vacant, value: Payload)` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/runtime_indexed_restricted_field_read/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/layouts/runtime_placed_external_nested_array_exit/main.omg` | `let first: u64 = read(&mut view.words[0][1]);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/layouts/runtime_run_zero_sized_element_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/layouts/runtime_target_layout_queries_exit/main.omg` | `let facts: LayoutTargetFacts = layout_target_facts(&mut target);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/machines/private_frontier_enum_guard_disjunction_exit/main.omg` | `data Kind [copy] {case A; case B;} data Cell {value:u64; kind:Kind;} machine read(cell:&mut Cell)->Kind{cell.kind}` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/memory/resident_content_mutation_transport/main.omg` | `storage: Extent in Granted &mut  Resident<WordPlacement, Word>;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/memory/runtime_selected_resident_exchange_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/ownership/aliased_shared_views_compile/main.omg` | `// name `&mut main.score` — shared loans alias freely; only the exclusive` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/ownership/move_after_view_read_compile/main.omg` | `// member: `&mut main.tag` feeds `peek`, then `move main.tag` transfers the` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/ownership/registration_owned_claim_retains_provenance_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/ownership/runtime_borrowed_case_result_restore_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/ownership/runtime_captured_case_structural_call_exit/main.omg` | `machine snapshot<Role>(token: &mut Token in Issued) -> Snapshot {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/ownership/runtime_replacement_call_child_snapshot_exit/main.omg` | `observe_saved(&mut saved);` | Q7: non-mut `let saved` with `observe_saved(&mut saved);`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/ownership/shared_record_view_compile/main.omg` | `// A `&` loan may name a whole record: `peek(&mut main.row)` lends read-only` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/ownership/view_to_view_reborrow_compile/main.omg` | `// A stored `&` view feeds a second binding: `let again: &mut u64 = peek`` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/packages/retained_dependency_bodies_exit/vendor/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/parser/deep_nesting_within_limit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/proofs/declaration_predicate_calls_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/proofs/finite_boolean_policy_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/proofs/named_witness_static_trait_plural_compile/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/proofs/scalar_field_snapshot_captures_replaced_collection/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/providers/lifetime_boundary_requirement_dispatch_exit/main.omg` | `// lending `&mut main.bytes[0..41]`: the unit plan admits a borrowed boundary` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/providers/named_foreign_record_result_contract/main.omg` | `machine enter( image_handle: EfiImageHandle, system_table: &mut EfiSystemTable ) -> EfiStatus reaches ProgramImage invokes ProgramImage { firmware_entry(image_handle, system_table) }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/providers/runtime_foreign_owned_allocation_release_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/providers/runtime_provider_reference_view_exit/main.omg` | `boundary trait Version { machine locate() -> &mut RawView reaches; }` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/providers/scalar_membership_boundary_exit/main.omg` | `public domain u32::Flags requires (self &mut  2) == 0;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/recast/borrowed_byte_scalar_views_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/recast/case_payload_member_view_compile/main.omg` | `// `&mut captured as &i64` reinterprets the payload word's bits as a signed` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/recast/i16_unsigned_view_compile/main.omg` | `// A signed 16-bit value re-views as unsigned bits: `&mut main.reading as` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/recast/native_bounded_array_window_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/recast/runtime_guarded_offset_recast_exit/main.omg` | `let v: &mut u32 = &main.buf[off] as &u32;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/recast/runtime_shared_record_float_range_weakening_exit/main.omg` | `let view: &mut WideReading = &context.reading as &WideReading;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/recast/u32_signed_bits_view_compile/main.omg` | `// `&mut raw as &i32` reads u32's maximum as -1 — the sign bit flips the` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/register_allocation/runtime_call_crossing_decimal_reload_exit/main.omg` | `transition accepted { true -> block header(&mut configuration) _ -> invalid() }` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/relevance/erased_formal_borrowed/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/relevance/erased_reference_receiver_field_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/local_record_array_state_view_exit/main.omg` | `let view: &[i32] = &mut packet.samples.items[..];` | Q7: source `let packet` without `mut`; `let view: &[i32] = &mut packet.samples.items[..];`
- **discarded-valid** accepted_by=- | `tests/omega/pass/slices/nested_operand_view/main.omg` | `bits.y = ((bits.x &mut  0xffff) as u32) & (&bits.x as &u32);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/returned_collection_window_copy/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/slices/runtime_borrowed_noncopy_array_argument_exit/main.omg` | `let field_count: u64 = extent(&mut main.entries);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/runtime_empty_qualified_scalar_array_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/runtime_indexed_store_operand_effect_order_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/slices/runtime_mutable_byte_view_loop_write_exit/main.omg` | `out[at] = ((bits &mut  127) | 128) as u8;` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/runtime_namespace_range_contract_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/slices/runtime_range_qualified_fixed_scalar_snapshot_exit/main.omg` | `let chosen: i32 in Small = copy(&mut items, 1);` | valid mutant; route `outside` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/runtime_same_carrier_inline_cast_index_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/slices/runtime_slice_index_copy_dispatch_exit/main.omg` | `let entry_view: &[Entry] = &mut main.entries[..];` | valid: `&mut` loan into shared slot is reborrow from mut place (source parameter `main: &mut _`)
- **discarded-valid** accepted_by=- | `tests/omega/pass/slices/runtime_unit_state_subslice_and_read_call_operands_exit/main.omg` | `let view: &[Entry] = &mut main.entries[..];` | valid: `&mut` loan into shared slot is reborrow from mut place (source parameter `main: &mut _`)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/slices/slice_core_surface/main.omg` | `// `&mut main.entries[..]` borrows a field array as `&[T]` and `.len` reports the` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **valid-route-outside** accepted_by=current | `tests/omega/pass/slices/subslice_folded_bound_facts_compile/main.omg` | `let view: &[Entry] = &mut main.entries[..];` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/slices/wrapping_count_window_length/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/structs/affine_record_state_clone/main.omg` | `copied.count = source.count;` | invalid: callee writes through `&mut` parameter `source`
- **rejected-both** accepted_by=- | `tests/omega/pass/structural/local_record_receivers/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/targets/target_body_call_contracts_exit/main.omg` | `linux_x86_64 machine Helper::value(helper: &mut Helper, input: u64) -> u64` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/termination/borrowed_carrier_rank/main.omg` | `// `&mut Pair`, so the `bag` role arrives through one reference boundary` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/termination/measure_borrowed_projection_range/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/termination/progress_selector_owned_arrival_affine_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/termination/progress_selector_owned_arrival_repeated_rebind_exit/main.omg` | `None` | 
- **valid-route-outside** accepted_by=current | `tests/omega/pass/termination/rank_range_nested_slice_carrier_borrowed/main.omg` | `// is `&mut Pair`, so the `bag` role arrives through one reference boundary and` | valid mutant; route `no-entry` not a checker verdict (fix4)
- **rejected-both** accepted_by=- | `tests/omega/pass/termination/runtime_selected_requirement_rank_prefix_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/pass/text/byte_content_store_continuation_exit/main.omg` | `let prefix: &[u8] in Utf8 = TextBuffer::view(&mut main.text);` | valid: `&mut` loan into shared slot is reborrow from mut place (source parameter `main: &mut _`)
- **rejected-both** accepted_by=- | `tests/omega/pass/text/local_bounded_text_scalar_loan_exit/main.omg` | `let sum: i32 in Wrapping = digit_sum(&mut text);` | Q7: non-mut `let text` with `let sum: i32 in Wrapping = digit_sum(&mut text);`
- **discarded-valid** accepted_by=- | `tests/omega/pass/text/plain_and_qualified_byte_arrays_exit/main.omg` | `let first: u8 = head(&mut plain);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **discarded-valid** accepted_by=- | `tests/omega/pass/text/runtime_bounded_carrier_concat_exit/main.omg` | `main.text = Buffer16::join("Room ", Buffer4::view(&mut main.label));` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **valid-route-outside** accepted_by=current | `tests/omega/pass/text/runtime_qualified_owner_view_open_exit/main.omg` | `let first: bool = count(TextBuffer::view(&mut line)) == 2;` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/text/zii_default_string_equality_exit/main.omg` | `main.okf = TextBuffer::equal(&mut main.never, "");` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/time/runtime_kernel_clock_wait_loans_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/traits/applied_nominal_callback_scopes_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/traits/equatable_string_field_equality_exit/main.omg` | `machine equals(left: &mut T, right: &T) -> bool;` | Q7: source `let T, right: &T) -> bool` without `mut`; `machine equals(left: &mut T, right: &T) -> bool;`
- **valid-route-outside** accepted_by=current | `tests/omega/pass/traits/requirement_evidence_renamed_regions_exit/main.omg` | `let answer: i32 = observe(&mut storage.value, &storage.neighbor);` | valid mutant; route `outside` not a checker verdict (fix4)
- **discarded-valid** accepted_by=- | `tests/omega/pass/traits/runtime_finite_family_correlated_case_result_exit/main.omg` | `machine code<Width: u32, Depth: u32>(subject: &mut T) -> Choice` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/pass/traits/runtime_qualified_owned_requirement_calls_exit/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/pass/types/exact_address_conversion/main.omg` | `None` | 
- **known-gap-q7-mut** accepted_by=current | `tests/omega/pass/types/storage_address_aggregate/main.omg` | `let address: addr = address_of(&mut items);` | Q7: non-mut `let items` with `let address: addr = address_of(&mut items);`
- **rejected-both** accepted_by=- | `tests/omega/pass/wire/library_compact_binary_packed_decode_exit/main.omg` | `transition CompactBinary::read_packed(&mut bytes[..], &mut words[0u64..2u64], 100u64) {` | Q7: non-mut `let bytes` with `transition CompactBinary::read_packed(&mut bytes[..], &mut words[0u64..2u64], 100u64) {`
- **rejected-both** accepted_by=- | `tests/omega/pass/wire/library_compact_binary_utf8_exit/main.omg` | `transition CompactBinary::valid_utf8(&mut empty[..])` | Q7: non-mut `let empty` with `transition CompactBinary::valid_utf8(&mut empty[..])`
- **rejected-both** accepted_by=- | `tests/omega/pass/wire/runtime_wire_decode_byte_slice_exit/main.omg` | `BlobCodec::encode(&mut sample, &mut main.buffer, &mut main.written);` | Q7: non-mut `let sample` with `BlobCodec::encode(&mut sample, &mut main.buffer, &mut main.written);`
- **rejected-both** accepted_by=- | `tests/omega/pass/wire/runtime_wire_exact_array_without_count_exit/main.omg` | `TelemetryCodec::encode(&mut msg, &mut main.buffer, &mut main.written);` | Q7: non-mut `let msg` with `TelemetryCodec::encode(&mut msg, &mut main.buffer, &mut main.written);`
- **rejected-both** accepted_by=- | `tests/omega/pass/wire/wire_field_numbers/main.omg` | `BlobCodec::encode(&mut sample, &mut main.buffer, &mut main.written);` | Q7: non-mut `let sample` with `BlobCodec::encode(&mut sample, &mut main.buffer, &mut main.written);`
- **rejected-both** accepted_by=- | `tests/omega/run/borrows/call_result_reference_field_overwrite/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/run/borrows/returned_indexed_case_descriptor_field/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/run/borrows/shared_runtime_index_borrow/main.omg` | `let borrowed: &mut u8 = &main.buf[main.off];` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/run/calls/named_transition_scalar_borrow_arguments_exit/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/run/calls/table_report_call_survival/main.omg` | `report(main, &mut table, &budget);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/run/domains/issued_owned_case_parameter/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/run/domains/issued_record_integer_bounds/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/run/domains/qualified_fixed_byte_array_carrier_exit/main.omg` | `tagged.label = label;` | invalid: callee writes through `&mut` parameter `tagged`
- **rejected-both** accepted_by=- | `tests/omega/run/domains/qualified_fixed_byte_array_plain_copy_exit/main.omg` | `tagged.label = label;` | invalid: callee writes through `&mut` parameter `tagged`
- **discarded-valid** accepted_by=- | `tests/omega/run/dyn_finite_family_correlated/main.omg` | `machine code<Width: u32, Depth: u32>(subject: &mut T) -> u32 requires (Width == 16 && Depth == 4) || (Width == 32 && Depth == 8);` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/run/generics/runtime_value_index_operands/main.omg` | `None` | 
- **discarded-valid** accepted_by=- | `tests/omega/run/layouts/placed_ram_stored_field_return/main.omg` | `machine observe(view: &mut Access<RegPlacement, Registers>) -> u32 {` | valid: `&mut` at least as strict as `&` without shared-slot violation
- **rejected-both** accepted_by=- | `tests/omega/run/ownership/content_transform_empty_array/main.omg` | `None` | 
- **rejected-both** accepted_by=- | `tests/omega/run/ranges/refuted_boolean_alias_local/main.omg` | `None` | 


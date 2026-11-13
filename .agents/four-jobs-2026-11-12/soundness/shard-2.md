# Soundness mutations audit (shard 2 of 8)

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

## Bucket table

| Bucket | Count |
| --- | ---: |
| confirmed_hole | 277 |
| known_gap_q7_mut | 0 |
| disagreement_valid | 3 |
| valid_route_outside | 529 |
| discarded_valid | 1 |
| rejected_both | 1218 |
| timeout | 0 |
| failed_run | 0 |
| **Total mutants** | **2028** |

## Fixture / mutant reconciliation (fix4)

| | Count |
| --- | ---: |
| Fixtures in shard | 1130 |
| Fixtures processed | 1130 |
| Original check failed (skipped) | 51 |
| No-mutation fixtures (all mutators N/A) | 155 |
| Mutated fixtures (≥1 mutant row) | 924 |
| Mutant rows (TSV) | 2028 |

Fixtures with more than one mutant row:
- `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/chained_const_binder_scale_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/compound_assign/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_anonymous_array_stores_with_float_import_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_cyclic_parameter_fixed_use_after_console_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_explicit_call_operand_policy_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_guarded_read_count_assignment_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_payload_range_narrowing_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_shift_count_domain_exit/main.omg` (2 rows)
- `tests/omega/pass/arithmetic/runtime_wrapping_mutable_local_exit/main.omg` (2 rows)
- `tests/omega/pass/arrays/local_record_array_guard_exit/main.omg` (2 rows)
- `tests/omega/pass/asm/atomic_release_boolean_disposition/main.omg` (3 rows)
- `tests/omega/pass/asm/state_fact_transfers_exit/main.omg` (3 rows)
- `tests/omega/pass/atomics/runtime_atomic_branch_frontier_exit/main.omg` (2 rows)
- `tests/omega/pass/atomics/runtime_atomic_loop_frontier_exit/main.omg` (4 rows)
- `tests/omega/pass/borrow/mutable_selector_literal_replacement_exit/main.omg` (3 rows)
- `tests/omega/pass/borrow/reference_parameter_referent_copy_exit/main.omg` (3 rows)
- `tests/omega/pass/borrow/runtime_signed_element_call_exit/main.omg` (4 rows)
- `tests/omega/pass/borrow/runtime_write_only_array_selector_snapshot_exit/main.omg` (3 rows)
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_forward_exit/main.omg` (2 rows)
- `tests/omega/pass/borrow/runtime_write_only_record_array_argument_exit/main.omg` (3 rows)
- `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_projected_exit/main.omg` (3 rows)
- `tests/omega/pass/borrow/write_only_composed_forward_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/borrow_stated_index_disequality_mut/main.omg` (2 rows)
- `tests/omega/pass/borrows/borrowed_reference_field_read_and_write_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/captured_state_shared_reborrow/main.omg` (3 rows)
- `tests/omega/pass/borrows/completed_reference_read_preserves_prefix/main.omg` (2 rows)
- `tests/omega/pass/borrows/copied_dispatch_payload_before_mutating_argument_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/lifetime_return_borrow/main.omg` (2 rows)
- `tests/omega/pass/borrows/local_record_nested_sequence_extent_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/mutable_reference_match_local/main.omg` (2 rows)
- `tests/omega/pass/borrows/nested_machine_unit_output_long_cycle_exit/main.omg` (4 rows)
- `tests/omega/pass/borrows/opaque_incoming_stored_byte_view_exit/main.omg` (4 rows)
- `tests/omega/pass/borrows/owned_nested_record_copy_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/projected_copy_parent_custody_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/qualified_byte_owner_exclusive_call_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg` (4 rows)
- `tests/omega/pass/borrows/returned_primitive_reference_assignment_capture_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/returned_reference_source_guarantee_exit/main.omg` (5 rows)
- `tests/omega/pass/borrows/runtime_array_primitive_reference_capture_exit/main.omg` (2 rows)
- `tests/omega/pass/borrows/runtime_captured_primitive_reference_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/runtime_case_reference_mixed_state_returns_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/runtime_descriptor_replacement_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/runtime_exclusive_byte_field_cleanup_exit/main.omg` (3 rows)
- `tests/omega/pass/borrows/runtime_forwarded_view_field_domain_exit/main.omg` (4 rows)
- `tests/omega/pass/borrows/runtime_indexed_carrier_conditional_exit/main.omg` (5 rows)
- `tests/omega/pass/borrows/runtime_indexed_stored_reference_call/main.omg` (5 rows)
- `tests/omega/pass/borrows/runtime_multidimensional_reference_result_exit/main.omg` (4 rows)
- `tests/omega/pass/borrows/runtime_nested_byte_field_capture_exit/main.omg` (5 rows)
- `tests/omega/pass/borrows/runtime_owned_carrier_descriptor_replacement_exit/main.omg` (3 rows)
- … and 588 more

## Confirmed holes by kind

- `drop_machine_requires`: 103
- `mut_ref_to_ref`: 95
- `ref_to_mut_ref`: 58
- `widen_requires_lt`: 21

## Confirmed holes by accepted_by

- `both`: 5
- `current`: 272

## Confirmed holes by hole_subtype

- `(empty)`: 174
- `probe-not-built`: 65
- `inferred-entry`: 33
- `unchecked`: 5

## Confirmed holes (fix4 airtight)

### 1. `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -19,5 +19,5 @@
 
 data Main {}
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let a: u8 = anonymous_sum();
     let b: u8 = unsigned_intermediate();
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_arithmetic_anonymous_integer_arith_mut_ref_to_ref_c9ffa0793b7a/main.omg`
- hole_subtype: ``; probe: 

### 2. `tests/omega/pass/arithmetic/compound_assign/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let mut v: i32 = 10;
     v += 5;
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_arithmetic_compound_assign_mut_ref_to_ref_8a6250c50b7e/main.omg`
- hole_subtype: ``; probe: 

### 3. `tests/omega/pass/arithmetic/runtime_wrapping_mutable_local_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 data Main { }
 
-machine Main::main(context: &mut Main) reaches Console {
+machine Main::main(context: &Main) reaches Console {
     let mut value: i32 in Wrapping;
     value = value + 2147483647;
```

- Obligation line: `machine Main::main(context: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_arithmetic_runtime_wrapping_mutable_local_exit_mut_ref_to_ref_39cf9da20bb9/main.omg`
- hole_subtype: ``; probe: 

### 4. `tests/omega/pass/asm/state_fact_transfers_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type constrained`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
     }
     machine digit(rest: u64 in Trapping, character: u64 in Trapping) -> u64
-    requires 48 <= character && character <= 57
     {
         let mut byte: u8 = character as u8;
```

- Obligation line: `machine digit(rest: u64 in Trapping, character: u64 in Trapping) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_asm_state_fact_transfers_exit_drop_machine_requires_bcebe22a4b5d/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `digit(character, 0)` omega_exit=1

### 5. `tests/omega/pass/asm/state_fact_transfers_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type constrained`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -36,5 +36,5 @@
     transition { _ -> small(value) }
     machine small(value: u64) -> u64
-    requires value < 10
+    requires value <= 10
     { value }
 }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_asm_state_fact_transfers_exit_widen_requires_lt_94c565b9f4c2/main.omg`
- hole_subtype: ``; probe: 

### 6. `tests/omega/pass/atomics/runtime_atomic_loop_frontier_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
     wake.after = 91;
     wake.pending.store(1, Publish);
-    run(&wake);
+    run(&mut wake);
 }
 
```

- Obligation line: `run(&mut wake);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_atomics_runtime_atomic_loop_frontier_exit_ref_to_mut_ref_0a1de746d589/main.omg`
- hole_subtype: ``; probe: 

### 7. `tests/omega/pass/borrow/runtime_signed_element_call_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,5 @@
     }
     machine update(values: &mut [f64], amount: f64, index: i32 in Wrapping)
-    requires index >= 0 && index < values.len {
+    requires index >= 0 && index <= values.len {
         values[index] = values[index] - amount;
         transition { _ -> scan(values, amount, index + 1) }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrow_runtime_signed_element_call_exit_widen_requires_lt_f80ababf0e21/main.omg`
- hole_subtype: ``; probe: 

### 8. `tests/omega/pass/borrows/explicit_lifetime_select/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // An erased lifetime argument on borrow-carrying data selects the one
-// input retained by the result: `View<'buf>` wraps `&'buf mut i32`;
+// input retained by the result: `View<'buf>` wraps `&mut 'buf mut i32`;
 // `select<'left, 'right>` returns `View<'left>` — the second borrow
 // stays free to store, the first is frozen into the view.
```

- Obligation line: `// input retained by the result: `View<'buf>` wraps `&mut 'buf mut i32`;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_explicit_lifetime_select_ref_to_mut_ref_2e2a0ec837fc/main.omg`
- hole_subtype: ``; probe: 

### 9. `tests/omega/pass/borrows/lifetime_return_borrow/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // A borrowed-parameter lifetime forwarded to the result:
-// `combine<'destination>(... destination: &'destination mut Cell)
+// `combine<'destination>(... destination: &mut 'destination mut Cell)
 // -> &'destination mut Cell` returns the passed loan so the caller
 // keeps writing through it; eight shared `&Cell` operands fold into
```

- Obligation line: `// `combine<'destination>(... destination: &mut 'destination mut Cell)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_lifetime_return_borrow_ref_to_mut_ref_14433851d6fc/main.omg`
- hole_subtype: ``; probe: 

### 10. `tests/omega/pass/borrows/nested_machine_unit_output_long_cycle_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
         }
         machine step(slot: &mut View<'storage>, incoming: View<'storage>, pending: u64)
-        requires pending > 0
         {
             incoming.cell.value = ((incoming.cell.value as u64 in Wrapping) + 1) as u64;
```

- Obligation line: `transition { _ -> relay(slot, incoming, pending - 1) }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_nested_machine_unit_output_long_cycle_exit_drop_machine_requires_fac4f2535192/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `step(0, 0, 0)` omega_exit=1

### 11. `tests/omega/pass/borrows/opaque_incoming_stored_byte_view_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 data Bundle<'storage> { windows: [&'storage mut [u8]; 1]; }
 machine observe(bundle: &Bundle) -> bool
-requires bundle.windows[0].len == 1
 {
     bundle.windows[0].len == 1 && bundle.windows[0][0] == 19
```

- Obligation line: `data Bundle<'storage> { windows: [&'storage mut [u8]; 1]; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_opaque_incoming_stored_byte_view_exit_drop_machine_requires_2d40d79989bc/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 12. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -15,5 +15,4 @@
     }
     machine check(chain: &'c Chain, cursor: &'c Node, node: &'c Node, position: u64) -> &'c [u8]
-        requires position < 4
     {
         transition node.used <= 8 {
```

- Obligation line: `machine find<'c>(chain: &'c Chain, cursor: &'c Node, position: u64) -> &'c [u8] {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_reference_cycle_projection_root_partition_drop_machine_requires_115626dd2afa/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `check(4, 0, 0, 0)` omega_exit=1

### 13. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -15,5 +15,5 @@
     }
     machine check(chain: &'c Chain, cursor: &'c Node, node: &'c Node, position: u64) -> &'c [u8]
-        requires position < 4
+        requires position <= 4
     {
         transition node.used <= 8 {
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_reference_cycle_projection_root_partition_widen_requires_lt_fcb11c0d080d/main.omg`
- hole_subtype: ``; probe: 

### 14. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // A reference-returning machine cycles through two states. Along the cycle the
-// second state receives one projection of the chain (`&chain.nodes[position]`)
+// second state receives one projection of the chain (`&mut chain.nodes[position]`)
 // after two exact parameters, so the finiteness solver holds five storage roots in
 // three components and must address its partition by root, not by component.
```

- Obligation line: `// second state receives one projection of the chain (`&mut chain.nodes[position]`)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_reference_cycle_projection_root_partition_ref_to_mut_ref_f95bffbe2575/main.omg`
- hole_subtype: ``; probe: 

### 15. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -24,5 +24,5 @@
 }
 
-machine Main::main(this: &mut Main) {
+machine Main::main(this: &Main) {
     let chain: Chain = Chain { };
     let found: &[u8] = find(&chain, &chain.sentinel, 0);
```

- Obligation line: `transition node.used <= 8 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_reference_cycle_projection_root_partition_mut_ref_to_ref_ac2653dd0157/main.omg`
- hole_subtype: ``; probe: 

### 16. `tests/omega/pass/borrows/runtime_captured_primitive_reference_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 
 data Main
-requires position < 2;
+requires position <= 2;
 {
     position: u64;
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_captured_primitive_reference_exit_widen_requires_lt_0d0e1824bc3e/main.omg`
- hole_subtype: ``; probe: 

### 17. `tests/omega/pass/borrows/runtime_forwarded_view_field_domain_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 domain [u8]::Utf8 requires valid_utf8(self);
-// A `&mut [Room]` view of records whose `label` field is declared `in Utf8`,
+// A `&[Room]` view of records whose `label` field is declared `in Utf8`,
 // forwarded whole to a machine. The callee's parameter owes each element's
 // label membership; a loan lends the storage itself, so the call site's
```

- Obligation line: `rooms[index].value = 70;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_forwarded_view_field_domain_exit_mut_ref_to_ref_e8d7e0e1069d/main.omg`
- hole_subtype: ``; probe: 

### 18. `tests/omega/pass/borrows/runtime_record_byte_region_loan_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 // buffer holding firmware-style descriptors is walked at a RUNTIME stride
 // (`offset + descriptor_size`, never sizeof), each entry re-viewed
-// through the §5b record recast (`&buffer[offset] as &Desc`) with member
+// through the §5b record recast (`&mut buffer[offset] as &Desc`) with member
 // reads through the view. Every proof is compiler-side:
 //   - the walk bound: the back-edge guard `next <= 32` admits the
```

- Obligation line: `// through the §5b record recast (`&mut buffer[offset] as &Desc`) with member`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_record_byte_region_loan_exit_ref_to_mut_ref_c2901ef34399/main.omg`
- hole_subtype: ``; probe: 

### 19. `tests/omega/pass/borrows/runtime_record_byte_region_loan_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,5 @@
 data Desc { kind: u32; pages: u64; }
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let mut buffer: [u8; 48];
     // Descriptor 0: kind 3, pages 10.
```

- Obligation line: `buffer[0] = 3;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_record_byte_region_loan_exit_mut_ref_to_ref_a70ecb7bb2e9/main.omg`
- hole_subtype: ``; probe: 

### 20. `tests/omega/pass/borrows/runtime_reference_projection_evaluation_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -65,5 +65,4 @@
 machine read_cell(cell: &Cell) -> i32 { cell.value }
 machine indexed_holder<'storage>(first: &'storage Cell, second: &'storage Cell, position: u64) -> i32
-    requires position < 2
 {
     read_cell([hold_cell(first), hold_cell(second)][position].cell)
```

- Obligation line: `machine indexed(bytes: &[u8], position: u64) -> u8 requires position < bytes.len { read(&bytes[position]) }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_reference_projection_evaluation_exit_drop_machine_requires_69c2f173f801/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 21. `tests/omega/pass/borrows/runtime_reference_projection_evaluation_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -65,5 +65,5 @@
 machine read_cell(cell: &Cell) -> i32 { cell.value }
 machine indexed_holder<'storage>(first: &'storage Cell, second: &'storage Cell, position: u64) -> i32
-    requires position < 2
+    requires position <= 2
 {
     read_cell([hold_cell(first), hold_cell(second)][position].cell)
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_reference_projection_evaluation_exit_widen_requires_lt_71c47df5ef76/main.omg`
- hole_subtype: ``; probe: 

### 22. `tests/omega/pass/borrows/runtime_shared_qualified_view_scalar_choice_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A machine taking a shared qualified view (`&[u8] in Path`) chooses its
+// A machine taking a shared qualified view (`&mut [u8] in Path`) chooses its
 // scalar result through a multi-arm guard whose arms call. A shared loan
 // cannot change its referent, so the qualification it arrived with holds at
```

- Obligation line: `// A machine taking a shared qualified view (`&mut [u8] in Path`) chooses its`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_shared_qualified_view_scalar_choice_exit_ref_to_mut_ref_36654d2b2920/main.omg`
- hole_subtype: ``; probe: 

### 23. `tests/omega/pass/borrows/runtime_whole_referent_scalar_store_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Whole-referent replacement of a scalar through `&mut`, from a state that
+// Whole-referent replacement of a scalar through `&mut mut`, from a state that
 // received the loan as a block parameter. exits: 70 = the store reached
 // the caller; 71 = it did not.
```

- Obligation line: `// Whole-referent replacement of a scalar through `&mut mut`, from a state that`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_runtime_whole_referent_scalar_store_exit_ref_to_mut_ref_29cf50d5d32e/main.omg`
- hole_subtype: ``; probe: 

### 24. `tests/omega/pass/borrows/transition_shared_borrow_result/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let holder: Holder = Holder { node: Node { value: 7, tag: 3 } };
     let seen: &Node = Holder::first(&holder);
```

- Obligation line: `let holder: Holder = Holder { node: Node { value: 7, tag: 3 } };`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_borrows_transition_shared_borrow_result_mut_ref_to_ref_16d60647918a/main.omg`
- hole_subtype: ``; probe: 

### 25. `tests/omega/pass/build/build_domain_context/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,2 +1,2 @@
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_build_build_domain_context_mut_ref_to_ref_51bca6669f79/main.omg`
- hole_subtype: ``; probe: 

### 26. `tests/omega/pass/build/terminal_calling_policy_execution/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -68,5 +68,5 @@
 // storage in the policy inspection artifact.
 public data PolicyReferent { payload: u64; }
-public data PolicyReference { value: &PolicyReferent; }
+public data PolicyReference { value: &mut PolicyReferent; }
 public boundary trait ReferenceEntry: Calling<RetainedCalling> {
     machine value(left: PolicyReference, right: u64) -> u64 reaches;
```

- Obligation line: `public data PolicyReference { value: &mut PolicyReferent; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_build_terminal_calling_policy_execution_ref_to_mut_ref_2b88a5baa69a/main.omg`
- hole_subtype: ``; probe: 

### 27. `tests/omega/pass/calls/builtin_named_machine_call_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -29,5 +29,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let limited: u32 = clamp(3);
     let widened: u64 = (max(2) as u64) + 1;
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_builtin_named_machine_call_exit_mut_ref_to_ref_b86d7e37379e/main.omg`
- hole_subtype: ``; probe: 

### 28. `tests/omega/pass/calls/callback_owned_float_record_packing/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -175,5 +175,5 @@
 public boundary trait Prepare: Calling<PrepareCalling> {
     machine prepare(
-        interface: &mut ForeignInterface, abi: u32, count: u32,
+        interface: &ForeignInterface, abi: u32, count: u32,
         result_type: &PairType, argument_types: &ArgumentTypes
     ) -> u32 reaches;
```

- Obligation line: `plan.call.parameter_count = 5;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_callback_owned_float_record_packing_mut_ref_to_ref_41252809a57d/main.omg`
- hole_subtype: ``; probe: 

### 29. `tests/omega/pass/calls/cyclic_mutable_borrow_block_parameter_call/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A `&mut` machine argument rebinds through an UNRANKED control cycle's
+// A `&mut mut` machine argument rebinds through an UNRANKED control cycle's
 // block-parameter channel, then is re-borrowed whole into a unit machine
 // call. The terminal CallUnit argument spells the place with an empty path
```

- Obligation line: `// A `&mut mut` machine argument rebinds through an UNRANKED control cycle's`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_cyclic_mutable_borrow_block_parameter_call_ref_to_mut_ref_1ce3093bd6b4/main.omg`
- hole_subtype: ``; probe: 

### 30. `tests/omega/pass/calls/cyclic_mutable_borrow_block_parameter_call/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,5 @@
 // rebinding channel as well, so this shape checks.
 //
-// Reduced from squalr's `Mini` provider loop — `module: &mut NormalizedModule`
+// Reduced from squalr's `Mini` provider loop — `module: &NormalizedModule`
 // carried through the cycle and forwarded to `module.set_module_name(...)`,
 // where the arg place is the block parameter of the machine's `&mut` input.
```

- Obligation line: `sink.value = sink.value + 1 as u64 in Wrapping;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_cyclic_mutable_borrow_block_parameter_call_mut_ref_to_ref_60b74a9275d6/main.omg`
- hole_subtype: ``; probe: 

### 31. `tests/omega/pass/calls/local_projected_field_argument_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A static machine lends one field of its local record, `&mut screen.banner`,
+// A static machine lends one field of its local record, `&screen.banner`,
 // to a machine that takes the field's own type. The loan presents exactly that
 // subtree and moves nothing out of the local: the write lands in
```

- Obligation line: `banner.width = banner.width + by;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_local_projected_field_argument_exit_mut_ref_to_ref_b8fedb1b419f/main.omg`
- hole_subtype: ``; probe: 

### 32. `tests/omega/pass/calls/local_record_shared_argument_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A static machine lends its local record `&mut` to a writer and then `&` to
+// A static machine lends its local record `&mut mut` to a writer and then `&` to
 // a scalar-returning reader. The record's wrapping field domain rides both
 // loans; the shared loan observes the value the exclusive one wrote, and the
```

- Obligation line: `// A static machine lends its local record `&mut mut` to a writer and then `&` to`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_local_record_shared_argument_exit_ref_to_mut_ref_0f925ff6f151/main.omg`
- hole_subtype: ``; probe: 

### 33. `tests/omega/pass/calls/nested_call_runtime_index_arg_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // Runtime-index borrowed members as nested call arguments:
-// `Item::clone(&list.items[i])` as a transition value target. The selector
+// `Item::clone(&mut list.items[i])` as a transition value target. The selector
 // is walked the way the checker numbers it (collection direct, index
 // separate), and the borrowed element reaches the callee's structural
```

- Obligation line: `// `Item::clone(&mut list.items[i])` as a transition value target. The selector`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_nested_call_runtime_index_arg_exit_ref_to_mut_ref_7eff3c70d091/main.omg`
- hole_subtype: ``; probe: 

### 34. `tests/omega/pass/calls/requires_carrier_ceiling_vacuous/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 
 machine echo(scale: u64) -> u64
-requires scale <= 18446744073709551615
 {
     transition { _ -> (scale) }
```

- Obligation line: `machine echo(scale: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_requires_carrier_ceiling_vacuous_drop_machine_requires_1086a0a30c87/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `echo(18446744073709551616)` omega_exit=1

### 35. `tests/omega/pass/calls/requires_carrier_ceiling_vacuous/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,5 @@
 }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let s: u64 = opaque();
     let r: u64 = echo(s);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_requires_carrier_ceiling_vacuous_mut_ref_to_ref_a9e9d0841c7d/main.omg`
- hole_subtype: ``; probe: 

### 36. `tests/omega/pass/calls/runtime_dispatch_machine_array_slice_arg_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // RUN CANARY (exit 70) -- a MACHINE ARRAY passed as a SLICE argument to a
 // dispatched value call passing a machine array as a slice argument beside a
-// DISJOINT mutable field projection (`pick(&mut main.picked, main.nums, ..)`
+// DISJOINT mutable field projection (`pick(&main.picked, main.nums, ..)`
 // with `nums: [i32; 4]`). Lending the whole subject beside a shared view of its
 // own field is refused, correctly, so the two projections compose per argument,
```

- Obligation line: `picked = s[j];`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_dispatch_machine_array_slice_arg_exit_mut_ref_to_ref_ff2177d2ade4/main.omg`
- hole_subtype: ``; probe: 

### 37. `tests/omega/pass/calls/runtime_guarded_boundary_tail_call_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -16,5 +16,5 @@
 }
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let selected: i32 = forward(70);
     let fallback: i32 = forward(0);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_guarded_boundary_tail_call_exit_mut_ref_to_ref_f2edb97842ce/main.omg`
- hole_subtype: ``; probe: 

### 38. `tests/omega/pass/calls/runtime_mutable_call_before_transition_args_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -27,5 +27,4 @@
 
     machine present(loader: &mut Loader, index: u64)
-    requires index <= 16383
     {
         // `tags`/`firsts` are `[u8; N] in Trapping`, so an element reads as
```

- Obligation line: `loader.tag = loader.tags[index] as u8;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_mutable_call_before_transition_args_exit_drop_machine_requires_3b540973cda0/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `present(16384, 0)` omega_exit=1

### 39. `tests/omega/pass/calls/runtime_shared_ref_param_member_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// SHARED `&Struct` param in a NON-boundary machine -- the first RUN coverage
+// SHARED `&mut Struct` param in a NON-boundary machine -- the first RUN coverage
 // of this shape in the corpus. `read(&main.inner)` passes a shared reference;
 // the callee reads both fields as assignment RHS/operand and guards on one.
```

- Obligation line: `// SHARED `&mut Struct` param in a NON-boundary machine -- the first RUN coverage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_shared_ref_param_member_exit_ref_to_mut_ref_56c3c63942f9/main.omg`
- hole_subtype: ``; probe: 

### 40. `tests/omega/pass/calls/runtime_stored_constructor_snapshots_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,4 @@
 
 machine observe_float(saved: SavedFloat, value: f64, limit: f64)
-requires (saved.value < limit) == (value < limit)
 crashes Trap {
     transition (saved.value < limit) == (value < limit) {
```

- Obligation line: `data SavedBoolean [copy] { value: bool; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_stored_constructor_snapshots_exit_drop_machine_requires_c063e41b8f6a/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `observe_float(limit, 0, 0)` omega_exit=1

### 41. `tests/omega/pass/calls/runtime_trailing_state_mut_param_phase_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A result-bearing machine receives a borrowed `&mut Rng`, forwards it through
+// A result-bearing machine receives a borrowed `&Rng`, forwards it through
 // two mutating calls, guards on the count, then enters a trailing state that
 // mutates and guards the same receiver again. Each guard must observe the
```

- Obligation line: `r.seed = r.seed + 7;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_runtime_trailing_state_mut_param_phase_exit_mut_ref_to_ref_d5dd7c633ce1/main.omg`
- hole_subtype: ``; probe: 

### 42. `tests/omega/pass/calls/serviceful_borrowed_scalar_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 data Main { }
 
-machine Main::measure(main: &Main, bytes: &[u8]) -> u64 reaches Console {
+machine Main::measure(main: &mut Main, bytes: &[u8]) -> u64 reaches Console {
     Console::write_line(bytes);
     bytes.len
```

- Obligation line: `machine Main::measure(main: &mut Main, bytes: &[u8]) -> u64 reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_calls_serviceful_borrowed_scalar_exit_ref_to_mut_ref_2e8a91c0c907/main.omg`
- hole_subtype: ``; probe: 

### 43. `tests/omega/pass/cases/later_arm_requires/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,5 @@
 }
 machine step(count: u64) -> u64
-requires count < 10 && count >= 1
+requires count <= 10 && count >= 1
 {
     transition { _ -> (count * 7) }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_cases_later_arm_requires_widen_requires_lt_ffa8f7c8f5d9/main.omg`
- hole_subtype: ``; probe: 

### 44. `tests/omega/pass/collections/borrowed_byte_array_state_writes_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
 
     machine step(text: &[u8; 5], output: &mut [u8; 5], position: u64)
-        requires position <= 5
     {
         transition position < 5 {
```

- Obligation line: `machine step(text: &[u8; 5], output: &mut [u8; 5], position: u64)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_borrowed_byte_array_state_writes_exit_drop_machine_requires_b122223a485f/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `step(6, 0, 0)` omega_exit=1

### 45. `tests/omega/pass/collections/borrowed_byte_array_state_writes_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -23,5 +23,5 @@
 
     machine copy(text: &[u8; 5], output: &mut [u8; 5], position: u64)
-        requires position < 5
+        requires position <= 5
     {
         let byte: u8 in Wrapping = (text[position] as u8 in Wrapping) + 1;
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_borrowed_byte_array_state_writes_exit_widen_requires_lt_99f7bd605b32/main.omg`
- hole_subtype: ``; probe: 

### 46. `tests/omega/pass/collections/borrowed_byte_array_state_writes_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // Element writes through a borrowed whole fixed byte array state parameter.
-// Two locals lend `&[u8; 5]` and `&mut [u8; 5]` into a step/copy cycle; the
+// Two locals lend `&mut [u8; 5]` and `&mut [u8; 5]` into a step/copy cycle; the
 // copy writes at a runtime index and the last state writes a literal index.
 // The exclusive loan forwards whole through every state, including the
```

- Obligation line: `// Two locals lend `&mut [u8; 5]` and `&mut [u8; 5]` into a step/copy cycle; the`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_borrowed_byte_array_state_writes_exit_ref_to_mut_ref_8f778862ba4a/main.omg`
- hole_subtype: ``; probe: 

### 47. `tests/omega/pass/collections/borrowed_byte_array_state_writes_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // Element writes through a borrowed whole fixed byte array state parameter.
-// Two locals lend `&[u8; 5]` and `&mut [u8; 5]` into a step/copy cycle; the
+// Two locals lend `&[u8; 5]` and `&[u8; 5]` into a step/copy cycle; the
 // copy writes at a runtime index and the last state writes a literal index.
 // The exclusive loan forwards whole through every state, including the
```

- Obligation line: `let text: [u8; 5] = "HELLO";`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_borrowed_byte_array_state_writes_exit_mut_ref_to_ref_adb178064e58/main.omg`
- hole_subtype: ``; probe: 

### 48. `tests/omega/pass/collections/cyclic_shared_plain_array_parameter_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -33,5 +33,5 @@
     machine body(numbers: &[i32; 3], index: i32, best: i32) -> i32
     requires index >= 0
-    requires index < 3 {
+    requires index <= 3 {
         transition {
             _ -> step(numbers, index + 1, max_i32(best, numbers[index]))
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_cyclic_shared_plain_array_parameter_exit_widen_requires_lt_116f8a884915/main.omg`
- hole_subtype: ``; probe: 

### 49. `tests/omega/pass/collections/cyclic_shared_plain_array_parameter_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 // it terminates, so its parameters go through the unranked-cycle eligibility
 // gate. That gate admitted an exclusive
-// `&mut [i32; 3]` and refused the shared `&[i32; 3]` here, which is backwards:
+// `&[i32; 3]` and refused the shared `&[i32; 3]` here, which is backwards:
 // a shared loan freezes its referent, so every arrival observes the same
 // storage, and it cannot need more than the exclusive loan of the same
```

- Obligation line: `requires index >= 0 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_cyclic_shared_plain_array_parameter_exit_mut_ref_to_ref_ee198a583bf1/main.omg`
- hole_subtype: ``; probe: 

### 50. `tests/omega/pass/collections/element_view_runtime_loan_read/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Shared record loan through a runtime index: `&cells[bag.at]` captures a
+// Shared record loan through a runtime index: `&mut cells[bag.at]` captures a
 // `&Cell` element whose selector is a structural field of the borrowed
 // parameter. The loan mints a runtime-indexed element read — the view's live
```

- Obligation line: `// Shared record loan through a runtime index: `&mut cells[bag.at]` captures a`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_element_view_runtime_loan_read_ref_to_mut_ref_163ede0c513b/main.omg`
- hole_subtype: ``; probe: 

### 51. `tests/omega/pass/collections/mutable_slice_view_edge_release/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 
 // Mutable element-view release across a named edge: `release` establishes
-// an exclusive `&mut [i32]` view of the bag's cells, then crosses an
+// an exclusive `&[i32]` view of the bag's cells, then crosses an
 // ordinary transition with the loan live. The state's result custody must
 // book the view's loan release as its owner on the edge — an owned-storage
```

- Obligation line: `let cells: &mut [i32] = &mut bag.cells[..];`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_mutable_slice_view_edge_release_mut_ref_to_ref_4f7d370c6824/main.omg`
- hole_subtype: ``; probe: 

### 52. `tests/omega/pass/collections/runtime_borrowed_fixed_array_state_reads_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A scalar machine reads a caller's fixed array through a shared `&[i32; 6]`
+// A scalar machine reads a caller's fixed array through a shared `&mut [i32; 6]`
 // loan with runtime indices, across two states, without a loop. The loan is a
 // fixed array, not an element view: its declared extent bounds each read
```

- Obligation line: `// A scalar machine reads a caller's fixed array through a shared `&mut [i32; 6]``
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_borrowed_fixed_array_state_reads_exit_ref_to_mut_ref_45c637f666ff/main.omg`
- hole_subtype: ``; probe: 

### 53. `tests/omega/pass/collections/runtime_borrowed_fixed_array_state_reads_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -29,5 +29,5 @@
 }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let numbers: [i32; 6] = [4, 15, 8, 42, 16, 23];
     let total: i32 = pair_sum(&numbers, 3, 5);
```

- Obligation line: `let left: i32 = numbers[first];`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_borrowed_fixed_array_state_reads_exit_mut_ref_to_ref_233c040348fe/main.omg`
- hole_subtype: ``; probe: 

### 54. `tests/omega/pass/collections/runtime_dynamic_indexed_borrow_call_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,5 @@
 }
 machine visit(rows: &mut [Row; 3], cursor: &mut Cursor)
-requires cursor.row < 3, cursor.column < 2 {
+requires cursor.row <= 3, cursor.column < 2 {
     let selected: dyn Writer = Written as dyn Writer;
     Writer::store(selected, &mut rows[cursor.row].data[cursor.column], 42);
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_dynamic_indexed_borrow_call_exit_widen_requires_lt_47d787b87168/main.omg`
- hole_subtype: ``; probe: 

### 55. `tests/omega/pass/collections/runtime_fixed_byte_view_dynamic_state_shared_loan_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -25,5 +25,4 @@
     }
     machine select(rows: &mut [Row; 2], cursor: &mut Cursor, selected: u64)
-        requires selected < 2
     {
         transition { _ -> forward(&rows[selected].bytes, reset(cursor)) }
```

- Obligation line: `true -> select(&mut rows, &mut cursor, selected)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_fixed_byte_view_dynamic_state_shared_loan_edrop_machine_requires_daaaa474658c/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `select(2, 0, 0)` omega_exit=1

### 56. `tests/omega/pass/collections/runtime_fixed_byte_view_dynamic_state_shared_loan_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -25,5 +25,5 @@
     }
     machine select(rows: &mut [Row; 2], cursor: &mut Cursor, selected: u64)
-        requires selected < 2
+        requires selected <= 2
     {
         transition { _ -> forward(&rows[selected].bytes, reset(cursor)) }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_fixed_byte_view_dynamic_state_shared_loan_ewiden_requires_lt_92216f4bbaf7/main.omg`
- hole_subtype: ``; probe: 

### 57. `tests/omega/pass/collections/runtime_hoisted_index_write_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -25,5 +25,4 @@
     transition main.j <= main.rows && main.k <= main.cols { true -> put(main, main.j, main.k) _ -> bad(main) }
     machine put(main: &mut Main, y: u32, x: u32)
-    requires y <= main.rows && x <= main.cols
     {
         // The RHS must be genuinely runtime: a static-folding value lowers
```

- Obligation line: `{ cells: [u8; 12]; rows: u32; cols: u32; j: u32; k: u32; out: i32; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_hoisted_index_write_exit_drop_machine_requires_6dbba3ac1dcb/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `put(main, 0, 0)` omega_exit=1

### 58. `tests/omega/pass/collections/runtime_nested_array_view_call_write_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
     }
     machine at(slots: &mut [Slot], cursor: &mut Cursor)
-    requires cursor.index >= 0 && cursor.index < slots.len
     {
         slots[cursor.index].scores[1] = value(cursor);
```

- Obligation line: `data Grid { slots: [Slot; 2]; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_nested_array_view_call_write_exit_drop_machine_requires_ab107ca77f0a/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `at(slots, 0)` omega_exit=1

### 59. `tests/omega/pass/collections/runtime_nested_array_view_call_write_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,5 @@
     }
     machine at(slots: &mut [Slot], cursor: &mut Cursor)
-    requires cursor.index >= 0 && cursor.index < slots.len
+    requires cursor.index >= 0 && cursor.index <= slots.len
     {
         slots[cursor.index].scores[1] = value(cursor);
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_nested_array_view_call_write_exit_widen_requires_lt_11eaffa81ca8/main.omg`
- hole_subtype: ``; probe: 

### 60. `tests/omega/pass/collections/runtime_owned_descriptor_parameter_read_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data with custody`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
     }
     machine selected(window: Window, position: u64) -> i32
-    requires position < window.values.len
     {
         window.values[position]
```

- Obligation line: `data Window { padding: u64; values: &[i32]; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_owned_descriptor_parameter_read_exit_drop_machine_requires_94f0974873b5/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `selected(window, 0)` omega_exit=1

### 61. `tests/omega/pass/collections/runtime_owned_descriptor_parameter_read_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data with custody`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
     }
     machine selected(window: Window, position: u64) -> i32
-    requires position < window.values.len
+    requires position <= window.values.len
     {
         window.values[position]
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_owned_descriptor_parameter_read_exit_widen_requires_lt_94524da628db/main.omg`
- hole_subtype: ``; probe: 

### 62. `tests/omega/pass/collections/runtime_whole_array_value_copy_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
 // previously ALIASED this (shared the backing element cells) while native deep-copied; the
 // fix deep-clones a Value::Array on assignment/`let` ONLY when the target type is an owned
-// [T; N] (a slice &[T] stays a shared view), so this now verifies interp==native.
+// [T; N] (a slice &mut [T] stays a shared view), so this now verifies interp==native.
 use omega_language_std::console;
 
```

- Obligation line: `// [T; N] (a slice &mut [T] stays a shared view), so this now verifies interp==native.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_collections_runtime_whole_array_value_copy_exit_ref_to_mut_ref_09d0a0b185f5/main.omg`
- hole_subtype: ``; probe: 

### 63. `tests/omega/pass/comptime/runtime_const_array_product_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 }
 machine preserve<const Rows: u64, const Columns: u64>(value: Matrix<Rows, Columns>) -> Matrix<Rows, Columns>
-requires Rows >= 1 && Rows <= 4 && Columns >= 1 && Columns <= 4
 { value }
 machine main() crashes Trap {
```

- Obligation line: `public data Matrix<const Rows: u64, const Columns: u64> [copy]`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_comptime_runtime_const_array_product_exit_drop_machine_requires_de914c24ac72/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 64. `tests/omega/pass/concurrency/runtime_returned_carrier_suspension_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -20,5 +20,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Scheduler crashes Trap {
+machine Main::main(main: &Main) reaches Scheduler crashes Trap {
     let first: i32 = 42;
     let second: i32 = 19;
```

- Obligation line: `let first: i32 = 42;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_concurrency_runtime_returned_carrier_suspension_exit_mut_ref_to_ref_87fb03e98103/main.omg`
- hole_subtype: ``; probe: 

### 65. `tests/omega/pass/concurrency/suspend_param_last_use/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,3 +14,3 @@
 }
 data Main { }
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_concurrency_suspend_param_last_use_mut_ref_to_ref_3045113b2a38/main.omg`
- hole_subtype: ``; probe: 

### 66. `tests/omega/pass/constants/machine_const_indices/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,3 +12,3 @@
 }
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_constants_machine_const_indices_mut_ref_to_ref_c6e8dcd95341/main.omg`
- hole_subtype: ``; probe: 

### 67. `tests/omega/pass/contracts/call_write_keeps_disjoint_guard_assert_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 // All guarded operations and value assertions remain unchanged; only success/failure reporting differs.
 // A call's write frame retires only the guard facts it may overlap. `pop`
-// writes its `&mut Stack` argument, so popping `main.a` keeps the entry
+// writes its `&Stack` argument, so popping `main.a` keeps the entry
 // guard's bound on `main.b.count` for the next pop in the same state. The same
 // holds when the two stacks arrive as separate state parameters.
```

- Obligation line: `requires stack.count >= 1 && stack.count <= 4 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_contracts_call_write_keeps_disjoint_guard_assert_exit_mut_ref_to_ref_971f49bc1bff/main.omg`
- hole_subtype: ``; probe: 

### 68. `tests/omega/pass/contracts/disjunctive_ensures/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 // callee inside a state-guard expression.
 machine bounded(ignored: bool, input: u16) -> u16
-requires input <= 255u16
 ensures result == input || result < input
 {
```

- Obligation line: `machine bounded(ignored: bool, input: u16) -> u16`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_contracts_disjunctive_ensures_drop_machine_requires_65ec0227595b/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `bounded(255u16, 0)` omega_exit=1

### 69. `tests/omega/pass/contracts/disjunctive_ensures/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,5 @@
 }
 data Main {}
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let picked: bool = value(true, 7);
 }
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_contracts_disjunctive_ensures_mut_ref_to_ref_21aa88fa3d46/main.omg`
- hole_subtype: ``; probe: 

### 70. `tests/omega/pass/contracts/type_conjunct_length_premise_sibling_write_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -22,5 +22,4 @@
     }
     machine store(bytes: &mut [u8], start: u64) -> Written
-    requires start < bytes.len
     {
         bytes[start] = 7u8;
```

- Obligation line: `machine poke(bytes: &mut [u8], start: u64) -> Written {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_contracts_type_conjunct_length_premise_sibling_write_exit_drop_machine_requires_1e20457a7c3b/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `store(bytes, 0)` omega_exit=1

### 71. `tests/omega/pass/contracts/type_conjunct_length_premise_sibling_write_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -22,5 +22,5 @@
     }
     machine store(bytes: &mut [u8], start: u64) -> Written
-    requires start < bytes.len
+    requires start <= bytes.len
     {
         bytes[start] = 7u8;
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_contracts_type_conjunct_length_premise_sibling_write_exit_widen_requires_lt_c951b463867c/main.omg`
- hole_subtype: ``; probe: 

### 72. `tests/omega/pass/control_flow/composite_state_parameter/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 //   literal    take_literal(Pair { a: 1, b: 2 }, 3)   unconditional edge
 //   field      take_field(main.pair)                    guarded edge
-//   call       take_call(make(&mut main.counter, 5), 6, main)
+//   call       take_call(make(&main.counter, 5), 6, main)
 //                                                       guarded edge; the
 //                                                       unselected arm's call
```

- Obligation line: `counter.made = counter.made + 1;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_composite_state_parameter_mut_ref_to_ref_d841157dd19f/main.omg`
- hole_subtype: ``; probe: 

### 73. `tests/omega/pass/control_flow/projected_loan_state_argument_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A `&main.<field>` successor argument loans the projected field into the
+// A `&mut main.<field>` successor argument loans the projected field into the
 // successor's borrowed parameter: the loan's root is the `main: &mut Main`
 // data parameter, and the state reads the field through the view.
```

- Obligation line: `// A `&mut main.<field>` successor argument loans the projected field into the`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_projected_loan_state_argument_exit_ref_to_mut_ref_668d7401309a/main.omg`
- hole_subtype: ``; probe: 

### 74. `tests/omega/pass/control_flow/projected_loan_state_argument_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // A `&main.<field>` successor argument loans the projected field into the
-// successor's borrowed parameter: the loan's root is the `main: &mut Main`
+// successor's borrowed parameter: the loan's root is the `main: &Main`
 // data parameter, and the state reads the field through the view.
 // exits: view.a, which the entry stores as 11.
```

- Obligation line: `main.pair = Pair { a: 11 };`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_projected_loan_state_argument_exit_mut_ref_to_ref_c6146e99f048/main.omg`
- hole_subtype: ``; probe: 

### 75. `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // The anonymous transition subject preserves the original exclusive writes and state-parameter local loan.
 // Expected exit: 0.
-// A `&mut` loan may name a plain local copied from a state parameter:
+// A `&mut mut` loan may name a plain local copied from a state parameter:
 // `fill(&mut held)` lends the local's claim — state parameters themselves
 // are read-only bindings, so the value moves through a local slot.
```

- Obligation line: `// A `&mut mut` loan may name a plain local copied from a state parameter:`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_runtime_anonymous_transition_subject_exit_ref_to_mut_ref_1da57cd2aa7f/main.omg`
- hole_subtype: ``; probe: 

### 76. `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 // Expected exit: 0.
 // A `&mut` loan may name a plain local copied from a state parameter:
-// `fill(&mut held)` lends the local's claim — state parameters themselves
+// `fill(&held)` lends the local's claim — state parameters themselves
 // are read-only bindings, so the value moves through a local slot.
 data CapturedLocal { answer: u64; }
```

- Obligation line: `cell = 70;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_runtime_anonymous_transition_subject_exit_mut_ref_to_ref_e17823ff1fa1/main.omg`
- hole_subtype: ``; probe: 

### 77. `tests/omega/pass/control_flow/runtime_entry_receiver_loan_with_byte_fields_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // A hosted entry keeps `self` and lends the whole receiver into its states,
-// the shape every migrated sample uses (`step(&mut self)` into
+// the shape every migrated sample uses (`step(&self)` into
 // `state step(main: &mut Main)`). `Main` holds a service binding, a plain byte
 // array and a domain-qualified byte field, so it is not a plain record carrier.
```

- Obligation line: `main.count = 3;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_runtime_entry_receiver_loan_with_byte_fields_exit_mut_ref_to_ref_791654947b61/main.omg`
- hole_subtype: ``; probe: 

### 78. `tests/omega/pass/control_flow/runtime_nonentry_state_local_loan_held_at_return_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A non-entry state lends its own local record `&mut` to a successor that
+// A non-entry state lends its own local record `&mut mut` to a successor that
 // completes the machine while still holding the loan: it writes through the
 // loan, lends it shared to an ordinary call that reports what it reads, and
```

- Obligation line: `// A non-entry state lends its own local record `&mut mut` to a successor that`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_runtime_nonentry_state_local_loan_held_at_return_eref_to_mut_ref_6ab22f68b2ad/main.omg`
- hole_subtype: ``; probe: 

### 79. `tests/omega/pass/control_flow/runtime_nonentry_state_local_loan_held_at_return_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -28,5 +28,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let start: u64 = 3;
     transition start > 2 {
```

- Obligation line: `transition score.bonus == 70 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_runtime_nonentry_state_local_loan_held_at_return_emut_ref_to_ref_21e6728341b7/main.omg`
- hole_subtype: ``; probe: 

### 80. `tests/omega/pass/control_flow/self_loop_state_compile/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -19,5 +19,4 @@
 
     machine again(main: &mut Main, count: u64)
-    requires count >= 1
     {
         transition 1 {
```

- Obligation line: `_ -> again(main, count)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_self_loop_state_compile_drop_machine_requires_33b0e18d42c6/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 81. `tests/omega/pass/control_flow/state_local_operand_edge_call_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 // and delivers its structural result into the target's owned parameter:
 //
-//   copy local     take_copy(Main::shift(&mut context.counter, seed, 10), context)
+//   copy local     take_copy(Main::shift(&context.counter, seed, 10), context)
 //                  `seed` is a `[copy]` record local; unconditional edge
 //   result local   take_result(Main::shift(&mut context.counter, made, 2), context)
```

- Obligation line: `counter.calls = counter.calls + 1;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_state_local_operand_edge_call_exit_mut_ref_to_ref_6d12e83f83c0/main.omg`
- hole_subtype: ``; probe: 

### 82. `tests/omega/pass/control_flow/transition_tail_self_call/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 
 machine Counter::count(step: u64) -> u64
-    requires step < 100
 {
     transition {
```

- Obligation line: `machine Counter::count(step: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_transition_tail_self_call_drop_machine_requires_97d47a818495/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 83. `tests/omega/pass/control_flow/transition_tail_self_call/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,5 @@
 
 machine Counter::count(step: u64) -> u64
-    requires step < 100
+    requires step <= 100
 {
     transition {
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_transition_tail_self_call_widen_requires_lt_97fdcb40189c/main.omg`
- hole_subtype: ``; probe: 

### 84. `tests/omega/pass/control_flow/transition_tail_self_call/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -17,5 +17,5 @@
 }
 
-machine Counter::run(counter: &mut Counter) -> u64 {
+machine Counter::run(counter: &Counter) -> u64 {
     transition {
         _ -> Counter::count(5)
```

- Obligation line: `machine Counter::run(counter: &Counter) -> u64 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_control_flow_transition_tail_self_call_mut_ref_to_ref_884675d67e29/main.omg`
- hole_subtype: ``; probe: 

### 85. `tests/omega/pass/core/content_retained_debt_only_wrapper/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,5 @@
     reaches
     ensures result in Pending::Retained crashes Trap;
-    machine complete<'source>(pending: Pending<'source> in Pending::Retained, observed: &mut u64) -> Capacity
+    machine complete<'source>(pending: Pending<'source> in Pending::Retained, observed: &u64) -> Capacity
     reaches
     ensures result in Capacity::Available crashes Trap;
```

- Obligation line: `machine complete<'source>(pending: Pending<'source> in Pending::Retained, observed: &u64) -> Capacity`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_core_content_retained_debt_only_wrapper_mut_ref_to_ref_4787dffa809b/main.omg`
- hole_subtype: ``; probe: 

### 86. `tests/omega/pass/data/runtime_record_field_value_pattern_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -15,5 +15,4 @@
 
 machine require_ok(header: Header)
-requires header.ok == 0
 {
 }
```

- Obligation line: `Header { ok: 0, version } -> matched(main, version)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_data_runtime_record_field_value_pattern_exit_drop_machine_requires_bbe60c0c9524/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 87. `tests/omega/pass/dependent/runtime_whole_view_collection_fact_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 
 machine count(cells: &[i32]) -> u64
-requires exactly_four(cells) == true
 { cells.len }
 
```

- Obligation line: `machine exactly_four(cells: &[i32]) -> bool { cells.len == 4 }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_dependent_runtime_whole_view_collection_fact_exit_drop_machine_requires_3e3a1e8491d9/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 88. `tests/omega/pass/domains/domain_signed_clamp/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let a: i8 in Saturating = 100;
     let b: i8 in Saturating = 100;
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_domain_signed_clamp_mut_ref_to_ref_221da7f7fe2e/main.omg`
- hole_subtype: ``; probe: 

### 89. `tests/omega/pass/domains/match_domain_patterns/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -26,5 +26,5 @@
 }
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     transition main.player {
         Player::Alive -> alive()
```

- Obligation line: `self.health >= 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_match_domain_patterns_mut_ref_to_ref_b1c9485fee81/main.omg`
- hole_subtype: ``; probe: 

### 90. `tests/omega/pass/domains/referent_field_byte_store_keeps_fixed_array_domain/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A named `&mut T` referent's fixed-array field keeps its byte class across an
+// A named `&T` referent's fixed-array field keeps its byte class across an
 // indexed store whose byte a taken-arm guard bounds. The store re-mints the
 // carrier's class at the referent's own root spelling -- the same law an
```

- Obligation line: `buffer.label = "Gate";`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_referent_field_byte_store_keeps_fixed_array_domain_mut_ref_to_ref_3840ec40af71/main.omg`
- hole_subtype: ``; probe: 

### 91. `tests/omega/pass/domains/runtime_constructor_ensured_membership_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -36,5 +36,5 @@
     stub.amount
 }
-machine Main::main(main: &mut Main) crashes Trap {
+machine Main::main(main: &Main) crashes Trap {
     observe(Envelope { ticket: Ticket::issue() });
     observe(envelope());
```

- Obligation line: `transition ticket.amount == 41 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_runtime_constructor_ensured_membership_exit_mut_ref_to_ref_421a3f1ee8fe/main.omg`
- hole_subtype: ``; probe: 

### 92. `tests/omega/pass/domains/write_domain_store/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // A write-only whole-leaf replacement discharges the incoming value's
 // domain fact on the write side: `tagged.label = label` proves
-// `in Utf8` against the stored value exactly as a `&mut` target would.
+// `in Utf8` against the stored value exactly as a `&mut mut` target would.
 // `in Wrapping` fields are behaviour tags on the carrier — every value
 // is already a member, no range proof owed.
```

- Obligation line: `// `in Utf8` against the stored value exactly as a `&mut mut` target would.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_write_domain_store_ref_to_mut_ref_42537963cb73/main.omg`
- hole_subtype: ``; probe: 

### 93. `tests/omega/pass/domains/write_domain_store/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -24,5 +24,5 @@
 data Main {
 }
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let mut tagged: Tagged;
     let mut policy: Policy;
```

- Obligation line: `tagged.label = label;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_domains_write_domain_store_mut_ref_to_ref_a4a8b7efb787/main.omg`
- hole_subtype: ``; probe: 

### 94. `tests/omega/pass/drops/case_subject_root_cleanup/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,5 @@
 }
 data Sink { }
-machine Sink::bump(sink: &mut Sink) { }
+machine Sink::bump(sink: &Sink) { }
 
 data Main { weight: u64; }
```

- Obligation line: `let item: Item = Item { kind: Kind::B, weight: 9 };`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_drops_case_subject_root_cleanup_mut_ref_to_ref_09edf35ac770/main.omg`
- hole_subtype: ``; probe: 

### 95. `tests/omega/pass/drops/runtime_named_parent_linear_cleanup_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 
 machine drop_receipt(receipt: &mut Receipt)
-requires receipt.value == 9
 {
     receipt.value = 0;
```

- Obligation line: `data Receipt [linear] dropped by drop_receipt { value: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_drops_runtime_named_parent_linear_cleanup_exit_drop_machine_requires_76b0a473c86e/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 96. `tests/omega/pass/drops/runtime_temporary_nominal_prerequisite_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,4 @@
 data Token dropped by drop_token { ready: bool; value: u64; }
 machine drop_token(token: &mut Token)
-requires token.ready
 {
     token.ready = false;
```

- Obligation line: `machine read(token: &Token) -> u64 { token.value }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_drops_runtime_temporary_nominal_prerequisite_exit_drop_machine_requires_097c25906462/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 97. `tests/omega/pass/dungeon/runtime_threaded_mut_arg_interrupt_soak_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Soak test for threaded `&mut` arguments under kernel preemption.
+// Soak test for threaded `&mut mut` arguments under kernel preemption.
 //
 // The dispatch loop copies pointer-sized frame slots with load/store pairs
```

- Obligation line: `// Soak test for threaded `&mut mut` arguments under kernel preemption.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_dungeon_runtime_threaded_mut_arg_interrupt_soak_exit_ref_to_mut_ref_15dd7370e967/main.omg`
- hole_subtype: ``; probe: 

### 98. `tests/omega/pass/dungeon/runtime_threaded_mut_arg_interrupt_soak_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,5 @@
 // reserved platform register) on every kernel->user return, so a value parked
 // in x18 across a timer interrupt is silently replaced with 0 -- the dungeon
-// crawler segfaulted this way when a threaded `&mut Level` slot copy lost its
+// crawler segfaulted this way when a threaded `&Level` slot copy lost its
 // pointer mid-generation (nondeterministic, landing wherever the first tick
 // fell). This canary threads a `&mut Tally` through a dispatched call fifty
```

- Obligation line: `tally.value = tally.value + 1;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_dungeon_runtime_threaded_mut_arg_interrupt_soak_exit_mut_ref_to_ref_6e8c35fb63f8/main.omg`
- hole_subtype: ``; probe: 

### 99. `tests/omega/pass/entry/nested_where_record_binding_field_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -13,5 +13,5 @@
 }
 
-machine Main::main(main: &mut Main)
+machine Main::main(main: &Main)
 reaches Console
 {
```

- Obligation line: `machine Main::main(main: &Main)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_entry_nested_where_record_binding_field_exit_mut_ref_to_ref_fd6b20501d3a/main.omg`
- hole_subtype: ``; probe: 

### 100. `tests/omega/pass/expressions/giant_literal_subject/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -16,5 +16,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     transition choose() == 7 {
         true -> done()
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_expressions_giant_literal_subject_mut_ref_to_ref_2b51a0d3de47/main.omg`
- hole_subtype: ``; probe: 

### 101. `tests/omega/pass/facts/abstract_signature_call_applied_callback_computed_const_premise_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     direct
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_applied_callback_computed_const_pdrop_machine_requires_6aadcf4340a9/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 102. `tests/omega/pass/facts/abstract_signature_call_applied_callback_structural_type_argument_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     direct
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_applied_callback_structural_type_drop_machine_requires_2151ef4493c1/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 103. `tests/omega/pass/facts/abstract_signature_call_applied_callback_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     direct
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_applied_callback_through_parametedrop_machine_requires_fc3a8096f4f3/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 104. `tests/omega/pass/facts/abstract_signature_call_constant_initializer_supplied_body_terminal_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract clause that is not a comparison or conjunction`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_constant_initializer_supplied_boddrop_machine_requires_75abe3cbdbbe/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 105. `tests/omega/pass/facts/abstract_signature_call_constant_initializer_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract clause that is not a comparison or conjunction`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_constant_initializer_through_othedrop_machine_requires_f5510397905a/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 106. `tests/omega/pass/facts/abstract_signature_call_through_local_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract clause that is not a comparison or conjunction`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_abstract_signature_call_through_local_exit_drop_machine_requires_8cf16059d91f/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 107. `tests/omega/pass/facts/affine_discard_root_cleanup_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -13,5 +13,4 @@
 
 machine drop_resource(resource: &mut Resource)
-requires resource.valid == 1u64
 {
     resource.dropped.value = resource.valid;
```

- Obligation line: `machine exercise(dropped: &mut Counter) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_affine_discard_root_cleanup_through_call_result_exit_drop_machine_requires_2de37b75d464/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 108. `tests/omega/pass/facts/affine_discard_root_cleanup_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -13,5 +13,4 @@
 
 machine drop_resource(resource: &mut Resource)
-requires resource.valid == 1u64
 {
     resource.dropped.value = resource.valid;
```

- Obligation line: `let diff: u64 = 50u64 - result;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_affine_discard_root_cleanup_through_target_body_exit_drop_machine_requires_9d4f47f9b362/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 109. `tests/omega/pass/facts/applied_requirement_indices_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,4 @@
     machine paired<Element>(value: Payload in Pair<Word, Element>) -> Payload in Pair<Word, Element> { value }
     machine stated(value: Payload in Tagged<Word>) -> Payload in Tagged<Word>
-    requires value in Tagged<Word>
     ensures result in Tagged<Word>
     { value }
```

- Obligation line: `data Payload [copy] { value: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_applied_requirement_indices_through_match_join_exit_drop_machine_requires_b11b2be61a3a/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `stated(Word)` omega_exit=1

### 110. `tests/omega/pass/facts/as_slice_views_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 data Entry { value: i32; }
 machine require_four(length: u64) -> u64
-requires length == 4
 { length }
 machine extent(entries: &[Entry; 4], flag: bool) -> u64 {
```

- Obligation line: `machine require_four(length: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_as_slice_views_through_match_join_exit_drop_machine_requires_fba87d54814b/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 111. `tests/omega/pass/facts/body_bearing_comparison_stronger_fields_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract contracts on a state`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -19,5 +19,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == false
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_body_bearing_comparison_stronger_fields_exit_drop_machine_requires_20f22002bd7d/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 112. `tests/omega/pass/facts/body_bearing_comparison_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine operator spelling`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -17,5 +17,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == false
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_body_bearing_comparison_through_parameter_exit_drop_machine_requires_17f29e851b34/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 113. `tests/omega/pass/facts/borrowed_case_result_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,4 @@
     }
     machine header(input: &'input [u8]) -> BorrowedCase<'input>
-    requires input.len > 0u64
     {
         let length: u64 = input[0u64] as u64;
```

- Obligation line: `data BorrowedCase<'input> { case Missing; case Bytes(value: &'input [u8]); }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_borrowed_case_result_through_other_state_exit_drop_machine_requires_51e335c4605b/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `header(0u64)` omega_exit=1

### 114. `tests/omega/pass/facts/bounded_activation_parameter_committed_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data without a checked shape`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
     machine live(claim: ActivationClaim) -> u64 {
         let mut owner: ActivationClaim = claim;
-        let bytes: &mut [u8] = ActivationClaim::bytes(&mut owner);
+        let bytes: &[u8] = ActivationClaim::bytes(&mut owner);
         let count: u64 = bytes.len;
         ActivationClaim::release(owner);
```

- Obligation line: `let mut owner: ActivationClaim = claim;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_bounded_activation_parameter_committed_exit_mut_ref_to_ref_2958b6b0cb35/main.omg`
- hole_subtype: ``; probe: 

### 115. `tests/omega/pass/facts/call_result_bound_discharge_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine echo(scale: u64) -> u64
-requires scale <= 5
 {
     scale
```

- Obligation line: `machine echo(scale: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_call_result_bound_discharge_through_parameter_exit_drop_machine_requires_7b87ee9a5f13/main.omg`
- hole_subtype: `unchecked`; probe: probe call `echo(6)` omega_exit=0

### 116. `tests/omega/pass/facts/case_payload_exclusive_loan_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_case_payload_exclusive_loan_through_const_name_exit_drop_machine_requires_8f2d7e338bcd/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 117. `tests/omega/pass/facts/content_reunion_through_field_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine type equations, owned data or conformances`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -24,5 +24,4 @@
 
 machine rebuild_region(region: Region in Owned) -> Region in Owned
-requires region.base <= 58 && region.length <= 58 - region.base
 ensures result.base == region.base, result.length == region.length,
     core::Identity<IntervalSet<Nat>>(Owned::content(old(&region)), Owned::content(&result))
```

- Obligation line: `data Region [linear] dropped by drop_region { base: u64; length: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_content_reunion_through_field_exit_drop_machine_requires_03d865998502/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `rebuild_region(59)` omega_exit=1

### 118. `tests/omega/pass/facts/darwin_stat_abi_through_other_state_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,5 @@
 
 public boundary trait StatExchange {
-    machine fill(value: &mut DarwinStat) reaches;
+    machine fill(value: &DarwinStat) reaches;
     machine check(value: &DarwinStat) -> u64 reaches;
     machine extent() -> u64 reaches;
```

- Obligation line: `value.dev == -2147483647i32 && value.mode == 33188u16 && value.nlink == 3u16 && value.ino == 4294967298u64 && value.uid == 123u32 && value.gid == 456u32 && value.rdev == -123456789i32 && value.atime.seconds == -17i64 && value.atime.nanoseconds == 123456789i64 && value.mtime.seconds == 42i64 && value.mtime.nanoseconds == 987654321i64 && value.ctime.seconds == -9i64 && value.ctime.nanoseconds == 111111111i64 && value.btime.seconds == 77i64 && value.btime.nanoseconds == 222222222i64 && value.size == 8589934593i64 && value.blocks == 123456i64 && value.blksize == 4096i32 && value.flags == 2147483649u32 && value.generation == 2882400001u32 && value.reserved == -314159i32 && value.spare[0] == -4294967297i64 && value.spare[1] == 68719476737i64 && StatExchange::check(&value) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(value.rdev)) == 4171510507u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_darwin_stat_abi_through_other_state_exit_mut_ref_to_ref_a80a6df18354/main.omg`
- hole_subtype: ``; probe: 

### 119. `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_descriptor_payload_exclusive_loan_through_const_name_exitdrop_machine_requires_27a548c84b4a/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 120. `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_descriptor_range_exclusive_loan_through_call_result_exit_drop_machine_requires_35d5930fa8e9/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 121. `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_descriptor_range_exclusive_loan_through_target_body_exit_drop_machine_requires_360e0f4c4d34/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 122. `tests/omega/pass/facts/domain_predicate_call_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration const`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     direct
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_domain_predicate_call_through_const_name_exit_drop_machine_requires_ae6ddb065c8f/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 123. `tests/omega/pass/facts/erased_reference_contract_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 data Witness { value: i32; }
 machine keep(runtime: i32, witness [erased]: &i32, record [erased]: &Witness) -> i32
-requires witness == 7
 requires record.value == 7 { runtime }
 machine produce(flag: bool) -> i32
```

- Obligation line: `machine keep(runtime: i32, witness [erased]: &i32, record [erased]: &Witness) -> i32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_erased_reference_contract_through_call_result_exit_drop_machine_requires_77b842a73ebb/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 124. `tests/omega/pass/facts/erased_reference_contract_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 data Witness { value: i32; }
 machine keep(runtime: i32, witness [erased]: &i32, record [erased]: &Witness) -> i32
-requires witness == 7
 requires record.value == 7 { runtime }
 boundary trait SelectedBodyCheck {
```

- Obligation line: `machine keep(runtime: i32, witness [erased]: &i32, record [erased]: &Witness) -> i32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_erased_reference_contract_through_target_body_exit_drop_machine_requires_cc0ce49235de/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 125. `tests/omega/pass/facts/exclusive_match_arm_through_other_machine_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 // Expected exit: 0
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_exclusive_match_arm_through_other_machine_exit_drop_machine_requires_76efb75faaf3/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 126. `tests/omega/pass/facts/exclusive_observation_exposure_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,4 @@
 // Expected exit: 0
 machine agree(observed: u64, current: u64) -> u64
-requires observed == current && observed == 7
 {
     observed
```

- Obligation line: `machine agree(observed: u64, current: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_exclusive_observation_exposure_through_match_join_exit_drop_machine_requires_0ebba851b7a8/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 127. `tests/omega/pass/facts/exclusive_observation_exposure_through_match_join_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,5 @@
 }
 
-machine expose(slot: &mut u64) -> u64 {
+machine expose(slot: &u64) -> u64 {
     slot
 }
```

- Obligation line: `requires observed == current && observed == 7`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_exclusive_observation_exposure_through_match_join_exit_mut_ref_to_ref_a863b35b0dc6/main.omg`
- hole_subtype: ``; probe: 

### 128. `tests/omega/pass/facts/field_where_index_through_local_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,5 @@
 }
 
-machine read(main: &Main) -> i32 in Wrapping {
+machine read(main: &mut Main) -> i32 in Wrapping {
     let index: u64 = main.k;
     main.arr[index + 1]
```

- Obligation line: `machine read(main: &mut Main) -> i32 in Wrapping {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_field_where_index_through_local_exit_ref_to_mut_ref_d6a11cab71f4/main.omg`
- hole_subtype: ``; probe: 

### 129. `tests/omega/pass/facts/float_range_delivery_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 data Reading [copy] requires value >= 0.0 && value <= 0.25 { value: f64; }
 machine admit(value: f64) -> f64
-requires value >= 0.0 && value <= 0.5
 { value }
 machine probe(left: Reading, right: Reading, choice: bool) -> f64 {
```

- Obligation line: `data Reading [copy] requires value >= 0.0 && value <= 0.25 { value: f64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_float_range_delivery_through_match_join_exit_drop_machine_requires_78075d142db8/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `admit(1)` omega_exit=1

### 130. `tests/omega/pass/facts/generic_comparison_body_through_other_machine_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == false
 {
     true
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_generic_comparison_body_through_other_machine_exit_drop_machine_requires_70c28ad94e0f/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 131. `tests/omega/pass/facts/guarded_call_entry_through_const_name_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 // Expected exit: 0
 public data CompactBinary {}
-public data Ctx { slots: &mut [u64]; count: u64; valid: bool; }
+public data Ctx { slots: &[u64]; count: u64; valid: bool; }
 public data Observation [copy] { remaining: u64; remaining_steps: u64; }
 const INITIAL_FUEL: u64 = 2u64;
```

- Obligation line: `const INITIAL_FUEL: u64 = 2u64;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_guarded_call_entry_through_const_name_exit_mut_ref_to_ref_9d55ac8500a6/main.omg`
- hole_subtype: ``; probe: 

### 132. `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine verify_f32_window(reading: f32) -> i32
-requires reading >= 1.0f32 && reading <= 3.0f32
 {
     transition reading >= 1.25f32 && reading <= 2.75f32 {
```

- Obligation line: `data Reading [copy] requires value >= 1.25f32 && value <= 2.75f32 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_ieee_float_32bit_ordered_bound_through_other_state_exit_drop_machine_requires_aac3c32fa734/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `verify_f32_window(4)` omega_exit=1

### 133. `tests/omega/pass/facts/ieee_float_ordered_comparison_through_other_machine_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
 
 machine verify_window(reading: f64) -> i32
-requires reading >= 1.0 && reading <= 3.0
 {
     0i32
```

- Obligation line: `machine verify_window(reading: f64) -> i32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_ieee_float_ordered_comparison_through_other_machine_exit_drop_machine_requires_190a7991c776/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `verify_window(4)` omega_exit=1

### 134. `tests/omega/pass/facts/indexed_storage_address_identity_through_local_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type fixed array`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
     let mut items: [u64; 2] = [7, 11];
     let index: u64 = input;
-    let selected: addr = address_of(&items[index]);
+    let selected: addr = address_of(&mut items[index]);
     let again: addr = address_of(&items[index]);
     let first: addr = address_of(&items[0]);
```

- Obligation line: `let selected: addr = address_of(&mut items[index]);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_indexed_storage_address_identity_through_local_exit_ref_to_mut_ref_344cd1db0376/main.omg`
- hole_subtype: ``; probe: 

### 135. `tests/omega/pass/facts/induction_call_guarantee_through_field_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type constrained`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,4 @@
 
 machine observe(total: u64 in Wrapping, n: u64 in Wrapping) -> bool
-requires total * 2 == n * (n + 1)
 {
     true
```

- Obligation line: `machine gauss(n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_induction_call_guarantee_through_field_exit_drop_machine_requires_de6bc16d9177/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 136. `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_const_name_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration const`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: masking an unsigned 64-bit integer to its low byte (val & 255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.
+// Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.
 // Expected exit: 0
 
```

- Obligation line: `// Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_integer_width_truncation_mask_bound_through_const_name_exref_to_mut_ref_e54b34fc2677/main.omg`
- hole_subtype: ``; probe: 

### 137. `tests/omega/pass/facts/invariant_window_snapshot_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
 machine require_nonnegative(value: i32) requires value >= 0 {}
 machine consume(value: i32) -> i32
-requires value == -1
 ensures result == value
 { value }
```

- Obligation line: `machine identity(value: i32) -> i32 ensures result == value { value }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_invariant_window_snapshot_through_call_result_exit_drop_machine_requires_78d48b1eb05e/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 138. `tests/omega/pass/facts/invariant_window_snapshot_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine invokes`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
 public boundary trait SnapshotConsumer {
     machine consume(value: i32) -> i32
-    requires value == -1
     ensures result == value
     reaches;
```

- Obligation line: `machine consume(value: i32) -> i32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_invariant_window_snapshot_through_target_body_exit_drop_machine_requires_effe28470cf6/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 139. `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration const`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 // Expected exit: 0
 machine step1(x: u64) -> u64
-    requires x == 10
     ensures result == 11
 {
```

- Obligation line: `machine step1(x: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_live_across_two_calls_preservation_bound_through_const_nadrop_machine_requires_1b45e60d7f31/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 140. `tests/omega/pass/facts/match_join_place_read_through_local_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,4 @@
 
 machine read_field(place: &Pair) -> i64
-requires place.kept == 17
 {
     place.kept
```

- Obligation line: `machine read_field(place: &Pair) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_match_join_place_read_through_local_exit_drop_machine_requires_30a217ae0292/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 141. `tests/omega/pass/facts/match_loan_last_use_through_field_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine choose(counter: &mut u64) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_match_loan_last_use_through_field_exit_drop_machine_requires_29a524b7a32a/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 142. `tests/omega/pass/facts/match_primitive_local_origin_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration const`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -13,5 +13,4 @@
 
 machine read_origin(value: i64) -> i64
-requires value == 17
 {
     value
```

- Obligation line: `machine read_origin(value: i64) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_match_primitive_local_origin_through_const_name_exit_drop_machine_requires_466cd8f608f8/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 143. `tests/omega/pass/facts/match_runtime_view_extent_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 
 machine choose(counter: &mut u64, flag: bool) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64, flag: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_match_runtime_view_extent_through_other_state_exit_drop_machine_requires_e6a0d98516f7/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 144. `tests/omega/pass/facts/may_write_codec_argument_through_local_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -21,5 +21,5 @@
     let mut buffer: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
     let sample: Blob = Blob { bytes: source[0..2] };
-    Blob::encode(&sample, &mut buffer, choose(&mut chosen, &mut other, true));
+    Blob::encode(&sample, &buffer, choose(&mut chosen, &mut other, true));
     transition chosen == 5 && items[other] == 7 {
         true -> done()
```

- Obligation line: `let mut chosen: u64 = 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_may_write_codec_argument_through_local_exit_mut_ref_to_ref_4b9750da323d/main.omg`
- hole_subtype: ``; probe: 

### 145. `tests/omega/pass/facts/may_write_nested_helper_through_parameter_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -25,5 +25,5 @@
 
 machine probe(chosen: &mut u64, other: &mut u64, items: &[u8; 4]) -> u8
-requires chosen < 4 && other < 4
+requires chosen <= 4 && other < 4
 {
     let zero: u64 in Wrapping = 0;
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_may_write_nested_helper_through_parameter_exit_widen_requires_lt_531c3a7cd05f/main.omg`
- hole_subtype: ``; probe: 

### 146. `tests/omega/pass/facts/may_write_stored_rebind_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -30,5 +30,4 @@
     transition { _ -> read(items, other, chosen, marker) }
     machine read(items: [u8; 4], index: u64, chosen: u64, marker: u64 in Wrapping)
-    requires index < 4
     {
         transition (marker as u64) == 1 && chosen == 9 && items[index] == 7 {
```

- Obligation line: `machine read(items: [u8; 4], index: u64, chosen: u64, marker: u64 in Wrapping)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_may_write_stored_rebind_through_other_state_exit_drop_machine_requires_ec04b978c22f/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `read(4, 0, 0, 0)` omega_exit=1

### 147. `tests/omega/pass/facts/may_write_stored_rebind_through_other_state_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -30,5 +30,5 @@
     transition { _ -> read(items, other, chosen, marker) }
     machine read(items: [u8; 4], index: u64, chosen: u64, marker: u64 in Wrapping)
-    requires index < 4
+    requires index <= 4
     {
         transition (marker as u64) == 1 && chosen == 9 && items[index] == 7 {
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_may_write_stored_rebind_through_other_state_exit_widen_requires_lt_1f75fdea6fcd/main.omg`
- hole_subtype: ``; probe: 

### 148. `tests/omega/pass/facts/method_array_operand_inference_through_other_machine_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: with Count supplied as 3, the omitted element T of Bytes::last is inferred as u8 from the actual &[u8; 3], and the selected implementation's [T; Count] carrier is the caller's copyable [u8; 3].
+// Fact: with Count supplied as 3, the omitted element T of Bytes::last is inferred as u8 from the actual &mut [u8; 3], and the selected implementation's [T; Count] carrier is the caller's copyable [u8; 3].
 // Expected exit: 0
 boundary trait Bytes {
```

- Obligation line: `// Fact: with Count supplied as 3, the omitted element T of Bytes::last is inferred as u8 from the actual &mut [u8; 3], and the selected implementation's [T; Count] carrier is the caller's copyable [u8; 3].`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_method_array_operand_inference_through_other_machine_exitref_to_mut_ref_fec46cb225f8/main.omg`
- hole_subtype: ``; probe: 

### 149. `tests/omega/pass/facts/mutable_borrow_field_writeback_through_field_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: Updating a record field through an exclusive mutable borrow (`&mut Counter`) and retiring the loan establishes the updated field bound (`counter.used <= 80u64`) while preserving unrelated fields, discharging exact subtraction `80u64 - counter.used` across all nine places.
+// Fact: Updating a record field through an exclusive mutable borrow (`&Counter`) and retiring the loan establishes the updated field bound (`counter.used <= 80u64`) while preserving unrelated fields, discharging exact subtraction `80u64 - counter.used` across all nine places.
 // Place: field. The counter is a nested field of an outer aggregate and is projected through `&mut cont.counter`.
 // Expected exit: 0
```

- Obligation line: `data Counter [copy] requires used <= 80u64 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_mutable_borrow_field_writeback_through_field_exit_mut_ref_to_ref_2b7183463c38/main.omg`
- hole_subtype: ``; probe: 

### 150. `tests/omega/pass/facts/mutable_slice_element_write_bound_through_const_name_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: Writing an element through a mutable slice view (&mut [u64]) whose length is guarded (view.len >= 2u64) proves safe indexed element mutation at view[1u64] while preserving the slice extent lower bound (view.len >= 2u64) to discharge exact subtraction view.len - 2u64 across all nine places.
+// Fact: Writing an element through a mutable slice view (&[u64]) whose length is guarded (view.len >= 2u64) proves safe indexed element mutation at view[1u64] while preserving the slice extent lower bound (view.len >= 2u64) to discharge exact subtraction view.len - 2u64 across all nine places.
 // Place: const name. The guard obligation references a declared symbolic constant.
 // Expected exit: 0
```

- Obligation line: `ensures result.len == 3`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_mutable_slice_element_write_bound_through_const_name_exitmut_ref_to_ref_640ea2e689ef/main.omg`
- hole_subtype: ``; probe: 

### 151. `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_other_state_exit/main.omg` (ref_to_mut_ref, accepted_by=both)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x & 31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.
+// Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut  31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.
 // Expected exit: 0
 
```

- Obligation line: `// Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut  31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_narrow_to_narrow_truncation_64to8_mask_bound_through_otheref_to_mut_ref_7c4253cd12a4/main.omg`
- hole_subtype: ``; probe: 

### 152. `tests/omega/pass/facts/nested_borrow_qualification_through_local_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -22,5 +22,5 @@
 
 data Main {}
-machine Main::main(main: &mut Main) crashes Trap {
+machine Main::main(main: &Main) crashes Trap {
     let mut words: [u16; 4];
     words[0] = 11;
```

- Obligation line: `public domain [u16; 4]::Terminated requires self[3] == 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_nested_borrow_qualification_through_local_exit_mut_ref_to_ref_cba948c55d86/main.omg`
- hole_subtype: ``; probe: 

### 153. `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,4 @@
 // Expected exit: 0
 machine count(counter: &mut u64, items: &[u8], cursor: u64)
-requires items.len > 0
 requires cursor < items.len
 terminates
```

- Obligation line: `machine count(counter: &mut u64, items: &[u8], cursor: u64)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_nested_machine_joint_rank_bounded_computed_length_call_redrop_machine_requires_90effbb6474e/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `count(0, 0, 0)` omega_exit=1

### 154. `tests/omega/pass/facts/nested_machine_suspending_cycle_local_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,4 @@
     }
     machine step(total: u64, remaining: u64) -> u64 reaches Scheduler suspends
-    requires remaining > 0
     {
         let seen: u64 = suspend Scheduler::advance(remaining);
```

- Obligation line: `machine sum(total: u64, remaining: u64) -> u64 reaches Scheduler suspends terminates by remaining {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_nested_machine_suspending_cycle_local_exit_drop_machine_requires_4096540240ef/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `step(0, 0)` omega_exit=1

### 155. `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -26,5 +26,4 @@
 data Room dropped by drop_room { count: u32 in Wrapping; }
 machine drop_room(room: &mut Room)
-requires room.count == 10
 reaches Console
 {
```

- Obligation line: `machine write(descriptor: i32, buffer: &mut i32, count: u64) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_nominal_loan_backing_cleanup_through_match_join_exit_drop_machine_requires_bdb8b707338b/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 156. `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_match_join_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,5 @@
 
 boundary trait PosixConsoleIo {
-    machine write(descriptor: i32, buffer: &mut i32, count: u64) -> i64
+    machine write(descriptor: i32, buffer: &i32, count: u64) -> i64
     reaches Console
     terminates;
```

- Obligation line: `requires room.count == 10`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_nominal_loan_backing_cleanup_through_match_join_exit_mut_ref_to_ref_b8a6ed97f321/main.omg`
- hole_subtype: ``; probe: 

### 157. `tests/omega/pass/facts/progress_selector_nested_arrival_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_progress_selector_nested_arrival_through_call_result_exitdrop_machine_requires_840c66c3d50f/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 158. `tests/omega/pass/facts/progress_selector_nested_arrival_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_progress_selector_nested_arrival_through_target_body_exitdrop_machine_requires_19258b293f89/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 159. `tests/omega/pass/facts/progress_selector_owned_arrival_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_progress_selector_owned_arrival_through_parameter_exit_drop_machine_requires_000c8ebec4e0/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 160. `tests/omega/pass/facts/progress_selector_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_progress_selector_through_other_state_exit_drop_machine_requires_b5e3ffeca0f1/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 161. `tests/omega/pass/facts/projection_receiver_call_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration domain`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     direct
```

- Obligation line: `machine hold(direct: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_projection_receiver_call_through_match_join_exit_drop_machine_requires_48047a4fecf6/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 162. `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine invokes`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,4 @@
 }
 machine need(value: u64) -> u64 invokes Endpoint
-requires value <= independent_limit()
 { value }
 machine limit() -> u64 {
```

- Obligation line: `machine independent_limit() -> u64 invokes Endpoint {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_provider_contract_endpoint_pending_call_through_parameterdrop_machine_requires_906de95d7463/main.omg`
- hole_subtype: `unchecked`; probe: probe call `need(independent_limit)` omega_exit=0

### 163. `tests/omega/pass/facts/qualified_field_capacity_through_other_machine_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -15,5 +15,4 @@
 data Issued { strategy: Strategy; allocation: Tail; }
 machine check_length(tail: &Tail, expected: u64)
-requires tail.length == expected
 {}
 machine split(whole: Tail in Tracked & Partitionable, length: u64) -> Parts
```

- Obligation line: `machine split(whole: Tail in Tracked & Partitionable, length: u64) -> Parts`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_qualified_field_capacity_through_other_machine_exit_drop_machine_requires_2bafe61b4a35/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 164. `tests/omega/pass/facts/range_fact_delivery_through_local_exit/main.omg` (drop_machine_requires, accepted_by=both)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,4 @@
 // Expected exit: 0.
 machine count(index: u64) -> u64
-requires index <= 4
 { index }
 machine probe(input: u64) -> u64 requires input < 4 {
```

- Obligation line: `machine count(index: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_range_fact_delivery_through_local_exit_drop_machine_requires_8f4fba698260/main.omg`
- hole_subtype: `unchecked`; probe: probe call `count(5)` omega_exit=0

### 165. `tests/omega/pass/facts/reference_recast_backing_footprint_through_local_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data with custody`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,5 @@
 machine probe() -> bool {
     let mut bytes: [u8; 4] = [11, 23, 37, 41];
-    let original: addr = address_of(&bytes[1]);
+    let original: addr = address_of(&mut bytes[1]);
     let view: &Pair = &bytes[1] as &Pair;
     address_of(&view.first) == original && view.second == 37
```

- Obligation line: `let original: addr = address_of(&mut bytes[1]);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_reference_recast_backing_footprint_through_local_exit_ref_to_mut_ref_e04449063b5b/main.omg`
- hole_subtype: ``; probe: 

### 166. `tests/omega/pass/facts/reflection_declaration_inspection_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,4 @@
 }
 machine describe<Kind>(counts: &mut Counts, field: &reflect::FieldInfo)
-requires Kind == u64 || Kind == bool
 {
     counts.fields = ((counts.fields as u64 in Wrapping) + (1u64 as u64 in Wrapping)) as u64;
```

- Obligation line: `data Subject { first: u64; second: bool; proof[erased]: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_reflection_declaration_inspection_through_const_name_exitdrop_machine_requires_bd36c116f3b0/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 167. `tests/omega/pass/facts/result_cleanup_guarantees_through_call_result_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// The shared Seed.ready == true observation survives each place into make_token(&seed), whose completed result is ready for normal shared-call cleanup.
+// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.
 // Expected exit: 0
 data Seed [copy] { ready: bool; }
```

- Obligation line: `// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_result_cleanup_guarantees_through_call_result_exit_ref_to_mut_ref_076d75f7cbcd/main.omg`
- hole_subtype: ``; probe: 

### 168. `tests/omega/pass/facts/result_cleanup_guarantees_through_target_body_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// The shared Seed.ready == true observation survives each place into make_token(&seed), whose completed result is ready for normal shared-call cleanup.
+// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.
 // Expected exit: 0
 data Seed [copy] { ready: bool; }
```

- Obligation line: `// The shared Seed.ready == true observation survives each place into make_token(&mut seed), whose completed result is ready for normal shared-call cleanup.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_result_cleanup_guarantees_through_target_body_exit_ref_to_mut_ref_d1270696a321/main.omg`
- hole_subtype: ``; probe: 

### 169. `tests/omega/pass/facts/returned_indexed_linear_residual_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 data Receipt [linear] dropped by drop_receipt { code: u64; }
 machine drop_receipt(receipt: &mut Receipt)
-requires receipt.code == 7
 {
     receipt.code = 0;
```

- Obligation line: `data Seed [copy] { code: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_returned_indexed_linear_residual_through_parameter_exit_drop_machine_requires_e3633827426d/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 170. `tests/omega/pass/facts/returned_linear_residual_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,4 @@
 data Receipt [linear] dropped by drop_receipt { code: u64; }
 machine drop_receipt(receipt: &mut Receipt)
-requires receipt.code == 7
 {
     receipt.code = 0;
```

- Obligation line: `data Seed [copy] { code: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_returned_linear_residual_through_other_state_exit_drop_machine_requires_4e44db0eb346/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 171. `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_violation_unreported` (must not be runtime-trap-only; route hint `route_us=9988`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,4 @@
     }
     machine check_high(v: u64) -> u64
-        requires v >= 10u64
     {
         transition v > 100u64 {
```

- Obligation line: `machine clamp(val: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_saturating_integer_arithmetic_through_parameter_exit_drop_machine_requires_885a2641d515/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 172. `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_field_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,4 @@
 data Note { before: u64; observed: u64; after: u64; }
 machine agree(observed: u64, current: u64) -> u64
-requires observed == current && observed == 7
 {
     observed
```

- Obligation line: `machine agree(observed: u64, current: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_saved_observation_snapshot_after_call_through_field_exit_drop_machine_requires_054af425cd1c/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 173. `tests/omega/pass/facts/saved_observation_snapshot_indexed_handback_const_name_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 
 public data Holder { before: u64; items: [u64; 2]; after: u64; }
-machine kept(values: &[u64; 2]) -> bool { values[0] == 7 }
+machine kept(values: &mut [u64; 2]) -> bool { values[0] == 7 }
 public machine preserve(holder: &mut Holder, kept_index: u64, changed: u64, value: u64)
 requires kept_index < 2 && changed < 2 && kept_index != changed && holder.items[kept_index] == 7
```

- Obligation line: `machine kept(values: &mut [u64; 2]) -> bool { values[0] == 7 }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_saved_observation_snapshot_indexed_handback_const_name_exref_to_mut_ref_6a3e791b2312/main.omg`
- hole_subtype: ``; probe: 

### 174. `tests/omega/pass/facts/saved_observation_snapshot_through_match_join_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 
 machine agree(observed: u64, current: u64) -> u64
-requires observed == current && observed == 7
 {
     observed
```

- Obligation line: `machine agree(observed: u64, current: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_saved_observation_snapshot_through_match_join_exit_drop_machine_requires_9fd6089ebf72/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 175. `tests/omega/pass/facts/selected_native_code_layout_through_field_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,4 @@
     transition layout.regions.len > 0 { true -> region(layout) _ -> false }
     machine region(layout: &CodeLayout) -> bool
-    requires layout.regions.len > 0
     {
         layout.entry != 0 && layout.regions[0].base == layout.entry && layout.regions[0].length > 0
```

- Obligation line: `machine inspect(layout: &CodeLayout) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_selected_native_code_layout_through_field_exit_drop_machine_requires_3be168f60253/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `region(0)` omega_exit=1

### 176. `tests/omega/pass/facts/selected_provider_issuance_through_const_name_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -86,5 +86,5 @@
 }
 data Main {}
-machine Main::main(context: &mut Main) reaches InitialSupply + Writer + NativeBytes crashes Trap {
+machine Main::main(context: &Main) reaches InitialSupply + Writer + NativeBytes crashes Trap {
     let buffer: Buffer in Buffer::Owned = InitialSupply::acquire();
     let original_pointer: addr = buffer.pointer;
```

- Obligation line: `const ALLOCATION_COUNT: u64 = 1;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_selected_provider_issuance_through_const_name_exit_mut_ref_to_ref_95cf3e349d8a/main.omg`
- hole_subtype: ``; probe: 

### 177. `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_violation_unreported` (must not be runtime-trap-only; route hint `route_us=35280`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: 64-bit signed bitwise AND mask raw & 63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.
+// Fact: 64-bit signed bitwise AND mask raw &mut  63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.
 // Place: local. The 64-bit signed operand and mask are copied into locals before bitwise AND and interval discharge.
 // Expected exit: 0
```

- Obligation line: `// Fact: 64-bit signed bitwise AND mask raw &mut  63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_signed_64bit_bitwise_and_mask_bound_through_local_exit_ref_to_mut_ref_3b2c6b6dd664/main.omg`
- hole_subtype: ``; probe: 

### 178. `tests/omega/pass/facts/signed_arithmetic_shift_right_through_local_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_violation_unreported` (must not be runtime-trap-only; route hint `route_us=2146`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 
 machine consume(diff: i64) -> i64
-    requires diff >= 0i64 && diff <= 0i64
 {
     diff
```

- Obligation line: `machine consume(diff: i64) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_signed_arithmetic_shift_right_through_local_exit_drop_machine_requires_f13326968c12/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `consume(0i64)` omega_exit=1

### 179. `tests/omega/pass/facts/signed_exact_addition_bound_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=both)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 
 machine consume_margin(margin: i64) -> i64
-    requires margin >= 0i64 && margin <= 40i64
 {
     margin
```

- Obligation line: `machine consume_margin(margin: i64) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_signed_exact_addition_bound_through_call_result_exit_drop_machine_requires_c1f914b1cf9d/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `consume_margin(40i64)` omega_exit=1

### 180. `tests/omega/pass/facts/signed_exact_addition_bound_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 
 machine consume_margin(margin: i64) -> i64
-    requires margin >= 0i64 && margin <= 40i64
 {
     margin
```

- Obligation line: `machine consume_margin(margin: i64) -> i64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_signed_exact_addition_bound_through_target_body_exit_drop_machine_requires_445084513d12/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `consume_margin(40i64)` omega_exit=1

### 181. `tests/omega/pass/facts/signed_exact_multiplication_bound_through_parameter_exit/main.omg` (drop_machine_requires, accepted_by=both)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 
 machine verify_product(product: i64) -> i32
-requires product >= 40 && product <= 90
 {
     0i32
```

- Obligation line: `machine verify_product(product: i64) -> i32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_signed_exact_multiplication_bound_through_parameter_exit_drop_machine_requires_f134649a8227/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `verify_product(91)` omega_exit=1

### 182. `tests/omega/pass/facts/slice_view_element_read_bound_through_call_result_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: a slice view (&[u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.
+// Fact: a slice view (&mut [u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.
 // Place: call result. The slice view arrives from a call return result guaranteeing the extent bound.
 // Expected exit: 0
```

- Obligation line: `// Fact: a slice view (&mut [u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_slice_view_element_read_bound_through_call_result_exit_ref_to_mut_ref_74133dcf10d5/main.omg`
- hole_subtype: ``; probe: 

### 183. `tests/omega/pass/facts/slice_view_element_read_bound_through_target_body_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: a slice view (&[u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.
+// Fact: a slice view (&mut [u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.
 // Place: another target body. Each hosted target body computes and verifies the element read and extent bound.
 // Expected exit: 0
```

- Obligation line: `// Fact: a slice view (&mut [u64]) whose length is guarded (view.len >= 3u64) proves safe indexed element read at view[2u64] and preserves the length lower bound to discharge exact subtraction view.len - 3u64 across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_slice_view_element_read_bound_through_target_body_exit_ref_to_mut_ref_a570f4c07c6c/main.omg`
- hole_subtype: ``; probe: 

### 184. `tests/omega/pass/facts/slice_view_layout_geometry_through_parameter_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: call compiler operation call`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: querying size_of and align_of on a 64-bit slice view (&[u64]) establishes its exact two-word (ptr, len) descriptor geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.
+// Fact: querying size_of and align_of on a 64-bit slice view (&mut [u64]) establishes its exact two-word (ptr, len) descriptor geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.
 // Place: parameter. Helper machines receive the queried size and alignment directly as explicit parameters.
 // Expected exit: 0
```

- Obligation line: `// Fact: querying size_of and align_of on a 64-bit slice view (&mut [u64]) establishes its exact two-word (ptr, len) descriptor geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_slice_view_layout_geometry_through_parameter_exit_ref_to_mut_ref_b9cf2b9e272e/main.omg`
- hole_subtype: ``; probe: 

### 185. `tests/omega/pass/facts/static_evidence_premise_through_other_state_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -23,5 +23,4 @@
 
 machine project<Proof satisfies Kept>(kept: bool) -> bool
-requires kept == true
 requires Proof::holds(kept)
 {
```

- Obligation line: `machine project<Proof satisfies Kept>(kept: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_static_evidence_premise_through_other_state_exit_drop_machine_requires_ed5c879ceaab/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 186. `tests/omega/pass/facts/typed_reflection_mutation_through_const_name_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,4 @@
 data Subject [copy] { first: u64; second: u64; third: u64; }
 machine increment<Kind>(count: &mut u64, info: &reflect::FieldInfo, value: &mut Kind)
-requires Kind == u64
 {
     value = ((value as u64 in Wrapping) + (1u64 as u64 in Wrapping)) as u64;
```

- Obligation line: `data Subject [copy] { first: u64; second: u64; third: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_typed_reflection_mutation_through_const_name_exit_drop_machine_requires_cc0417ebd571/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 187. `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_field_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data with custody`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: 64-bit unsigned bitwise AND mask raw & 63u64 with bounded interval 40u64 <= masked <= 63u64 establishes 40u64 <= masked <= 63u64 across all nine places so exact subtractions 63u64 - masked and masked - 40u64 are safe.
+// Fact: 64-bit unsigned bitwise AND mask raw &mut  63u64 with bounded interval 40u64 <= masked <= 63u64 establishes 40u64 <= masked <= 63u64 across all nine places so exact subtractions 63u64 - masked and masked - 40u64 are safe.
 // Expected exit: 0
 
```

- Obligation line: `// Fact: 64-bit unsigned bitwise AND mask raw &mut  63u64 with bounded interval 40u64 <= masked <= 63u64 establishes 40u64 <= masked <= 63u64 across all nine places so exact subtractions 63u64 - masked and masked - 40u64 are safe.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_unsigned_64bit_bitwise_and_mask_bound_through_field_exit_ref_to_mut_ref_3e77f77b82c8/main.omg`
- hole_subtype: ``; probe: 

### 188. `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 
 machine compute_xor(raw: u64) -> u64
-    requires raw >= 59 && raw <= 59
 {
     raw ^ 15u64
```

- Obligation line: `machine compute_xor(raw: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_unsigned_64bit_bitwise_xor_bound_through_target_body_exitdrop_machine_requires_5a6148fa8fee/main.omg`
- hole_subtype: `unchecked`; probe: probe call `compute_xor(60)` omega_exit=0

### 189. `tests/omega/pass/facts/unsigned_8bit_shift_right_bound_through_other_machine_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: typing literal landed type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,4 @@
 
 machine compute_shift(val: u8) -> u8
-    requires val >= 160 && val <= 160
 {
     val >> 2u32
```

- Obligation line: `machine compute_shift(val: u8) -> u8`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_unsigned_8bit_shift_right_bound_through_other_machine_exidrop_machine_requires_65c0a54f6ea7/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `compute_shift(161)` omega_exit=1

### 190. `tests/omega/pass/facts/value_location_alignment_through_call_result_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.
+// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.
 // Place: call result. A helper returns the observed alignment of its borrowed referent; the caller subtracts.
 // Expected exit: 0
```

- Obligation line: `// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_value_location_alignment_through_call_result_exit_ref_to_mut_ref_e246570cd20a/main.omg`
- hole_subtype: ``; probe: 

### 191. `tests/omega/pass/facts/value_location_alignment_through_target_body_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.
+// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.
 // Place: another selected target implementation. Each Build-selected implementation observes a local u64 and subtracts.
 // Expected exit: 0
```

- Obligation line: `// Fact: align_of(value) observes the storage location of a live u64 place, the referent of a &mut u64, or the first element of a nonempty [u64] slice, and reports that location's guaranteed alignment, at least 8u64 on every hosted target, discharging exact subtraction alignment - 8u64 across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_value_location_alignment_through_target_body_exit_ref_to_mut_ref_224467a12c3d/main.omg`
- hole_subtype: ``; probe: 

### 192. `tests/omega/pass/facts/wrapping_integer_arithmetic_through_other_machine_exit/main.omg` (ref_to_mut_ref, accepted_by=both)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Masking an unsigned scalar with a constant bitmask (val & 63u64) establishes the upper bound (masked <= 63u64), discharging exact subtraction 63u64 - masked.
+// Masking an unsigned scalar with a constant bitmask (val &mut  63u64) establishes the upper bound (masked <= 63u64), discharging exact subtraction 63u64 - masked.
 // Expected exit: 0.
 machine forward(val: u64) -> u64 {
```

- Obligation line: `// Masking an unsigned scalar with a constant bitmask (val &mut  63u64) establishes the upper bound (masked <= 63u64), discharging exact subtraction 63u64 - masked.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_wrapping_integer_arithmetic_through_other_machine_exit_ref_to_mut_ref_b55ee1d16c4c/main.omg`
- hole_subtype: ``; probe: 

### 193. `tests/omega/pass/facts/write_only_borrow_initialization_through_match_join_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: a write-only borrow (&write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.
+// Fact: a write-only borrow (&mut write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.
 // Place: match join. The initialized slot value is unified across reconverging match arms before exact subtraction.
 // Expected exit: 0
```

- Obligation line: `// Fact: a write-only borrow (&mut write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_borrow_initialization_through_match_join_exit_ref_to_mut_ref_ba755bc19996/main.omg`
- hole_subtype: ``; probe: 

### 194. `tests/omega/pass/facts/write_only_borrow_struct_destination_through_local_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: Initializing a destination storage slot through a write-only borrow (&write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.
+// Fact: Initializing a destination storage slot through a write-only borrow (&mut write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.
 // Expected exit: 0
 
```

- Obligation line: `// Fact: Initializing a destination storage slot through a write-only borrow (&mut write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_borrow_struct_destination_through_local_exit_ref_to_mut_ref_cd5e94d3ec95/main.omg`
- hole_subtype: ``; probe: 

### 195. `tests/omega/pass/facts/write_only_ieee_store_readback_through_field_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: an IEEE f64 computed and stored through a write-only borrow (&write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places
+// Fact: an IEEE f64 computed and stored through a write-only borrow (&mut write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places
 // Place: field. The read-back value is carried through a record field before the comparison.
 // Expected exit: 0
```

- Obligation line: `// Fact: an IEEE f64 computed and stored through a write-only borrow (&mut write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_ieee_store_readback_through_field_exit_ref_to_mut_ref_33413fb19836/main.omg`
- hole_subtype: ``; probe: 

### 196. `tests/omega/pass/facts/write_only_match_join_through_call_result_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine choose(counter: &mut u64, flag: bool) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64, flag: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_match_join_through_call_result_exit_drop_machine_requires_2937cfefa6a9/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 197. `tests/omega/pass/facts/write_only_match_join_through_target_body_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: declaration Build selection that is not a machine or a data type`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine choose(counter: &mut u64, flag: bool) -> bool
-requires counter == 0
 {
     counter = 1;
```

- Obligation line: `machine choose(counter: &mut u64, flag: bool) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_match_join_through_target_body_exit_drop_machine_requires_ad4c1b4b4bcc/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 198. `tests/omega/pass/facts/write_only_reference_store_through_other_state_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Fact: a `&write` store into a record's reference leaf installs the incoming
+// Fact: a `&mut write` store into a record's reference leaf installs the incoming
 // descriptor — the stored target reads the new referent's 85 while the
 // displaced referent's 11 and the sibling stamp's 17 stay untouched.
```

- Obligation line: `// Fact: a `&mut write` store into a record's reference leaf installs the incoming`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_facts_write_only_reference_store_through_other_state_exit_ref_to_mut_ref_cd1a3c4c06fe/main.omg`
- hole_subtype: ``; probe: 

### 199. `tests/omega/pass/filesystem/windows_raw_metadata_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,5 @@
 // must classify it before entering a native query that excludes pipes.
 boundary trait FixturePipe {
-    machine create(read: &mut u64, write: &mut u64, security: u64, size: u32) -> i32
+    machine create(read: &u64, write: &mut u64, security: u64, size: u32) -> i32
     reaches
     requires security == 0 && size == 0
```

- Obligation line: `requires security == 0 && size == 0`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_filesystem_windows_raw_metadata_exit_mut_ref_to_ref_426f3641d382/main.omg`
- hole_subtype: ``; probe: 

### 200. `tests/omega/pass/float/float_copied_argument_completion/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine supply mode other than a checked body`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // Frozen review control: the first f32 actual keeps its captured value when the second actual clears its old storage.
 // Expected: source checking succeeds; probe is not executed.
-boundary machine clear(slot: &mut f32) -> u64
+boundary machine clear(slot: &f32) -> u64
 ensures slot == 0.0;
 machine forward(value: f32, ignored: u64) -> f32
```

- Obligation line: `boundary machine clear(slot: &f32) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_float_float_copied_argument_completion_mut_ref_to_ref_0e52f18745eb/main.omg`
- hole_subtype: ``; probe: 

### 201. `tests/omega/pass/float/guarded_scalar_state_contract_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,4 @@
     }
     machine bounded(value: f64) -> i64
-    requires value == value && value >= -100.0 && value <= 100.0
     {
         let converted: i64 = I64::from_f64(value);
```

- Obligation line: `machine bounded_integer(value: f64) -> i64 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_float_guarded_scalar_state_contract_exit_drop_machine_requires_261039195d7e/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `bounded(101)` omega_exit=1

### 202. `tests/omega/pass/float/guarded_scalar_state_contract_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type float`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -25,5 +25,5 @@
     }
     machine bounded(accepted: bool, value: f32) -> f32
-    requires accepted && value != 0.0f32 && value < 10.0f32 && value > -10.0f32
+    requires accepted && value != 0.0f32 && value <= 10.0f32 && value > -10.0f32
     { transition { _ -> (value) } }
     machine rejected() -> f32 { transition { _ -> (0.0f32) } }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_float_guarded_scalar_state_contract_exit_widen_requires_lt_f1a889db4550/main.omg`
- hole_subtype: ``; probe: 

### 203. `tests/omega/pass/float/named_float_integer_lower_boundary_conversion_traps/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let converted: i32 in Trapping = I32::from_f64(-2147483649.0);
     Console::exit_process(converted as i32);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_float_named_float_integer_lower_boundary_conversion_traps_mut_ref_to_ref_fd59fcc146c3/main.omg`
- hole_subtype: ``; probe: 

### 204. `tests/omega/pass/float/named_nan_semantic_only_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     transition CLASSIFIED {
         true -> good()
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_float_named_nan_semantic_only_exit_mut_ref_to_ref_5a4cc3b5c432/main.omg`
- hole_subtype: ``; probe: 

### 205. `tests/omega/pass/formatting/runtime_repeated_wrapping_decimal_format_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
     Console::write_byte(32);
 }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     number(1234);
     Console::exit_process(0);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_formatting_runtime_repeated_wrapping_decimal_format_exit_mut_ref_to_ref_d4bb9833b819/main.omg`
- hole_subtype: ``; probe: 

### 206. `tests/omega/pass/generics/inferred_equation_outer_context_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,4 @@
 // Expected exit: 0
 machine capacity<Length [copy], const Capacity: u64>(items: Length) -> u64
-requires Length == [u8; Capacity]
 { Capacity }
 machine forward<Value [copy]>(value: Value) -> Value { value }
```

- Obligation line: `machine capacity<Length [copy], const Capacity: u64>(items: Length) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_generics_inferred_equation_outer_context_exit_drop_machine_requires_d2d53c3ab933/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 207. `tests/omega/pass/generics/runtime_qualified_closed_equation_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,4 @@
 
 machine capacity<Payload>() -> u64
-requires Payload == models::Token
 { 7 }
 
```

- Obligation line: `data Projected<Anchor [copy], Token [copy]> [copy]`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_generics_runtime_qualified_closed_equation_exit_drop_machine_requires_b692fa0b0e29/main.omg`
- hole_subtype: `probe-not-built`; probe: could not locate machine for probe

### 208. `tests/omega/pass/host/runtime_console_bounded_line_partial_eof_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -34,5 +34,4 @@
     }
     machine report(main: &mut Main, reason: i32, count: u64)
-    requires 0 <= reason && reason <= 100
     {
         Console::write(&main.line);
```

- Obligation line: `ByteRead::Byte { value } -> suffix(main, reason, count, value)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_host_runtime_console_bounded_line_partial_eof_exit_drop_machine_requires_e0185a4b1218/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `report(reason, 0, 0)` omega_exit=1

### 209. `tests/omega/pass/host/runtime_descriptor_case_atomic_input_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 public data Choice { case Cell(counter: AtomicU32); case Empty; }
 public boundary trait DescriptorIo {
-    machine read(descriptor: i32, choice: &mut Choice, count: u64) -> i64 reaches blocks;
+    machine read(descriptor: i32, choice: &Choice, count: u64) -> i64 reaches blocks;
     machine write(descriptor: i32, choice: &mut Choice, count: u64) -> i64 reaches;
     machine write_tag(descriptor: i32, choice: &Choice, count: u64) -> i64 reaches;
```

- Obligation line: `machine read(descriptor: i32, choice: &Choice, count: u64) -> i64 reaches blocks;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_host_runtime_descriptor_case_atomic_input_exit_mut_ref_to_ref_bbddbaa6b656/main.omg`
- hole_subtype: ``; probe: 

### 210. `tests/omega/pass/host/runtime_descriptor_scalar_input_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 
 public boundary trait DescriptorIo {
-    machine read_word(descriptor: i32, word: &mut u32, count: u64) -> i64 reaches blocks;
+    machine read_word(descriptor: i32, word: &u32, count: u64) -> i64 reaches blocks;
     machine read_half(descriptor: i32, half: &mut u16, count: u64) -> i64 reaches blocks;
     machine write_word(descriptor: i32, word: &u32, count: u64) -> i64 reaches;
```

- Obligation line: `main.word = 1144201745;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_host_runtime_descriptor_scalar_input_exit_mut_ref_to_ref_d672b46b2329/main.omg`
- hole_subtype: ``; probe: 

### 211. `tests/omega/pass/host/scalar_reference_mixed_index_backing_foreign_export/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -27,5 +27,5 @@
 public boundary trait Compare { machine compare(left: &i64, right: &i64, count: u64) -> i32 reaches; }
 data Main requires index < 2 { bytes: [u8; 2]; index: u64; }
-machine Main::main(main: &mut Main) reaches Compare {
+machine Main::main(main: &Main) reaches Compare {
     let view: &RawView = &main.bytes[0] as &RawView;
     let narrow: &i64 = &view.value.value;
```

- Obligation line: `entries[0] = FieldEntry {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_host_scalar_reference_mixed_index_backing_foreign_export_mut_ref_to_ref_90848b1c8bb6/main.omg`
- hole_subtype: ``; probe: 

### 212. `tests/omega/pass/inline_asm/asm_port_immediate/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) reaches PortIo {
+machine Main::main(main: &Main) reaches PortIo {
     asm clobbers r10, r11, r15, rax, rdx {
         out 0x3F8, 0x41
```

- Obligation line: `machine Main::main(main: &Main) reaches PortIo {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_inline_asm_asm_port_immediate_mut_ref_to_ref_99f0efb6fead/main.omg`
- hole_subtype: ``; probe: 

### 213. `tests/omega/pass/inline_asm/asm_state_jmp/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     asm {
         jmp other()
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_inline_asm_asm_state_jmp_mut_ref_to_ref_353ea401325e/main.omg`
- hole_subtype: ``; probe: 

### 214. `tests/omega/pass/layouts/placed_reference_results/two_live/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -41,5 +41,4 @@
 
 machine require_seventeen(value: u64)
-requires value == 17
 {}
 
```

- Obligation line: `machine borrow_view(value: &mut Value) -> &mut Access<StablePlacement, Value> {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_layouts_placed_reference_results_two_live_drop_machine_requires_c5097bcc0b62/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 215. `tests/omega/pass/layouts/runtime_size_of_align_of_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,5 @@
 //   Empty      -- validated zero-sized layout: size 0, align 1; distinct
 //                 from the unresolved-array fallback that shares (0, 1)
-//   &Row       -- reference storage (8/8 on every bound 64-bit target), not
+//   &mut Row       -- reference storage (8/8 on every bound 64-bit target), not
 //                 the referent's 16
 //   LayoutStorage<PackedFlags, CompactBits> -- the installed plan places three logical
```

- Obligation line: `//   &mut Row       -- reference storage (8/8 on every bound 64-bit target), not`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_layouts_runtime_size_of_align_of_exit_ref_to_mut_ref_500065be267f/main.omg`
- hole_subtype: ``; probe: 

### 216. `tests/omega/pass/machines/machine_param_data/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -17,3 +17,3 @@
 machine accepts_concrete_stream(value: Stream<sample>) { }
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_machines_machine_param_data_mut_ref_to_ref_cfedad5028fd/main.omg`
- hole_subtype: ``; probe: 

### 217. `tests/omega/pass/modules/continued_domain_membership/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract contracts on a state`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 domain [u8]::Ascii requires ascii_only(self);
 
-machine preserve(bytes: &mut [u8; 4] in Ascii)
+machine preserve(bytes: &[u8; 4] in Ascii)
 ensures bytes in Ascii
 {
```

- Obligation line: `let mut bytes: [u8; 4] = [65u8, 0u8, 0u8, 0u8];`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_modules_continued_domain_membership_mut_ref_to_ref_25b09a30a3b4/main.omg`
- hole_subtype: ``; probe: 

### 218. `tests/omega/pass/modules/loaded_sources_expose_nothing/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
 // `Float::subtract` is loaded.
 trait Equatable<T> {
-    machine equals(left: &T, right: &T) -> bool;
+    machine equals(left: &mut T, right: &T) -> bool;
 }
 
```

- Obligation line: `machine equals(left: &mut T, right: &T) -> bool;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_modules_loaded_sources_expose_nothing_ref_to_mut_ref_164584fe54af/main.omg`
- hole_subtype: ``; probe: 

### 219. `tests/omega/pass/modules/nested_constant_projections_process_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches ProcessExit {
+machine Main::main(main: &Main) reaches ProcessExit {
     ProcessExit::exit_process(STATUS);
 }
```

- Obligation line: `machine Main::main(main: &Main) reaches ProcessExit {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_modules_nested_constant_projections_process_exit_mut_ref_to_ref_d433f3c73724/main.omg`
- hole_subtype: ``; probe: 

### 220. `tests/omega/pass/modules/same_name_record_field_reads_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -13,5 +13,5 @@
     let word: u64 = wide.value;
     transition byte == 200 && word == 300
-        && right::Holder::read(&narrow) == 200 && left::Holder::read(&wide) == 300 {
+        && right::Holder::read(&mut narrow) == 200 && left::Holder::read(&wide) == 300 {
         true -> done()
         false -> wrong()
```

- Obligation line: `&& right::Holder::read(&mut narrow) == 200 && left::Holder::read(&wide) == 300 {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_modules_same_name_record_field_reads_exit_ref_to_mut_ref_eed199e584f9/main.omg`
- hole_subtype: ``; probe: 

### 221. `tests/omega/pass/operators/slice_index_bounds_from_operator_contract/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 }
 
-machine inspect(main: &mut Main) {
+machine inspect(main: &Main) {
     let first: i32 = main.values[0];
 }
```

- Obligation line: `machine inspect(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_operators_slice_index_bounds_from_operator_contract_mut_ref_to_ref_6b72a1eef8ab/main.omg`
- hole_subtype: ``; probe: 

### 222. `tests/omega/pass/ownership/linear_transparent_record_frontier/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -27,5 +27,5 @@
 data Main { }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let left: Receipt = Receipt { code: 1 };
     let right: Receipt = Receipt { code: 2 };
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ownership_linear_transparent_record_frontier_mut_ref_to_ref_6fb0e60632bb/main.omg`
- hole_subtype: ``; probe: 

### 223. `tests/omega/pass/ownership/param_member_write_compile/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A `&mut` parameter's members write directly: `retag(&mut main.row)`
+// A `&mut mut` parameter's members write directly: `retag(&mut main.row)`
 // lends the record, and `row.tag = 5` assigns through the parameter —
 // exclusive access to the loaned record covers its members.
```

- Obligation line: `// A `&mut mut` parameter's members write directly: `retag(&mut main.row)``
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ownership_param_member_write_compile_ref_to_mut_ref_88347ec98b49/main.omg`
- hole_subtype: ``; probe: 

### 224. `tests/omega/pass/ownership/param_member_write_compile/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A `&mut` parameter's members write directly: `retag(&mut main.row)`
+// A `&mut` parameter's members write directly: `retag(&main.row)`
 // lends the record, and `row.tag = 5` assigns through the parameter —
 // exclusive access to the loaned record covers its members.
```

- Obligation line: `row.tag = 5;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ownership_param_member_write_compile_mut_ref_to_ref_62ec696e48a8/main.omg`
- hole_subtype: ``; probe: 

### 225. `tests/omega/pass/ownership/runtime_qualified_inventory_selection_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -17,5 +17,4 @@
     }
     machine store(slot: &mut Slot) -> bool
-    requires slot in Slot::Empty
     {
         slot = Slot::Held { token: issue(73) };
```

- Obligation line: `data Token [linear] { value: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ownership_runtime_qualified_inventory_selection_exit_drop_machine_requires_8fa514b6fde4/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 226. `tests/omega/pass/proofs/kernel_theorem_equality_certificates/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -33,5 +33,5 @@
 }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     equal_symmetric(3, 3);
     equal_transitive(3, 3, 3);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_proofs_kernel_theorem_equality_certificates_mut_ref_to_ref_26854cfedbb2/main.omg`
- hole_subtype: ``; probe: 

### 227. `tests/omega/pass/proofs/runtime_indexed_field_relations_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -7,5 +7,5 @@
     }
     machine capacity(counters: &mut [Counter], selected: u64) -> u64
-    requires selected < counters.len {
+    requires selected <= counters.len {
         transition counters[selected].value < counters[selected].limit {
             true -> store(counters, selected)
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_proofs_runtime_indexed_field_relations_exit_widen_requires_lt_29c37cbb2ccf/main.omg`
- hole_subtype: ``; probe: 

### 228. `tests/omega/pass/proofs/runtime_static_result_projection_citation_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -48,5 +48,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     concrete_projection(Nat::Zero);
     let code: i32 = verified_exit(70);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_proofs_runtime_static_result_projection_citation_exit_mut_ref_to_ref_1680efe46cd2/main.omg`
- hole_subtype: ``; probe: 

### 229. `tests/omega/pass/providers/borrowed_service_field_forward_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,5 @@
 }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let value: i32 = 82;
     report(value);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_borrowed_service_field_forward_exit_mut_ref_to_ref_7f05855d4c0d/main.omg`
- hole_subtype: ``; probe: 

### 230. `tests/omega/pass/providers/boundary_multi_effects/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -18,5 +18,5 @@
 data Client {
 }
-machine Client::run(client: &mut Client) reaches Net + Allocator {
+machine Client::run(client: &Client) reaches Net + Allocator {
     Net::write_line("payload");
 }
```

- Obligation line: `machine Client::run(client: &Client) reaches Net + Allocator {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_boundary_multi_effects_mut_ref_to_ref_f9d7a2ecac60/main.omg`
- hole_subtype: ``; probe: 

### 231. `tests/omega/pass/providers/closed_callback_repeated_application/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -224,5 +224,5 @@
 }
 public boundary trait SortRegistrar: Calling<SortCalling> {
-    machine sort<Unused, machine Handler>(elements: &mut Elements, count: u64, width: u64, native callback compare from Handler)
+    machine sort<Unused, machine Handler>(elements: &Elements, count: u64, width: u64, native callback compare from Handler)
     requires machine Handler satisfies Comparator::compare;
     reaches
```

- Obligation line: `entries[0] = FieldEntry {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_closed_callback_repeated_application_mut_ref_to_ref_6fa862084e3c/main.omg`
- hole_subtype: ``; probe: 

### 232. `tests/omega/pass/providers/external_boundary_requirement_via_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -22,5 +22,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main)
+machine Main::main(main: &Main)
 {
     let magnitude: i32 = ForeignMath::absolute(21);
```

- Obligation line: `machine Main::main(main: &Main)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_external_boundary_requirement_via_exit_mut_ref_to_ref_4a73c3354f62/main.omg`
- hole_subtype: ``; probe: 

### 233. `tests/omega/pass/providers/local_record_foreign_borrow_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Compare + Console {
+machine Main::main(main: &Main) reaches Compare + Console {
     let second: i64 = second_value(69);
     let expected: Pair = Pair { first: 69, second: second };
```

- Obligation line: `machine second_value(first: i64) -> i64 requires first >= 0 && first <= 69 ensures result >= 1 && result <= 70 { first + 1 }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_local_record_foreign_borrow_exit_mut_ref_to_ref_0c4d6f7dd9af/main.omg`
- hole_subtype: ``; probe: 

### 234. `tests/omega/pass/providers/runtime_boundary_linear_borrowed_argument_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A boundary-trait requirement lends a linear value: `weigh(token: &Token)`
+// A boundary-trait requirement lends a linear value: `weigh(token: &mut Token)`
 // and `tag(token: &Token, marker: u64)` take a shared loan of a `[linear]`
 // record. The requirement's checked plan classifies that formal through the
```

- Obligation line: `// A boundary-trait requirement lends a linear value: `weigh(token: &mut Token)``
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_runtime_boundary_linear_borrowed_argument_exit_ref_to_mut_ref_34ca2299e230/main.omg`
- hole_subtype: ``; probe: 

### 235. `tests/omega/pass/providers/target_family_entry/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,3 +12,3 @@
 
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_providers_target_family_entry_mut_ref_to_ref_ee47b49bf7a2/main.omg`
- hole_subtype: ``; probe: 

### 236. `tests/omega/pass/ranges/subject_unsigned_computed_index_lower_bound/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 // needed, because a `u64` sum cannot be negative. The temp takes its carrier
 // from the first place operand, so the operand must resolve -- through a
-// `&mut` subject exactly as it resolved through a `&mut self` receiver.
+// `&mut mut` subject exactly as it resolved through a `&mut self` receiver.
 //
 // The upper bound is the authored guard; only the lower half rides the type.
```

- Obligation line: `// `&mut mut` subject exactly as it resolved through a `&mut self` receiver.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ranges_subject_unsigned_computed_index_lower_bound_ref_to_mut_ref_a695c2a9c2c1/main.omg`
- hole_subtype: ``; probe: 

### 237. `tests/omega/pass/ranges/subject_unsigned_computed_index_lower_bound/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 // needed, because a `u64` sum cannot be negative. The temp takes its carrier
 // from the first place operand, so the operand must resolve -- through a
-// `&mut` subject exactly as it resolved through a `&mut self` receiver.
+// `&mut` subject exactly as it resolved through a `&self` receiver.
 //
 // The upper bound is the authored guard; only the lower half rides the type.
```

- Obligation line: `main.wh = 1;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_ranges_subject_unsigned_computed_index_lower_bound_mut_ref_to_ref_0638b1420b45/main.omg`
- hole_subtype: ``; probe: 

### 238. `tests/omega/pass/recast/float32_bits_view_compile/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Equal-width float recast works at f32 width too: `&rate as &u32` views
+// Equal-width float recast works at f32 width too: `&mut rate as &u32` views
 // an f32 holding 2.0 as 0x40000000 — the raw IEEE-754 pattern of the
 // single-precision value.
```

- Obligation line: `// Equal-width float recast works at f32 width too: `&mut rate as &u32` views`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_recast_float32_bits_view_compile_ref_to_mut_ref_07a37615347b/main.omg`
- hole_subtype: ``; probe: 

### 239. `tests/omega/pass/recast/mutable_float32_write_compile/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A mutable recast writes f32 bits through a u32 view: `&mut main.raw as
+// A mutable recast writes f32 bits through a u32 view: `&main.raw as
 // &mut f32` stores 3.25's IEEE-754 single pattern — read back as the
 // integer, it is 0x40500000 = 1079197696.
```

- Obligation line: `main.raw = 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_recast_mutable_float32_write_compile_mut_ref_to_ref_4143d8d98a98/main.omg`
- hole_subtype: ``; probe: 

### 240. `tests/omega/pass/recast/signed64_unsigned_view_compile/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// A signed 64-bit value re-views as unsigned bits: `&main.span as &u64`
+// A signed 64-bit value re-views as unsigned bits: `&mut main.span as &u64`
 // exposes the i64 storage's raw interpretation — `-1` reads as
 // 18446744073709551615, the wide-pair direction of the sign recast.
```

- Obligation line: `// A signed 64-bit value re-views as unsigned bits: `&mut main.span as &u64``
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_recast_signed64_unsigned_view_compile_ref_to_mut_ref_38f9a9e7d40f/main.omg`
- hole_subtype: ``; probe: 

### 241. `tests/omega/pass/recast/write_footprint/call_result/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,4 @@
 
 machine require_seventeen(value: u64)
-requires value == 17
 {}
 
```

- Obligation line: `machine borrow_view(value: &mut Value) -> &mut Alternate {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_recast_write_footprint_call_result_drop_machine_requires_42eb84f29d3a/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 242. `tests/omega/pass/relevance/erased_receiver_proof_machine_call/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -4,5 +4,5 @@
 // classification), `P::touch` takes its subject BY VALUE and is therefore a
 // proof machine, and the erased local `p` legally supplies that proof term.
-// By value is load-bearing: a `&P` subject would be a runtime consumption
+// By value is load-bearing: a `&mut P` subject would be a runtime consumption
 // attempt, since proof-only data has no storage to borrow.
 // Checked-only: proof machines emit no runtime code (contracts.md).
```

- Obligation line: `// By value is load-bearing: a `&mut P` subject would be a runtime consumption`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_relevance_erased_receiver_proof_machine_call_ref_to_mut_ref_bb0775c111c8/main.omg`
- hole_subtype: ``; probe: 

### 243. `tests/omega/pass/relevance/erased_state_param/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -14,5 +14,5 @@
     }
 }
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     Main::record(3, 10);
 }
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_relevance_erased_state_param_mut_ref_to_ref_564196bab244/main.omg`
- hole_subtype: ``; probe: 

### 244. `tests/omega/pass/slices/fused_entry_slice_call/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // A `[u64; 3]` field on the entry receiver coerces to a checked callee's
-// `&mut [u64]` through the borrowed-view presentation; the Fused entry
+// `&[u64]` through the borrowed-view presentation; the Fused entry
 // receiver keeps its Terminal attachment while it lends the field.
 
```

- Obligation line: `// `&[u64]` through the borrowed-view presentation; the Fused entry`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_slices_fused_entry_slice_call_mut_ref_to_ref_51d4135b5ad4/main.omg`
- hole_subtype: ``; probe: 

### 245. `tests/omega/pass/slices/inclusive_subslice_bounds/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -9,5 +9,5 @@
 }
 
-machine Main::main(main: &mut Main) -> u64 {
+machine Main::main(main: &Main) -> u64 {
     let view: &[Entry] = &main.entries[..];
     let inclusive_prefix: &[Entry] = view[0..=2];
```

- Obligation line: `let view: &[Entry] = &main.entries[..];`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_slices_inclusive_subslice_bounds_mut_ref_to_ref_db462354888f/main.omg`
- hole_subtype: ``; probe: 

### 246. `tests/omega/pass/slices/projected_slice_length/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -20,3 +20,3 @@
 
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_slices_projected_slice_length_mut_ref_to_ref_6335b3c50cce/main.omg`
- hole_subtype: ``; probe: 

### 247. `tests/omega/pass/slices/runtime_explicit_mutable_range_local_forwarding_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// `&mut items[1..3]` spells the mutable subrange explicitly
+// `&items[1..3]` spells the mutable subrange explicitly
 // (`bare-range-reference-access`). The resulting `&mut [i32]` binding then
 // forwards to a call and to a state under ordinary rules: `fill(window)` and
```

- Obligation line: `items[0] = 23;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_slices_runtime_explicit_mutable_range_local_forwarding_exit_mut_ref_to_ref_91e7fcb66dca/main.omg`
- hole_subtype: ``; probe: 

### 248. `tests/omega/pass/slices/saved_byte_predicate_alias_disjoint_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 // Expected exit: 0
 // Place: local.
-machine inspect(bytes: &[u8]) -> u8
+machine inspect(bytes: &mut [u8]) -> u8
 requires ascii_only(bytes) && bytes.len >= 1
 { bytes[0] }
```

- Obligation line: `machine inspect(bytes: &mut [u8]) -> u8`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_slices_saved_byte_predicate_alias_disjoint_exit_ref_to_mut_ref_7f063a27ef76/main.omg`
- hole_subtype: ``; probe: 

### 249. `tests/omega/pass/storage/forwarded_context_bounded_byte_field_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -11,5 +11,5 @@
 // Two one-line variants bound it. Spell the same field `text: [u8; 5]`,
 // without the `in Utf8` domain, and the program compiled. Keep the domain and
-// give every state `&mut self` instead of `main: &mut Main`, and it compiled.
+// give every state `&self` instead of `main: &mut Main`, and it compiled.
 // Neither the domain nor the forwarding was the fault on its own.
 //
```

- Obligation line: `main.i = 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_storage_forwarded_context_bounded_byte_field_exit_mut_ref_to_ref_1dd3620a3544/main.omg`
- hole_subtype: ``; probe: 

### 250. `tests/omega/pass/structs/copied_array_field_shared_record/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -6,5 +6,5 @@
 }
 machine first(info: &Info) -> i32 { info.name[0] as i32 }
-machine Main::main(context: &mut Main) reaches Console {
+machine Main::main(context: &Main) reaches Console {
     let mut info: Info;
     info.name[0] = 70;
```

- Obligation line: `info.name[0] = 70;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_structs_copied_array_field_shared_record_mut_ref_to_ref_83888133114e/main.omg`
- hole_subtype: ``; probe: 

### 251. `tests/omega/pass/targets/efi_vtable_call/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -5,5 +5,5 @@
 //
 // The full chain, all Omega: build.omg sets efi_application+freestanding; the
-// boundary entry receives the SystemTable (RDX) as a vouched &EfiSystemTable;
+// boundary entry receives the SystemTable (RDX) as a vouched &mut EfiSystemTable;
 // `table.con_out` DEREFERENCES it at the plan offset (+64); the external leaf
 // binds output_string via its named protocol field, which lowers to `mov rax,[rcx+8];
```

- Obligation line: `// boundary entry receives the SystemTable (RDX) as a vouched &mut EfiSystemTable;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_targets_efi_vtable_call_ref_to_mut_ref_ade1519f94b8/main.omg`
- hole_subtype: ``; probe: 

### 252. `tests/omega/pass/targets/target_member_ranked_self_transition_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -28,5 +28,5 @@
 
 data Main { }
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     let left: u64 = drain(5, 3);
     transition left == 2 {
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_targets_target_member_ranked_self_transition_exit_mut_ref_to_ref_705123eff83e/main.omg`
- hole_subtype: ``; probe: 

### 253. `tests/omega/pass/termination/identity_measure_widths/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -23,3 +23,3 @@
 
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_identity_measure_widths_mut_ref_to_ref_332f689b4fc2/main.omg`
- hole_subtype: ``; probe: 

### 254. `tests/omega/pass/termination/measure_param_endpoint/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -23,3 +23,3 @@
 
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_measure_param_endpoint_mut_ref_to_ref_b56d2920730b/main.omg`
- hole_subtype: ``; probe: 

### 255. `tests/omega/pass/termination/nested_pair_endpoint/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -22,3 +22,3 @@
 
 data Main {}
-machine Main::main(main: &mut Main) { }
+machine Main::main(main: &Main) { }
```

- Obligation line: `machine Main::main(main: &Main) { }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_nested_pair_endpoint_mut_ref_to_ref_547353186a79/main.omg`
- hole_subtype: ``; probe: 

### 256. `tests/omega/pass/termination/progress_selector_owned_arrival_mixed_ingress_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_progress_selector_owned_arrival_mixed_ingress_exit_drop_machine_requires_d5026f301998/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 257. `tests/omega/pass/termination/progress_selector_reference_snapshot_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -10,5 +10,4 @@
 }
 machine consume(value: &Token, calls: &mut u64) -> u64
-requires value in Token::Fair
 terminates
 {
```

- Obligation line: `public data Token [copy] { identity: u64; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_progress_selector_reference_snapshot_exit_drop_machine_requires_af4e1382fae8/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 258. `tests/omega/pass/termination/rank_endpoint_borrows_recovery/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // Recovered from the deleted `rank_endpoint_borrows` case (c60792d7bdd):
 // a countdown whose endpoint bound is read through a stored EXCLUSIVE
-// reference. `indirect.target` is a `&mut Wrap` carried inside `Indirect`
+// reference. `indirect.target` is a `&Wrap` carried inside `Indirect`
 // through every recursive call, and the projected `indirect.target.remaining`
 // forms the inclusive upper bound of `n -> Nat::Descending`.
```

- Obligation line: `// reference. `indirect.target` is a `&Wrap` carried inside `Indirect``
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_rank_endpoint_borrows_recovery_mut_ref_to_ref_35735a8ca467/main.omg`
- hole_subtype: ``; probe: 

### 259. `tests/omega/pass/termination/runtime_measure_input_requirement_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type data with custody`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -2,5 +2,4 @@
 // The view forms its helper call for every admitted input, including zero.
 machine bounded_rank(value: u64) -> u64
-requires value >= 0 && value <= 5
 ensures result == value
 { value }
```

- Obligation line: `machine bounded_rank(value: u64) -> u64`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_runtime_measure_input_requirement_exit_drop_machine_requires_0ff7bd63e5f3/main.omg`
- hole_subtype: `unchecked`; probe: probe call `bounded_rank(6)` omega_exit=0

### 260. `tests/omega/pass/termination/runtime_rank_range_state_call_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -30,5 +30,4 @@
     }
     machine hold(main: &mut Main, pending: u32) -> u32
-    requires pending <= 9
     {
         transition pending > 0 {
```

- Obligation line: `machine hold(main: &mut Main, pending: u32) -> u32`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_runtime_rank_range_state_call_exit_drop_machine_requires_e88fab7f9049/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `hold(10, 0)` omega_exit=1

### 261. `tests/omega/pass/termination/runtime_self_call_forwarding_state_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: contract term other than a formal or literal`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -42,5 +42,5 @@
     }
     machine forward(count: u64) -> u64
-    requires count < 4
+    requires count <= 4
     {
         transition { _ -> walk_arrival(count) }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_termination_runtime_self_call_forwarding_state_exit_widen_requires_lt_546e149b2ad8/main.omg`
- hole_subtype: ``; probe: 

### 262. `tests/omega/pass/text/runtime_param_domain_forward_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
     valid_utf8(self);
 // #66 param entry-assumption: a state parameter declared with an encoding
-// domain (`text: &[u8] in Utf8`) carries that domain through the state body,
+// domain (`text: &mut [u8] in Utf8`) carries that domain through the state body,
 // because its implicit `requires param in Domain` makes every caller prove
 // membership. FORWARDING the param to another domained param must discharge
```

- Obligation line: `// domain (`text: &mut [u8] in Utf8`) carries that domain through the state body,`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_text_runtime_param_domain_forward_exit_ref_to_mut_ref_fca38262da03/main.omg`
- hole_subtype: ``; probe: 

### 263. `tests/omega/pass/time/runtime_clock_padded_out_parameter_prefix_exit/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -3,5 +3,5 @@
 public data Instant { seconds: i64; nanoseconds: i64; marker: u8; }
 public boundary trait ClockProbe {
-    machine read(selector: u32, instant: &mut Instant) -> i32 reaches;
+    machine read(selector: u32, instant: &Instant) -> i32 reaches;
 }
 data Holder { before: i64; instant: Instant; after: i64; }
```

- Obligation line: `main.holder.before = 17;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_time_runtime_clock_padded_out_parameter_prefix_exit_mut_ref_to_ref_ec21ed1496a2/main.omg`
- hole_subtype: ``; probe: 

### 264. `tests/omega/pass/traits/boundary_trait_effects_host_call/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -28,5 +28,5 @@
 }
 
-machine Main::main(main: &mut Main) reaches Console {
+machine Main::main(main: &Main) reaches Console {
     Console::write_line("boundary reach survives syntax");
     Console::exit_process(0);
```

- Obligation line: `machine Main::main(main: &Main) reaches Console {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_traits_boundary_trait_effects_host_call_mut_ref_to_ref_23e40858f446/main.omg`
- hole_subtype: ``; probe: 

### 265. `tests/omega/pass/traits/operator_machine_add/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint ``)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -18,5 +18,5 @@
 data Main {}
 
-machine Main::main(main: &mut Main) {
+machine Main::main(main: &Main) {
     let left: Wrapped = Wrapped { value: 250u8 };
     let right: Wrapped = Wrapped { value: 10u8 };
```

- Obligation line: `machine Main::main(main: &Main) {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_traits_operator_machine_add_mut_ref_to_ref_e838151005db/main.omg`
- hole_subtype: ``; probe: 

### 266. `tests/omega/pass/traits/runtime_selected_slice_reference_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -27,5 +27,5 @@
     machine inspect(values: &'input [i32], threshold: i32, position: u64)
         -> Optional<&'input i32>
-    requires position < values.len {
+    requires position <= values.len {
         transition Compare(&values[position], threshold) {
             true -> Optional::Some { value: &values[position] }
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_traits_runtime_selected_slice_reference_exit_widen_requires_lt_091234ee0ab6/main.omg`
- hole_subtype: ``; probe: 

### 267. `tests/omega/pass/wire/library_compact_binary_reflection_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -60,5 +60,4 @@
     }
     machine compare(bytes: &[u8], expected: &[u8], index: u64, remaining_steps: u64) -> bool
-    requires index < bytes.len && index < expected.len && remaining_steps > 1u64
     {
         transition bytes[index] == expected[index] {
```

- Obligation line: `machine same(bytes: &[u8], expected: &[u8], index: u64, remaining_steps: u64) -> bool`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_wire_library_compact_binary_reflection_exit_drop_machine_requires_3d7740f2dff2/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `compare(bytes, 0, 0, 0)` omega_exit=1

### 268. `tests/omega/pass/wire/library_compact_binary_reflection_exit/main.omg` (widen_requires_lt, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/state_contracts.md` › Requirement clauses)

> Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type reference`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -60,5 +60,5 @@
     }
     machine compare(bytes: &[u8], expected: &[u8], index: u64, remaining_steps: u64) -> bool
-    requires index < bytes.len && index < expected.len && remaining_steps > 1u64
+    requires index <= bytes.len && index < expected.len && remaining_steps > 1u64
     {
         transition bytes[index] == expected[index] {
```

- Obligation line: `(widened requires clause)`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_wire_library_compact_binary_reflection_exit_widen_requires_lt_525021be6ee9/main.omg`
- hole_subtype: ``; probe: 

### 269. `tests/omega/pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -95,5 +95,4 @@
     transition fields.len == 1u64 { true -> create(fields) false -> DecodeResult::Invalid }
     machine create(fields: &[ScalarSlot]) -> DecodeResult<ScalarValue>
-    requires fields.len == 1u64
     { DecodeResult::Sound { value: ScalarValue { value: fields[0u64].unsigned as u64 } } }
 }
```

- Obligation line: `machine ScalarCodec::construct(bytes: &[u8], fields: &[ScalarSlot]) -> DecodeResult<ScalarValue> {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_wire_runtime_wire_decode_rejects_noncanonical_varint_exit_drop_machine_requires_fd36a561cfda/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 270. `tests/omega/pass/wire/runtime_wire_signed_extremes_roundtrip_exit/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine attached data`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -35,5 +35,5 @@
     let read: u64 = 0u64;
     let verdict: ExtremesVerdict = ExtremesVerdict::Invalid;
-    ExtremesCodec::encode(&value, &mut buffer, &mut written);
+    ExtremesCodec::encode(&mut value, &mut buffer, &mut written);
     ExtremesCodec::decode(&mut decoded, &buffer, &mut read, &mut verdict);
 
```

- Obligation line: `ExtremesCodec::encode(&mut value, &mut buffer, &mut written);`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_pass_wire_runtime_wire_signed_extremes_roundtrip_exit_ref_to_mut_ref_42ec54496115/main.omg`
- hole_subtype: ``; probe: 

### 271. `tests/omega/run/borrows/mut_record_loan_member_access/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Run-tier witness — exclusive `&mut`-record loan member access.
+// Run-tier witness — exclusive `&mut mut`-record loan member access.
 // `let m: &mut Desc = &mut main.desc` then `m.tag = v` writes and `m.tag`
 // reads resolve through the captured established place plus the referee's
```

- Obligation line: `// Run-tier witness — exclusive `&mut mut`-record loan member access.`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_borrows_mut_record_loan_member_access_ref_to_mut_ref_83e05e7befee/main.omg`
- hole_subtype: ``; probe: 

### 272. `tests/omega/run/borrows/mut_record_loan_member_access/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // Run-tier witness — exclusive `&mut`-record loan member access.
-// `let m: &mut Desc = &mut main.desc` then `m.tag = v` writes and `m.tag`
+// `let m: &Desc = &mut main.desc` then `m.tag = v` writes and `m.tag`
 // reads resolve through the captured established place plus the referee's
 // structural declaration, the mutable sibling of the shared-loan arm.
```

- Obligation line: `let m: &mut Desc = &mut main.desc;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_borrows_mut_record_loan_member_access_mut_ref_to_ref_881ad91574d5/main.omg`
- hole_subtype: ``; probe: 

### 273. `tests/omega/run/borrows/shared_field_borrow/main.omg` (ref_to_mut_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -1,3 +1,3 @@
-// Shared `&` borrows of parameter-rooted places: `&main.size`,
+// Shared `&mut ` borrows of parameter-rooted places: `&main.size`,
 // `&main.buf` `&main.buf[0]` literal index. These were the
 // narrow routing family the shared arm already admitted on paper, but a
```

- Obligation line: `// Shared `&mut ` borrows of parameter-rooted places: `&main.size`,`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_borrows_shared_field_borrow_ref_to_mut_ref_24201784039b/main.omg`
- hole_subtype: ``; probe: 

### 274. `tests/omega/run/facts/double_indexed_rmw_through_call_result_traps/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: type fixed array`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -18,5 +18,4 @@
     transition { _ -> rmw(&mut grid, &mut rows, 1, 2, 2, 1) }
     machine rmw(grid: &mut [[i32 in Wrapping; 4]; 3], rows: &mut [Row; 3], i: u64, j: u64, ri: u64, rj: u64)
-    requires i <= 2
     requires j <= 3
     requires ri <= 2
```

- Obligation line: `data Row { tag: i32 in Wrapping; data: [i32 in Wrapping; 2]; }`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_facts_double_indexed_rmw_through_call_result_traps_drop_machine_requires_f9b8ca6ee254/main.omg`
- hole_subtype: `inferred-entry`; probe: probe call `rmw(3, 0, 0, 0, 0, 0)` omega_exit=1

### 275. `tests/omega/run/ownership/content_transform_inactive_payload/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: parameter mutable parameter`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -19,5 +19,4 @@
 }
 machine renew_empty(whole: Envelope in Accounted) -> Envelope in Accounted
-requires whole in Envelope::Empty
 ensures core::Identity<CountedQuantity<Units>>(
     Accounted::content(old(&whole)),
```

- Obligation line: `data Envelope [linear] dropped by drop_envelope {`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_ownership_content_transform_inactive_payload_drop_machine_requires_6d1583194ab7/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 276. `tests/omega/run/slices/disjoint_local_state_windows/main.omg` (drop_machine_requires, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/numeric_values.md` › Exact operations)

> Division or remainder by zero must be excluded before formation.

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine service reach`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -12,5 +12,4 @@
     }
     machine windows(main: &mut Main, left: &mut [i32 in Wrapping], right: &mut [i32 in Wrapping])
-    requires left.len >= 1 && right.len >= 1
     {
         left[0] = 5;
```

- Obligation line: `machine windows(main: &mut Main, left: &mut [i32 in Wrapping], right: &mut [i32 in Wrapping])`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_slices_disjoint_local_state_windows_drop_machine_requires_8335d931670f/main.omg`
- hole_subtype: `probe-not-built`; probe: could not synthesize violating argument

### 277. `tests/omega/run/state_owned_nested_borrow/main.omg` (mut_ref_to_ref, accepted_by=current)

**(a) Check-time spec** (`wiki/spec/language/ownership.md` › Owned local mutability)

> Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent

**(b) Acceptance mechanism:** `check_time_pass` (must not be runtime-trap-only; route hint `Unsupported: machine generic parameters`)

**(c) Minimal diff:**

```diff
--- original
+++ mutant
@@ -8,5 +8,5 @@
 }
 data Main {}
-machine main(main: &mut Main) crashes Trap {
+machine main(main: &Main) crashes Trap {
     let mut words: [u16; 4];
     words[3] = 0;
```

- Obligation line: `domain [u16; 4]::Terminated requires self[3] == 0;`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-2/tests_omega_run_state_owned_nested_borrow_mut_ref_to_ref_8bbe0d1e42b5/main.omg`
- hole_subtype: ``; probe: 

## Hand check: 5 discarded-valid

### 1. `tests/omega/pass/facts/case_payload_transport_through_other_state_exit/main.omg` / drop_machine_ensures / discarded_valid
- Validity read: dropping ensures weakens the guarantee; program may remain spec-valid
- accepted_by: `both`; route: `accepted`

## Hand check: 5 valid-route-outside

### 1. `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg` / drop_machine_ensures / valid_route_outside
- Validity read: dropping ensures weakens the guarantee; program may remain spec-valid
- accepted_by: `current`; route: `no-entry`

### 2. `tests/omega/pass/arithmetic/runtime_anonymous_array_stores_with_float_import_exit/main.omg` / remove_mut / valid_route_outside
- Validity read: demoted `mut` does not pair with a write or &mut on that binding
- accepted_by: `current`; route: `outside`

### 3. `tests/omega/pass/arithmetic/runtime_cyclic_parameter_fixed_use_after_console_exit/main.omg` / remove_mut / valid_route_outside
- Validity read: demoted `mut` does not pair with a write or &mut on that binding
- accepted_by: `current`; route: `outside`

### 4. `tests/omega/pass/arithmetic/runtime_explicit_call_operand_policy_exit/main.omg` / remove_mut / valid_route_outside
- Validity read: demoted `mut` does not pair with a write or &mut on that binding
- accepted_by: `current`; route: `outside`

### 5. `tests/omega/pass/arithmetic/runtime_guarded_read_count_assignment_exit/main.omg` / remove_mut / valid_route_outside
- Validity read: demoted `mut` does not pair with a write or &mut on that binding
- accepted_by: `current`; route: `outside`

## Hand check: 5 known-gap-q7-mut

_None._

## Commands

```bash
PYTHONUNBUFFERED=1 python3 .agents/four-jobs-2026-11-12/soundness/shard2_mutate_run.py
python3 .agents/four-jobs-2026-11-12/soundness/shard2_confirm.py
```

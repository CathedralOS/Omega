# Soundness mutations shard 7 (job 2)

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

Lead rules: `soundness_fix` + `soundness_fix2` (bucket table sums to mutant
total; drop_requires route reason recorded; Q7 remove_mut shadowing checked).

## Mutant total

- Rows in TSV (mutants run): **1585**
- Runner `generated=` counter: **n/a**
- Runner `run=` counter: **n/a**

- Delta: none (TSV row count matches completed runner total).

## Bucket table

Total: **1585**

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 46 |
| `discarded-valid` | 251 |
| `known-gap-q7-mut` | 136 |
| `rejected-both` | 1152 |
| **sum** | **1585** |

### Confirmed holes by mutation kind

- `mut_ref_instead_of_shared`: 34
- `shared_instead_of_mut`: 10
- `widen_requires`: 1
- `drop_requires`: 1

## Confirmed holes (summary)

- `mut_ref_instead_of_shared` `tests/omega/pass/arithmetic/every_width_index/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `let view: &[u64] = &mut values[..];`; 
- `shared_instead_of_mut` `tests/omega/pass/asm/observation_preserves_address_model/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `uefi_x86_64 machine convert(mapping: & Mapping) -> u64`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrow/storage_address_loop_copy_identity_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let original_address: addr = address_of(&mut original[0]);`; 
- `shared_instead_of_mut` `tests/omega/pass/borrows/loan_scalar_copies/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `// Scalar locals initialized from member reads through a `&` record`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrows/retained_envelope_refs/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `// a retained reference (`&mut retained` where retained is itself `&i32`) — plus a`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrows/runtime_computed_case_copy_order_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine select(cursor: &mut Cursor) -> u64`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrows/runtime_measured_range_record_domains_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `transition visited == 70 && shifted == 70 && TextBuffer::equal(&mut main.rooms[1].label, "vault") {`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrows/runtime_reference_carrier_state_loan_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine inspect(envelope: &mut Envelope<'storage>) -> u64 {`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/borrows/runtime_tail_recursive_disjoint_field_fact_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine require_bound(work: &mut Work) requires work.count <= 8 { }`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/build/native_component_case_loan_cycle/producer/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let result: u64 = block poll(&mut gate, &mut peer, 1u64);`; 
- `shared_instead_of_mut` `tests/omega/pass/calls/structural_mut_to_shared_argument/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `// A `&` reborrow weakens to the callee's shared loan: the caller's mutable`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/calls/structural_mut_to_shared_argument/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine Counts::clone(source: &mut Counts) -> Counts {`; 
- `shared_instead_of_mut` `tests/omega/pass/cases/case_where_match/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `machine Main::main(main: & Main) {`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/collections/runtime_projected_range_call_nonempty_requirement_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine later(cursor: &mut Cursor, result: u32) -> u32 {`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/contracts/runtime_literal_view_extent_calls_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let bounded_length: u64 = two_bytes(&mut bounded);`; 
- `widen_requires` `tests/omega/pass/contracts/strict_clause_terminated_headers/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `machine value(input: u64) -> u64 requires input < 10 terminates;`; 
- `shared_instead_of_mut` `tests/omega/pass/control_flow/copy_enum_cycle_edge_write_frame/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `// write-capable root: `& T` is the only exclusive observation-and-mutation`; 
- `shared_instead_of_mut` `tests/omega/pass/core/content_projection_readable_reference_observation/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `machine mutable(whole: & Region in Owned)`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/domains/domain_intersection/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `// `password in Password::Valid &mut  Password::Secure` — each member's`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/domains/indexed_domain_fact/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `// `ensures result in Granted &mut  Resident<SlotPlacement, Slot>` is`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/domains/routed_selected_copy_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let observed: u64 = select(&mut frames, 9);`; 
- `shared_instead_of_mut` `tests/omega/pass/expressions/derives_helper_return/main.omg` — omega **accepted**, current **accepted**, route **no-entry**; obligation `machine Worker::open_main_log(worker: &mut Worker, folder: Folder) -> File`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/computed_constant_array_extent_through_other_machine_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `transition Inspector::check(&mut local_arr) { true -> done() _ -> bad() }`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/dependent_subtract_through_call_result_compile/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine used_amount(main: &mut Main) -> u32`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/domain_selected_cleanup_computed_other_machine_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine observe(keeper: &mut Keeper in Held<7>) {}`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_computed_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine observe(keeper: &mut Keeper) {}`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/domain_selected_cleanup_through_other_machine_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine observe(keeper: &mut Keeper) {}`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/finite_family_input_index/other_machine/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `check_observations(&mut observed, expected_marker, width, expected_branch);`; 
- `shared_instead_of_mut` `tests/omega/pass/facts/guarded_call_entry_through_other_state_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let mut context: Ctx = Ctx { slots: slots, count: 0u64, valid: true };`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/guarded_line_loan_through_other_machine_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `ReadOutcome::Complete { count } -> evaluate(observe(&mut line, count))`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/implementation_region_forwarding_through_const_name_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let result: u64 = exercise(&mut environment, &storage);`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/indexed_crash_guard_through_local_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `transition items[0] == 7 { true -> finish(read_seven(&mut items, 0)) _ -> bad() }`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/indexed_storage_address_identity_through_call_result_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let selected: addr = address_of(&mut items[index]);`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/indexed_storage_address_identity_through_target_body_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let selected: addr = address_of(&mut items[index]);`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_other_state_exit/main.omg` — omega **accepted**, current **accepted**, route **accepted**; obligation `// Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.`; 
- `drop_requires` `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_other_state_exit/main.omg` — omega **accepted**, current **accepted**, route **accepted**; obligation `machine step1(x: u64) -> u64`; dropped requires: x == 10
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/may_write_codec_argument_through_call_result_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `Blob::encode(&mut sample, &mut buffer, choose(&mut chosen, &mut other, true));`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/may_write_codec_argument_through_target_body_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine probe(chosen: &mut u64, other: &mut u64, items: &[u8; 4]) -> u8`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/may_write_codec_through_parameter_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `machine probe(chosen: &mut u64, other: &mut u64, items: &[u8; 4]) -> u8`; 
- `mut_ref_instead_of_shared` `tests/omega/pass/facts/may_write_named_state_through_other_machine_exit/main.omg` — omega **accepted**, current **accepted**, route **outside**; obligation `let found: u8 = read_at(&mut items, other);`; 

## Hand verification: 5 confirmed holes

### mut_ref_instead_of_shared
- `tests/omega/pass/arithmetic/every_width_index/main.omg` → `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg`
- 

```diff
-     let view: &[u64] = &values[..];
+     let view: &[u64] = &mut values[..];
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg	current=accepted	current_us=1618115	root=compilation	route=no-entry

### shared_instead_of_mut
- `tests/omega/pass/asm/observation_preserves_address_model/main.omg` → `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg`
- 

```diff
- uefi_x86_64 machine convert(mapping: &mut Mapping) -> u64
+ uefi_x86_64 machine convert(mapping: & Mapping) -> u64
```

- omega exit=0
- omega stdout head: compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg	current=accepted	current_us=1613150	root=compilation	route=no-entry

### mut_ref_instead_of_shared
- `tests/omega/pass/borrow/storage_address_loop_copy_identity_exit/main.omg` → `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg`
- 

```diff
-     let original_address: addr = address_of(&original[0]);
+     let original_address: addr = address_of(&mut original[0]);
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg	current=accepted	current_us=3075403	root=compilation	route=outside	Unsupported: type reference

### shared_instead_of_mut
- `tests/omega/pass/borrows/loan_scalar_copies/main.omg` → `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg`
- 

```diff
- // Scalar locals initialized from member reads through a `&mut` record
+ // Scalar locals initialized from member reads through a `&` record
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg	current=accepted	current_us=1652650	root=compilation	route=no-entry

### mut_ref_instead_of_shared
- `tests/omega/pass/borrows/retained_envelope_refs/main.omg` → `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg`
- 

```diff
- // a retained reference (`&retained` where retained is itself `&i32`) — plus a
+ // a retained reference (`&mut retained` where retained is itself `&i32`) — plus a
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg	current=accepted	current_us=1634423	root=compilation	route=no-entry

## Hand verification: 5 discarded drop_requires

### discarded drop_requires
- `tests/omega/pass/borrows/opaque_incoming_nested_projected_record_result_exit/main.omg`
- route **outside**; reason: Unsupported: type reference

```diff
- requires wrapper.bundle.windows[0].len == 1
+ {
- {
+     wrapper.bundle.windows[0].len == 1 && wrapper.bundle.windows[0][0] == 19
-     wrapper.bundle.windows[0].len == 1 && wrapper.bundle.windows[0][0] == 19
+ }
- }
+ data Container<'bundle, 'storage> { wrapper: Wrapper<'bundle, 'storage>; }
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__opaque_incoming_nested_projected_record_result_exit__main.omg/drop_requires_48a83d9daa/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__opaque_incoming_nested_projected_record_result_exit__main.omg/drop_requires_48a83d9daa/main.omg	current=accepted	current_us=3494236	root=compilation	route=outside	Unsupported: type reference

### discarded drop_requires
- `tests/omega/pass/calls/ensures_result_membership_state_graph_issuer_exit/main.omg`
- route **outside**; reason: Unsupported: machine attached data

```diff
- requires value.ticket in Ticket::Issued
+ crashes Trap {
- crashes Trap {
+     transition value.replaced && value.ticket.amount == 41 && value.ticket.marker == 9 {
-     transition value.replaced && value.ticket.amount == 41 && value.ticket.marker == 9 {
+         true -> done()
-         true -> done()
+         false -> bad()
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__ensures_result_membership_state_graph_issuer_exit__main.omg/drop_requires_104c0fe3e2/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__ensures_result_membership_state_graph_issuer_exit__main.omg/drop_requires_104c0fe3e2/main.omg	current=accepted	current_us=3412457	root=compilation	route=outside	Unsupported: machine attached data

### discarded drop_requires
- `tests/omega/pass/calls/native_code_layout_lifetime_exit/main.omg`
- route **outside**; reason: Unsupported: machine generic parameters

```diff
-     requires layout.regions.len > 0
+     {
-     {
+         layout.entry != 0 && layout.regions[0].base == layout.entry && layout.regions[0].length > 0
-         layout.entry != 0 && layout.regions[0].base == layout.entry && layout.regions[0].length > 0
+     }
-     }
+ }
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__native_code_layout_lifetime_exit__main.omg/drop_requires_48a142fab6/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__native_code_layout_lifetime_exit__main.omg/drop_requires_48a142fab6/main.omg	current=accepted	current_us=3211549	root=compilation	route=outside	Unsupported: machine generic parameters

### discarded drop_requires
- `tests/omega/pass/contracts/private_arrival_cases_exit/main.omg`
- route **outside**; reason: Unsupported: parameter mutable parameter

```diff
- requires v == 0
+ { v }
- { v }
+ machine read_count(c: &mut Counter) -> u64 { need_zero(c.count) }
- machine read_count(c: &mut Counter) -> u64 { need_zero(c.count) }
+ 
- 
+ public machine invoke_root<machine Reader>(s: &mut Slot) -> i64
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__private_arrival_cases_exit__main.omg/drop_requires_a77c98d65c/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__private_arrival_cases_exit__main.omg/drop_requires_a77c98d65c/main.omg	current=accepted	current_us=3606600	root=compilation	route=outside	Unsupported: parameter mutable parameter

### discarded drop_requires
- `tests/omega/pass/contracts/runtime_literal_view_extent_calls_exit/main.omg`
- route **outside**; reason: Unsupported: type reference

```diff
- requires bytes.len == 2
+ {
- {
+     bytes.len
-     bytes.len
+ }
- }
+ 
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__runtime_literal_view_extent_calls_exit__main.omg/drop_requires_f984f68499/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__runtime_literal_view_extent_calls_exit__main.omg/drop_requires_f984f68499/main.omg	current=accepted	current_us=3255295	root=compilation	route=outside	Unsupported: type reference

## Hand verification: 5 known-gap Q7 remove_mut

### remove_mut (Q7)
- obligation: `transition captured == 1u32 && cell.value == 2u32 && difference == 0u32 {`
- `tests/omega/pass/arithmetic/call_argument_snapshot_survives_write_exit/main.omg`

```diff
-     let mut cell: Cell = Cell { value: 1u32 };
+     let cell: Cell = Cell { value: 1u32 };
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__call_argument_snapshot_survives_write_exit__main.omg/drop_mut_8f93f94ace/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__call_argument_snapshot_survives_write_exit__main.omg/drop_mut_8f93f94ace/main.omg	current=accepted	current_us=3416274	root=compilation	route=outside	Unsupported: parameter mutable parameter

### remove_mut (Q7)
- obligation: `transition value == 0 && flag == 1 {`
- `tests/omega/pass/arithmetic/runtime_captured_call_result_subtracts_itself_exit/main.omg`

```diff
-     let mut flag: u64 = 0;
+     let flag: u64 = 0;
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__runtime_captured_call_result_subtracts_itself_exit__main.omg/drop_mut_233016365f/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__runtime_captured_call_result_subtracts_itself_exit__main.omg/drop_mut_233016365f/main.omg	current=accepted	current_us=3444331	root=compilation	route=outside	Unsupported: parameter mutable parameter

### remove_mut (Q7)
- obligation: `Main::main(&mut entry_storage);`
- `tests/omega/pass/borrow/runtime_write_only_affine_sum_exit/main.omg`

```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_affine_sum_exit__main.omg/drop_mut_3842ac4802/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_affine_sum_exit__main.omg/drop_mut_3842ac4802/main.omg	current=accepted	current_us=3297735	root=compilation	route=outside	Unsupported: type data with custody

### remove_mut (Q7)
- obligation: `replace(&write held, identity(incoming));`
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_operand_exit/main.omg`

```diff
-     let mut held: Held = Held { target: &mut main.left, stamp: 17 };
+     let held: Held = Held { target: &mut main.left, stamp: 17 };
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_call_operand_exit__main.omg/drop_mut_c1fe05d15f/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_exclusive_reference_call_operand_exit__main.omg/drop_mut_c1fe05d15f/main.omg	current=accepted	current_us=3805507	root=compilation	route=outside	Unsupported: parameter mutable parameter

### remove_mut (Q7)
- obligation: `Main::main(&mut entry_storage);`
- `tests/omega/pass/borrow/runtime_write_only_indexed_record_copy_exit/main.omg`

```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```

- omega exit=0
- omega stdout head: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_indexed_record_copy_exit__main.omg/drop_mut_c3dcdea5f6/main.omg; wrote_output=false | 
- program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__runtime_write_only_indexed_record_copy_exit__main.omg/drop_mut_c3dcdea5f6/main.omg	current=accepted	current_us=3366883	root=compilation	route=outside	Unsupported: parameter mutable parameter


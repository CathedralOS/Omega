# Soundness mutations shard 7 (job 2)

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
FOUR_JOBS_SHARD=7 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
FOUR_JOBS_SHARD=7 python3 .agents/four-jobs-2026-11-12/soundness/review_shard_candidates.py
```

## Bucket table (sums to mutant total)

Total mutants run: **1548**

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 48 |
| `discarded-valid` | 200 |
| `known-gap-q7-mut` | 169 |
| `rejected-both` | 1131 |
| **sum** | **1548** |

Confirmed hole = spec-invalid mutant **and** current checker accepts (omega or route `current=`). Route accept alone is not evidence.

### Confirmed holes by mutation kind

- `mut_ref_instead_of_shared`: 33
- `shared_instead_of_mut`: 10
- `drop_requires`: 4
- `widen_requires`: 1

## Known gap: Q7 mut

169 rows bucketed as tracked missing-`mut` enforcement (not spec holes).

## Confirmed holes

### mut_ref_instead_of_shared — `tests/omega/pass/arithmetic/every_width_index/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `let view: &[u64] = &mut values[..];`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/asm/observation_preserves_address_model/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `uefi_x86_64 machine convert(mapping: & Mapping) -> u64`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/borrow/storage_address_loop_copy_identity_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let original_address: addr = address_of(&mut original[0]);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/borrows/loan_scalar_copies/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `// Scalar locals initialized from member reads through a `&` record`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/borrows/retained_envelope_refs/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `// a retained reference (`&mut retained` where retained is itself `&i32`) — plus a`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/borrows/runtime_computed_case_copy_order_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__runtime_computed_case_copy_order_exit__main.omg/mut_ref_instead_of_shared_8d7755b8b8/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine select(cursor: &mut Cursor) -> u64`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/borrows/runtime_measured_range_record_domains_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__runtime_measured_range_record_domains_exit__main.omg/mut_ref_instead_of_shared_06d5598415/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `transition visited == 70 && shifted == 70 && TextBuffer::equal(&mut main.rooms[1].label, "vault") {`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/borrows/runtime_reference_carrier_state_loan_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__runtime_reference_carrier_state_loan_exit__main.omg/mut_ref_instead_of_shared_5945381899/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine inspect(envelope: &mut Envelope<'storage>) -> u64 {`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/borrows/runtime_tail_recursive_disjoint_field_fact_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__runtime_tail_recursive_disjoint_field_fact_exit__main.omg/mut_ref_instead_of_shared_317f447a4e/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine require_bound(work: &mut Work) requires work.count <= 8 { }`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/build/native_component_case_loan_cycle/producer/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__build__native_component_case_loan_cycle__producer__main.omg/mut_ref_instead_of_shared_45919d7565/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let result: u64 = block poll(&mut gate, &mut peer, 1u64);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### drop_requires — `tests/omega/pass/calls/ensures_result_membership_state_graph_issuer_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__ensures_result_membership_state_graph_issuer_exit__main.omg/drop_requires_104c0fe3e2/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine done() {}`
- Spec: `wiki/spec/proofs/contracts.md` (## Machines and bundles)

> Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.

### shared_instead_of_mut — `tests/omega/pass/calls/structural_mut_to_shared_argument/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__structural_mut_to_shared_argument__main.omg/shared_instead_of_mut_d6a1b85d65/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `// A `&` reborrow weakens to the callee's shared loan: the caller's mutable`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/calls/structural_mut_to_shared_argument/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__calls__structural_mut_to_shared_argument__main.omg/mut_ref_instead_of_shared_ebefe83215/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine Counts::clone(source: &mut Counts) -> Counts {`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/cases/case_where_match/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__cases__case_where_match__main.omg/shared_instead_of_mut_2c37d8395a/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `machine Main::main(main: & Main) {`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/collections/runtime_projected_range_call_nonempty_requirement_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__collections__runtime_projected_range_call_nonempty_requirement_exit__main.omg/mut_ref_instead_of_shared_a0a11a66a9/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine later(cursor: &mut Cursor, result: u32) -> u32 {`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/contracts/runtime_literal_view_extent_calls_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__runtime_literal_view_extent_calls_exit__main.omg/mut_ref_instead_of_shared_8360772f08/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let bounded_length: u64 = two_bytes(&mut bounded);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### widen_requires — `tests/omega/pass/contracts/strict_clause_terminated_headers/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__contracts__strict_clause_terminated_headers__main.omg/widen_requires_1a25e2bcce/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `machine value(input: u64) -> u64 requires input < 10 terminates;`
- Spec: `wiki/spec/proofs/contracts.md` (## Machines and bundles)

> Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.

### shared_instead_of_mut — `tests/omega/pass/control_flow/copy_enum_cycle_edge_write_frame/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__control_flow__copy_enum_cycle_edge_write_frame__main.omg/shared_instead_of_mut_b8b3d9bd24/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `// write-capable root: `& T` is the only exclusive observation-and-mutation`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### shared_instead_of_mut — `tests/omega/pass/core/content_projection_readable_reference_observation/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__core__content_projection_readable_reference_observation__main.omg/shared_instead_of_mut_7f3a94723b/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `machine mutable(whole: & Region in Owned)`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### drop_requires — `tests/omega/pass/domains/case_payload_widened_call_result_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__domains__case_payload_widened_call_result_exit__main.omg/drop_requires_16db3df0b5/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `case Byte(value: i32);`
- Spec: `wiki/spec/proofs/contracts.md` (## Machines and bundles)

> Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.

### mut_ref_instead_of_shared — `tests/omega/pass/domains/domain_intersection/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__domains__domain_intersection__main.omg/mut_ref_instead_of_shared_61bdcd3a67/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `// `password in Password::Valid &mut  Password::Secure` — each member's`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/domains/indexed_domain_fact/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__domains__indexed_domain_fact__main.omg/mut_ref_instead_of_shared_62341996bf/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `// `ensures result in Granted &mut  Resident<SlotPlacement, Slot>` is`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/domains/routed_selected_copy_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__domains__routed_selected_copy_exit__main.omg/mut_ref_instead_of_shared_e283f055e8/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let observed: u64 = select(&mut frames, 9);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/expressions/derives_helper_return/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__expressions__derives_helper_return__main.omg/shared_instead_of_mut_ce61577bac/main.omg`
- omega: **accepted**; current: **accepted**; route: **no-entry**
- Obligation line: `machine Worker::open_main_log(worker: &mut Worker, folder: Folder) -> File`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### drop_requires — `tests/omega/pass/facts/applied_requirement_indices_through_const_name_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__applied_requirement_indices_through_const_name_exit__main.omg/drop_requires_4c686084be/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine identity(value: Payload in Tagged<Word>) -> Payload in Tagged<Word> { value }`
- Spec: `wiki/spec/proofs/contracts.md` (## Machines and bundles)

> Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/computed_constant_array_extent_through_other_machine_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__computed_constant_array_extent_through_other_machine_exit__main.omg/mut_ref_instead_of_shared_6ec38f6be1/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `transition Inspector::check(&mut local_arr) { true -> done() _ -> bad() }`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/dependent_subtract_through_call_result_compile/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__dependent_subtract_through_call_result_compile__main.omg/mut_ref_instead_of_shared_860affc783/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine used_amount(main: &mut Main) -> u32`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/domain_selected_cleanup_computed_other_machine_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__domain_selected_cleanup_computed_other_machine_exit__main.omg/mut_ref_instead_of_shared_7f107b6170/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine observe(keeper: &mut Keeper in Held<7>) {}`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_computed_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__domain_selected_cleanup_indexed_inherited_computed_exit__main.omg/mut_ref_instead_of_shared_b73efa4487/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine observe(keeper: &mut Keeper) {}`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/domain_selected_cleanup_through_other_machine_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__domain_selected_cleanup_through_other_machine_exit__main.omg/mut_ref_instead_of_shared_0f4dbec749/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine observe(keeper: &mut Keeper) {}`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/finite_family_input_index/other_machine/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__finite_family_input_index__other_machine__main.omg/mut_ref_instead_of_shared_31048939bc/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `check_observations(&mut observed, expected_marker, width, expected_branch);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/facts/guarded_call_entry_through_other_state_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__guarded_call_entry_through_other_state_exit__main.omg/shared_instead_of_mut_ffbc42b327/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let mut context: Ctx = Ctx { slots: slots, count: 0u64, valid: true };`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/guarded_line_loan_through_other_machine_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__guarded_line_loan_through_other_machine_exit__main.omg/mut_ref_instead_of_shared_3c66048de6/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `ReadOutcome::Complete { count } -> evaluate(observe(&mut line, count))`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/implementation_region_forwarding_through_const_name_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__implementation_region_forwarding_through_const_name_exit__main.omg/mut_ref_instead_of_shared_d80a91f13f/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let result: u64 = exercise(&mut environment, &storage);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/indexed_crash_guard_through_local_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__indexed_crash_guard_through_local_exit__main.omg/mut_ref_instead_of_shared_fe7cadbfcc/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `transition items[0] == 7 { true -> finish(read_seven(&mut items, 0)) _ -> bad() }`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/indexed_storage_address_identity_through_call_result_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__indexed_storage_address_identity_through_call_result_exit__main.omg/mut_ref_instead_of_shared_a383a2a4ae/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let selected: addr = address_of(&mut items[index]);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/indexed_storage_address_identity_through_target_body_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__indexed_storage_address_identity_through_target_body_exit__main.omg/mut_ref_instead_of_shared_deb6c60ace/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let selected: addr = address_of(&mut items[index]);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_other_state_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__integer_width_truncation_mask_bound_through_other_state_exit__main.omg/mut_ref_instead_of_shared_9039748eb9/main.omg`
- omega: **accepted**; current: **accepted**; route: **accepted**
- Obligation line: `// Fact: masking an unsigned 64-bit integer to its low byte (val &mut  255u64) establishes the exact byte-width upper bound (masked <= 255u64), discharging exact subtraction 255u64 - masked across all nine places.`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/may_write_codec_argument_through_call_result_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__may_write_codec_argument_through_call_result_exit__main.omg/mut_ref_instead_of_shared_c813fbe135/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `Blob::encode(&mut sample, &mut buffer, choose(&mut chosen, &mut other, true));`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/may_write_codec_argument_through_target_body_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__may_write_codec_argument_through_target_body_exit__main.omg/mut_ref_instead_of_shared_359c988285/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine probe(chosen: &mut u64, other: &mut u64, items: &[u8; 4]) -> u8`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/may_write_codec_through_parameter_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__may_write_codec_through_parameter_exit__main.omg/mut_ref_instead_of_shared_4463bec3a4/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `machine probe(chosen: &mut u64, other: &mut u64, items: &[u8; 4]) -> u8`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### drop_requires — `tests/omega/pass/facts/may_write_computed_receiver_through_other_state_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__may_write_computed_receiver_through_other_state_exit__main.omg/drop_requires_a95561c048/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `transition chosen == 9 && items[index] == 7 {`
- Spec: `wiki/spec/proofs/contracts.md` (## Machines and bundles)

> Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/may_write_named_state_through_other_machine_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__may_write_named_state_through_other_machine_exit__main.omg/mut_ref_instead_of_shared_465e43e807/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let found: u8 = read_at(&mut items, other);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/method_array_operand_inference_through_field_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__method_array_operand_inference_through_field_exit__main.omg/mut_ref_instead_of_shared_d98ba19ae3/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let observed: u8 = Bytes::last<3u64>(&mut packet.items);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### shared_instead_of_mut — `tests/omega/pass/facts/mutable_borrow_field_writeback_through_parameter_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__mutable_borrow_field_writeback_through_parameter_exit__main.omg/shared_instead_of_mut_7bae172d14/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `// Fact: Updating a record field through an exclusive mutable borrow (`& Counter`) and retiring the loan establishes the updated field bound (`counter.used <= 80u64`) while preserving unrelated fields, discharging exact subtraction `80u64 - counter.used` across all nine places.`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### shared_instead_of_mut — `tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_state_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__mutable_slice_element_write_bound_through_other_state_exit__main.omg/shared_instead_of_mut_4276f8ec98/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `// Fact: Writing an element through a mutable slice view (& [u64]) whose length is guarded (view.len >= 2u64) proves safe indexed element mutation at view[1u64] while preserving the slice extent lower bound (view.len >= 2u64) to discharge exact subtraction view.len - 2u64 across all nine places.`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> formation of `&mut` or `&write` loans to those places.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/named_const_callable_family_through_match_join_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__named_const_callable_family_through_match_join_exit__main.omg/mut_ref_instead_of_shared_5568bf3924/main.omg`
- omega: **accepted**; current: **accepted**; route: **outside**
- Obligation line: `let first: u64 = apply<FOUR, measure>(&mut four);`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

### mut_ref_instead_of_shared — `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_local_exit/main.omg`

- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__facts__narrow_to_narrow_truncation_64to8_mask_bound_through_local_exit__main.omg/mut_ref_instead_of_shared_151f7c6e73/main.omg`
- omega: **accepted**; current: **accepted**; route: **accepted**
- Obligation line: `// Fact: masking an unsigned 64-bit integer to 5 bits and narrowing through u8 ((x &mut  31u64) as u8 as u64) preserves the exact upper bound (truncated <= 31u64), discharging exact subtraction 31u64 - truncated across all nine places.`
- Spec: `wiki/spec/language/ownership.md` (## Owned local mutability)

> A callee cannot grant writable access that the caller's place lacks.

## Checker disagreements


## Hand verification (5 samples)

### Sample: mut_ref_instead_of_shared / confirmed-hole

- Original: `tests/omega/pass/arithmetic/every_width_index/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg`

**Diff (first `let mut` change or removed `requires`):**
```omg
-     let view: &[u64] = &values[..];
+     let view: &[u64] = &mut values[..];
```

- omega exit=0
- omega stdout: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg; wrote_output=false | 
- route exit=0
- route: program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__arithmetic__every_width_index__main.omg/mut_ref_instead_of_shared_e758417cff/main.omg	current=accepted	current_us=1709064	root=compilation	route=no-entry

### Sample: shared_instead_of_mut / confirmed-hole

- Original: `tests/omega/pass/asm/observation_preserves_address_model/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg`

**Diff (first `let mut` change or removed `requires`):**
```omg
- uefi_x86_64 machine convert(mapping: &mut Mapping) -> u64
+ uefi_x86_64 machine convert(mapping: & Mapping) -> u64
```

- omega exit=0
- omega stdout: compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg; wrote_output=false | 
- route exit=0
- route: program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__asm__observation_preserves_address_model__main.omg/shared_instead_of_mut_15ac5c25c7/main.omg	current=accepted	current_us=1658125	root=compilation	route=no-entry

### Sample: mut_ref_instead_of_shared / confirmed-hole

- Original: `tests/omega/pass/borrow/storage_address_loop_copy_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg`

**Diff (first `let mut` change or removed `requires`):**
```omg
-     let original_address: addr = address_of(&original[0]);
+     let original_address: addr = address_of(&mut original[0]);
```

- omega exit=0
- omega stdout: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg; wrote_output=false | 
- route exit=0
- route: program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrow__storage_address_loop_copy_identity_exit__main.omg/mut_ref_instead_of_shared_9373606160/main.omg	current=accepted	current_us=3412317	root=compilation	route=outside	Unsupported: type reference

### Sample: shared_instead_of_mut / confirmed-hole

- Original: `tests/omega/pass/borrows/loan_scalar_copies/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg`

**Diff (first `let mut` change or removed `requires`):**
```omg
- // Scalar locals initialized from member reads through a `&mut` record
+ // Scalar locals initialized from member reads through a `&` record
```

- omega exit=0
- omega stdout: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg; wrote_output=false | 
- route exit=0
- route: program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__loan_scalar_copies__main.omg/shared_instead_of_mut_0ebb04153c/main.omg	current=accepted	current_us=1683482	root=compilation	route=no-entry

### Sample: mut_ref_instead_of_shared / confirmed-hole

- Original: `tests/omega/pass/borrows/retained_envelope_refs/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg`

**Diff (first `let mut` change or removed `requires`):**
```omg
- // a retained reference (`&retained` where retained is itself `&i32`) — plus a
+ // a retained reference (`&mut retained` where retained is itself `&i32`) — plus a
```

- omega exit=0
- omega stdout: compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg; wrote_output=false | 
- route exit=0
- route: program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-7/tests__omega__pass__borrows__retained_envelope_refs__main.omg/mut_ref_instead_of_shared_cad959472a/main.omg	current=accepted	current_us=1722198	root=compilation	route=no-entry


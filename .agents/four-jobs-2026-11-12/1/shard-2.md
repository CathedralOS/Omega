# Route coverage shard 2 (four-jobs 2026-11-12)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

## Host

```
Linux cursor 6.12.113+ #1 SMP PREEMPT_DYNAMIC Mon Nov  9 16:04:37 UTC 2026 x86_64 x86_64 x86_64 GNU/Linux
```

## Harness

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison
```

Binary: `/agent/repos/omega/target/debug/examples/route_one_comparison`

## Commands

```bash
git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' \
  'tests/omega/fail/**/main.omg' | LC_ALL=C sort
# shard S = 0-based index i where i % 8 == S

timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null
timeout 300 target/debug/examples/route_one_comparison --every-machine \
  <fail-fixture>/main.omg < /dev/null

python3 .agents/four-jobs-2026-11-12/1/run_shard.py 2 3
```

## Build slot / parallelism

`tools/build_slot.md` limits **one heavy job per test VM** (mbx build, corpus
sweep, ledger re-record). This shard run is not a corpus sweep: it executes
prebuilt `route_one_comparison` once per fixture (`timeout 300`), matching
`route_one_diagnostics.md` (three fixture processes at a time, `nice -n 19`).
That is below the VM-heavy lock used for `mbx`/`cargo` builds. A prior
serial smoke run (125 fixtures) matched the same verdict distribution as the
parallel driver before the parser fix.

## Agreement definition

Each row compares the shard **aggregate** `route_verdict` (same closure
precedence as `route_one_comparison --tally`: any `rejected` root rejects the
fixture; else `accepted` if any closure accepts even when others are outside)
with the harness **`current=`** field from the same `route_one_comparison` run
(the typed-frontier current checker), not corpus tier or ledger.

- **agree**: both accepted, or both rejected (current `stopped` pairs with
  route `rejected`).
- **disagree**: judged route vs current differ.
- **n/a**: route `outside`, `timeout`, or `error`.

## Meta first-refusal buckets

- `meta:no-program-entry`: Build binds no `ProgramEntry`; compilation root is
  not elaborated (common for `fail/`). With `--every-machine`, machine closures
  are still judged; this meta label is skipped when a machine root exists.
- `meta:typed-frontier-stopped`: current checker `stopped` before typed trees;
  route `unreached`.

## `installed_arrival_*` fixtures

Owner-noted slow cases `fail/build/installed_arrival_without_context` and
`installed_arrival_wrong_controller` are **not** in shard 2 (indices 950–951,
`i % 8` is 6 and 7). No `installed_arrival_*` paths in this shard.

## Shard totals

- Fixtures in shard: **2174**
- Judged (accepted + rejected): **310**
- Outside: **1864**
- Timeout (separate bucket): **0**
- Error: **0**
- Agree: **95**
- Disagree: **215**
- Agreement n/a: **1864**

- Disagree route=accepted current=rejected: **193**
- Disagree route=rejected current=accepted: **22**

### Route verdict counts

- `accepted`: 229
- `outside`: 1864
- `rejected`: 81

### Outside grouped by first refusal construct (top 30)

- **machine service reach** (332): `tests/omega/fail/borrows/foreign_output_live_lifetime_source_write/main.omg`, `tests/omega/fail/borrows/reference_result_case_payload_missing_premise/main.omg`, `tests/omega/fail/borrows/suspension_projected_call_view_forbidden_alternative_rejected/main.omg`
- **parameter mutable parameter** (253): `tests/omega/fail/borrow/relational_handback_requires_return_guarantee/main.omg`, `tests/omega/fail/borrow/scalar_field_local_loan_requires_handback/main.omg`, `tests/omega/fail/borrow/shared_boundary_presentation_preserves_exclusive_conflict/main.omg`
- **machine generic parameters** (175): `tests/omega/fail/borrow/captured_window_sibling_loan_keeps_loan/main.omg`, `tests/omega/fail/borrows/completed_reference_result_retains_selected_input/main.omg`, `tests/omega/fail/borrows/computed_shared_byte_field_window_requires_mutable/main.omg`
- **meta:no-program-entry** (150): `tests/omega/fail/build/native_rows_from_two_suppliers_conflict/library/main.omg`, `tests/omega/fail/build/retained_const_argument_out_of_carrier/shapes/main.omg`, `tests/omega/fail/core/content_projection_foreign_package/owner/main.omg`
- **type reference** (141): `tests/omega/fail/arithmetic/collection_result_rebound_descriptor_in_later_state/main.omg`, `tests/omega/fail/atomics/atomic_boolean_store/main.omg`, `tests/omega/fail/borrows/aliased_descriptor_slot_suspended_call/main.omg`
- **declaration Build selection that is not a machine or a data type** (135): `tests/omega/fail/borrows/nested_machine_carried_live_child_rejected/main.omg`, `tests/omega/fail/boundary/named_target_foreign_provider_missing_member/main.omg`, `tests/omega/fail/build/segment_selector_invalidates_descriptor/main.omg`
- **meta:typed-frontier-stopped** (131): `tests/omega/fail/asm/arm_subs_wide_alias/main.omg`, `tests/omega/fail/asm/checked_fence_dmb_extra_operand/main.omg`, `tests/omega/fail/asm/cyclic_label/main.omg`
- **type data with custody** (94): `tests/omega/fail/arithmetic/constructor_divisor_requires_nonzero/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_exact_overflow/main.omg`, `tests/omega/fail/borrows/borrow_local_alias_active/main.omg`
- **machine attached data** (75): `tests/omega/fail/arithmetic/implicit_store_call_result_conversion_rejected/main.omg`, `tests/omega/fail/arithmetic/wrong_struct_type_argument_rejected/main.omg`, `tests/omega/fail/asm/observation_does_not_establish_address_model/main.omg`
- **type constrained** (59): `tests/omega/fail/arithmetic/captured_boolean_exposed_condition/main.omg`, `tests/omega/fail/arrays/qualified_array_literal_rejects_element_policy_conversion/main.omg`, `tests/omega/fail/data/qualified_record_pattern_field_weakening/main.omg`
- **declaration const** (35): `tests/omega/fail/facts/abi_multi_arg_spill_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_representation_view_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_const_name_rejected/main.omg`
- **type float** (32): `tests/omega/fail/calls/affine_call_local_moved_twice/main.omg`, `tests/omega/fail/calls/named_transition_float_suffix_mismatch/main.omg`, `tests/omega/fail/facts/core_float_representation_through_parameter_rejected/main.omg`
- **contract contracts on a state** (24): `tests/omega/fail/borrows/copied_byte_predicate_invalidated_by_destination_write/main.omg`, `tests/omega/fail/borrows/qualified_match_loan_missing_arm/main.omg`, `tests/omega/fail/collections/declared_element_predicate_stronger_duplicate_rejected/main.omg`
- **type fixed array** (23): `tests/omega/fail/borrow/static_byte_state_loan_cannot_write/main.omg`, `tests/omega/fail/facts/computed_constant_array_extent_through_call_result_rejected/main.omg`, `tests/omega/fail/facts/fixed_array_eight_elem_index_bound_through_parameter_rejected/main.omg`
- **machine type equations, owned data or conformances** (23): `tests/omega/fail/calls/selected_crash_private_forwarding_rejected/main.omg`, `tests/omega/fail/core/content_conservation_unqualified_place/main.omg`, `tests/omega/fail/core/content_retained_custody_from_borrow/main.omg`
- **expression match on a scalar subject** (21): `tests/omega/fail/arithmetic/exact_dispatch_divisor_unproven/main.omg`, `tests/omega/fail/expressions/match_owned_unqualified_result_rejected/main.omg`, `tests/omega/fail/facts/bounded_products_through_match_join_rejected/main.omg`
- **contract term other than a formal or literal** (15): `tests/omega/fail/arithmetic/affine_difference_unpaired_term/main.omg`, `tests/omega/fail/arithmetic/total_contract_division_wrong_arm/main.omg`, `tests/omega/fail/asm/checked_asm_nominal_symbolic_conjunction_wrong/main.omg`
- **type generic data** (13): `tests/omega/fail/borrows/stored_primitive_reference_backing_write/main.omg`, `tests/omega/fail/facts/representation_agreement_through_field_rejected/main.omg`, `tests/omega/fail/facts/runtime_index_application_through_match_join_rejected/main.omg`
- **machine target selection** (12): `tests/omega/fail/borrows/boundary_argument_later_write_under_element_loan/main.omg`, `tests/omega/fail/borrows/opaque_generic_mutable_stored_writeonly_provider/main.omg`, `tests/omega/fail/borrows/target_family_backing_alternatives_rejected/main.omg`
- **machine suspends or blocks** (12): `tests/omega/fail/calls/recursive_terminal_parenthesized_missing_acknowledgement/main.omg`, `tests/omega/fail/effects/synchronous_callback_block_required/main.omg`, `tests/omega/fail/facts/inline_member_helper_through_const_name_rejected/main.omg`
- **machine assembly target** (11): `tests/omega/fail/asm/checked_asm_compare_stale_intermediate_goal/main.omg`, `tests/omega/fail/facts/checked_asm_add_carry_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/checked_asm_flag_restore_through_call_result_rejected/main.omg`
- **contract clause that is not a comparison or conjunction** (10): `tests/omega/fail/domains/inactive_payload_cannot_supply_false_contract/main.omg`, `tests/omega/fail/facts/projection_call_premise_through_field_rejected/main.omg`, `tests/omega/fail/facts/quotient_closed_literal_through_parameter_rejected/main.omg`
- **machine operator spelling** (6): `tests/omega/fail/arithmetic/authored_requires_numeric_bound/main.omg`, `tests/omega/fail/collections/authored_index_guard_bound/main.omg`, `tests/omega/fail/slices/returned_record_custom_comparison_length/main.omg`
- **contract clause that projects a formal or result** (6): `tests/omega/fail/domains/array_constructor_result_input_mutation/main.omg`, `tests/omega/fail/facts/nested_three_level_record_field_bound_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/sret_aggregate_return_bound_through_call_result_rejected/main.omg`
- **declaration domain** (6): `tests/omega/fail/domains/private_arrival_unguarded_qualification_rejected/main.omg`, `tests/omega/pass/arithmetic/runtime_qualified_cast_decision_exit/main.omg`, `tests/omega/pass/domains/private_arrival_guarded_qualification_exit/main.omg`
- **type addr** (5): `tests/omega/fail/build/explicit_provider_readiness_private/backing/main.omg`, `tests/omega/fail/core/address_offset_zero_missing_fit/main.omg`, `tests/omega/fail/facts/domain_geometry_through_parameter_rejected/main.omg`
- **call compiler operation call** (5): `tests/omega/fail/facts/fixed_array_layout_geometry_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/slice_view_layout_geometry_through_parameter_rejected/main.omg`, `tests/omega/pass/facts/case_tag_discriminant_layout_through_call_result_exit/main.omg`
- **contract ranking witness** (5): `tests/omega/fail/facts/loop_branch_relaxation_ranking_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/state_transition_ranking_bound_through_parameter_rejected/main.omg`, `tests/omega/fail/termination/bitwise_payload_no_rank_progress/main.omg`
- **expression type parameter** (5): `tests/omega/fail/facts/open_const_implementation_specialization_agreement_rejected/main.omg`, `tests/omega/fail/generics/open_const_bare_binder_forwarding_mismatch/main.omg`, `tests/omega/fail/generics/open_const_selected_private_alias_sourcepoint_invalid/main.omg`
- **machine invokes** (5): `tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_local_rejected/main.omg`, `tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_field_rejected/main.omg`, `tests/omega/pass/facts/captured_storage_bounds_through_target_body_exit/main.omg`

### Agreement hand-check (10 fixtures)

Five `disagree` with route accepted / current rejected, five with route
rejected / current accepted. Harness excerpts:


Verifier read harness `program` lines (field `current=` vs aggregated `route=`). Patterns:

- **Route accept / current reject (fail):** entry `route=no-entry`, defect machine `route=outside` with specific construct, helper machine `route=accepted` — tally aggregate is `accepted`, so **disagree** is expected when the current checker rejects the whole package.
- **Route reject / current accept (pass):** entry judged at compilation root with unproved obligations while current checker accepted — **disagree** (route stricter on this shard slice).

## Hand-check group: disagree_route_accept_current_reject

### `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg`

```
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=machine:invalid	route=outside	Unsupported: contract term other than a formal or literal
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=machine:main	route=accepted	route_us=296	discharge_us=90	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:1 bytes 8..15	shift count in machine `invalid` ensures contract is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a defined-count po
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:3 bytes 95..97	shift count in machine `invalid` state `entry` return value is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a define
```

### `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg`

```
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:change	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:invalid	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:main	route=accepted	route_us=165	discharge_us=51	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	main.omg:4 bytes 148..149	exact arithmetic in machine `invalid` state `entry` return value may overflow `u32`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast to a larger type, constrain the operands' range (bound a parameter with a `requires` clause, or
```

### `tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg`

```
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=machine:midpoint	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=machine:main	route=accepted	route_us=162	discharge_us=52	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	main.omg:6 bytes 147..148	exact arithmetic in machine `midpoint` state `entry` return value may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast to a larger type, constrain the operands' range (bound a parameter with a `requires` clause, o
```

### `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg`

```
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=compilation	route=no-entry
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:finish	route=outside	Unsupported: machine type equations, owned data or conformances
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:CompletionProvider::complete	route=outside	Unsupported: type data with custody
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:main	route=outside	Unsupported: machine service reach
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:done	route=accepted	route_us=167	discharge_us=54	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:bad	route=accepted	route_us=161	discharge_us=134	reverify_us=22	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	no-span	cannot prove ensures contract for exit from finish at statement 3: proposition:fact:atomic_release_written<AtomicU64,u64>(shared word.value,ack.nonce); no exact incoming reference origin for word
```

### `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg`

```
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=compilation	route=no-entry
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:read	route=outside	Unsupported: type reference
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:Main::main	route=outside	Unsupported: machine attached data
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:good	route=accepted	route_us=167	discharge_us=54	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:bad	route=accepted	route_us=178	discharge_us=149	reverify_us=13	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	no-span	statement 6 mutates `main.cell.value` while local borrow `borrowed` is still active (borrowed at statement 1; its last use is at statement 7)
```

## Hand-check group: disagree_route_reject_current_accept

### `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`

```
program	tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg	current=accepted	current_us=3293408	root=compilation	route=rejected	route_us=9973	discharge_us=9333	reverify_us=1708	reverified=8	obligations=12	judged=10	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg`

```
program	tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg	current=accepted	current_us=3147686	root=compilation	route=rejected	route_us=1322	discharge_us=737	reverify_us=205	reverified=4	obligations=5	judged=4	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg`

```
program	tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg	current=accepted	current_us=3201758	root=compilation	route=rejected	route_us=1251	discharge_us=850	reverify_us=439	reverified=5	obligations=6	judged=4	core_refused=1	unproved=1	refused_other=0	needed_relation=0	needed_branch=2	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg`

```
program	tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg	current=accepted	current_us=3123052	root=compilation	route=rejected	route_us=9536	discharge_us=9071	reverify_us=7575	reverified=10	obligations=13	judged=10	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg`

```
program	tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg	current=accepted	current_us=3372018	root=compilation	route=rejected	route_us=2384	discharge_us=1714	reverify_us=1095	reverified=6	obligations=8	judged=6	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=5	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### Disagreements (route accepts, current rejects)

- `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg` first_refusal=`contract term other than a formal or literal`
- `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg` first_refusal=`machine type equations, owned data or conformances`
- `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg` first_refusal=`type reference`
- `tests/omega/fail/asm/instruction_disposition_preserved_premise/main.omg` first_refusal=`machine service reach`
- `tests/omega/fail/asm/instruction_effect_missing_write/main.omg` first_refusal=`machine service reach`
- `tests/omega/fail/asm/proposition_wrong_transparent_exit/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/asm/skipped_type_error/main.omg` first_refusal=`machine attached data`
- `tests/omega/fail/asm/wrong_label_exit_fact/main.omg` first_refusal=`machine attached data`
- `tests/omega/fail/borrow/mutated_selector_entry_actual_case/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/borrow/write_only_affine_replacement_reuse/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrow/write_only_exclusive_owned_wrapper_new_backing_conflict/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrow/write_only_reference_aggregate_replacement_detached_loan/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrows/conditional_returned_descriptor_local_escape/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/borrows/cyclic_arrival_duplicates_exclusive_reference/main.omg` first_refusal=`type data with custody`
- `tests/omega/fail/borrows/dynamic_byte_result_retains_second_candidate/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/borrows/guard_subject_alias_held_through_call/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrows/local_subslice_live_parent_read_operand/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrows/reference_aggregate_conditional_replacement_loan/main.omg` first_refusal=`parameter mutable parameter`
- `tests/omega/fail/borrows/runtime_slice_result_retains_backing/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/borrows/selected_reference_alias_local_escape/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/borrows/terminal_case_retained_observation/main.omg` first_refusal=`machine generic parameters`
- `tests/omega/fail/build/program_entry_returns_value/main.omg` first_refusal=``
- `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` first_refusal=``
- `tests/omega/fail/calls/discarded_call_result/main.omg` first_refusal=`machine attached data`
- `tests/omega/fail/calls/resultless_return_contract/main.omg` first_refusal=`machine attached data`
- `tests/omega/fail/capabilities/boundary_qualification_subject_rejected/main.omg` first_refusal=``
- `tests/omega/fail/collections/guarded_computed_index_overrun/main.omg` first_refusal=`type reference`
- `tests/omega/fail/collections/projected_range_call_shared_storage/main.omg` first_refusal=`parameter mutable parameter`
- … 163 more in TSV

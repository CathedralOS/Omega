# Route coverage shard 6 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
Host: `cursor`
Harness: `/agent/repos/omega/target/debug/examples/route_one_comparison`

## Build and run

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
```

Shard selection: sorted `main.omg` corpus list with 0-based index `i % 8 == 6`.
Fixtures in shard: 2174.

Execution: **one fixture per process, serial** (no parallel fixture workers on this host). `tools/build_slot.md` requires one heavy corpus-style job per test VM and caps concurrent agent load; serial `route_one_comparison` invocations satisfy that.

- pass/run: `timeout 300 /agent/repos/omega/target/debug/examples/route_one_comparison <fixture>`
- fail: `timeout 300 /agent/repos/omega/target/debug/examples/route_one_comparison --every-machine <fixture>`

## Totals

- judged (route accepted or rejected): 290
- route accepted: 220
- route rejected: 70
- outside: 1884
- timeout: 0
- error: 0
- agree: 89
- disagree: 201
- agreement n/a: 1884

### Meta outside buckets (not construct refusals)

- `meta:no-entry`: compilation root has no bound program entry; route does not elaborate the full Build (typical for fail/ without a single entry). Machine closures may still run with `--every-machine`.
- `meta:unreached`: current checker stopped before the typed frontier (`current=stopped`); route cannot run (`route=unreached`).

- `meta:no-entry`: 139
- `meta:unreached`: 140

### Timeouts

(none in this shard)

### Slow fixtures (owner-reviewed)

- `tests/omega/fail/build/installed_arrival_without_context/main.omg`: 281.863s, route `outside`, current `accepted` — completed under the 300s `timeout` wrapper (not killed).

## Top 10 first-refusal constructs (outside, excluding meta:*)

| construct / feature | blocks |
| --- | ---: |
| `machine service reach` | 353 |
| `parameter mutable parameter` | 278 |
| `machine generic parameters` | 168 |
| `type reference` | 146 |
| `declaration Build selection that is not a machine or a data type` | 112 |
| `type data with custody` | 106 |
| `machine attached data` | 81 |
| `type constrained` | 56 |
| `declaration const` | 31 |
| `type float` | 29 |

## First refusal ranks (outside, top 80)

| first_refusal | blocks | examples |
| --- | ---: | --- |
| `machine service reach` | 353 | `tests/omega/fail/borrows/byte_array_state_loan_shared_and_exclusive/main.omg`, `tests/omega/fail/borrows/clock_out_parameter_mutable_from_shared/main.omg`, `tests/omega/fail/borrows/independent_invocation_reference_result_escapes/main.omg` |
| `parameter mutable parameter` | 278 | `tests/omega/fail/arithmetic/unit_match_terminal_bad_cast/main.omg`, `tests/omega/fail/atomics/atomic_boolean_loaded_mutable_reference/main.omg`, `tests/omega/fail/borrow/returned_write_destination_carrier_conflict/main.omg` |
| `machine generic parameters` | 168 | `tests/omega/fail/borrow/bounded_reference_cannot_lend_unrestricted_mutable/main.omg`, `tests/omega/fail/borrow/scalar_reference_result_uses_returned_argument/main.omg`, `tests/omega/fail/borrows/applied_provider_output_requires_backing/main.omg` |
| `type reference` | 146 | `tests/omega/fail/arithmetic/collection_result_distinct_windows/main.omg`, `tests/omega/fail/arithmetic/fixed_array_length_checks_indexed_receiver/main.omg`, `tests/omega/fail/arithmetic/sparse_captured_utf_path_unbounded_index/main.omg` |
| `declaration Build selection that is not a machine or a data type` | 112 | `tests/omega/fail/build/interrupt_return_missing_gpr_restore/main.omg`, `tests/omega/fail/facts/alias_fact_transfer_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/boundary_provider_call_through_call_result_rejected/main.omg` |
| `type data with custody` | 106 | `tests/omega/fail/build/package_local_calling_copy/main.omg`, `tests/omega/fail/calls/native_code_layout_proof_only_rejected/main.omg`, `tests/omega/fail/contracts/returned_integer_operation_wrong_operand/main.omg` |
| `machine attached data` | 81 | `tests/omega/fail/arithmetic/divide_by_zero_rejected/main.omg`, `tests/omega/fail/arithmetic/narrowing_anonymous_expression_unproven/main.omg`, `tests/omega/fail/atomics/shared_receiver_plain_field_write/main.omg` |
| `type constrained` | 56 | `tests/omega/fail/arithmetic/anonymous_large_literal_requires_representable_destination/main.omg`, `tests/omega/fail/arithmetic/integer_multiple_remainder_wrapping/main.omg`, `tests/omega/fail/atomics/fence_does_not_identify_observations/main.omg` |
| `declaration const` | 31 | `tests/omega/fail/facts/aggregate_struct_return_abi_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/case_where_equality_data_predicate_through_const_name_rejected/main.omg`, `tests/omega/fail/facts/computed_constant_array_extent_through_match_join_rejected/main.omg` |
| `type float` | 29 | `tests/omega/fail/borrows/float_comparison_read_during_live_exclusive_loan/main.omg`, `tests/omega/fail/facts/core_float_representation_through_local_rejected/main.omg`, `tests/omega/fail/facts/crash_entry_ieee_view_through_call_result_rejected/main.omg` |
| `contract contracts on a state` | 28 | `tests/omega/fail/borrows/nested_byte_field_capture_breaks_utf8/main.omg`, `tests/omega/fail/build/copied_domain_foreign/main.omg`, `tests/omega/fail/build/uefi_entry_schema_accepted_wrong_convention/main.omg` |
| `type fixed array` | 27 | `tests/omega/fail/borrows/record_exclusive_view_fields_overlap/main.omg`, `tests/omega/fail/borrows/stored_reference_exclusive_array_copy/main.omg`, `tests/omega/fail/collections/stored_reference_array_sibling_extent/main.omg` |
| `machine type equations, owned data or conformances` | 23 | `tests/omega/fail/core/content_transform_nested_loss/main.omg`, `tests/omega/fail/core/independent_content_contract_borrowed_input/main.omg`, `tests/omega/fail/facts/content_reunion_through_parameter_rejected/main.omg` |
| `expression match on a scalar subject` | 17 | `tests/omega/fail/expressions/match_fractional_result/main.omg`, `tests/omega/fail/facts/completed_integer_carrier_bounds_through_match_join_rejected/main.omg`, `tests/omega/fail/facts/five_variant_enum_payload_bound_through_match_join_rejected/main.omg` |
| `type generic data` | 17 | `tests/omega/fail/contracts/generic_component_invalid_bound/main.omg`, `tests/omega/fail/facts/atomic_try_exchange_once/match_join/main.omg`, `tests/omega/fail/facts/representation_agreement_through_other_state_rejected/main.omg` |
| `machine assembly target` | 11 | `tests/omega/fail/asm/checked_asm_saved_match_distinct_subjects/main.omg`, `tests/omega/fail/asm/scratch_termination_rank_required/main.omg`, `tests/omega/fail/facts/checked_asm_add_carry_through_field_rejected/main.omg` |
| `contract term other than a formal or literal` | 10 | `tests/omega/fail/arithmetic/exact_left_shift_false_floor_rejected/main.omg`, `tests/omega/fail/facts/case_where_equality_data_predicate_unguarded_payload_rejected/main.omg`, `tests/omega/fail/facts/checked_asm_nominal_predicate_through_field_rejected/main.omg` |
| `machine suspends or blocks` | 10 | `tests/omega/fail/borrow/recursive_terminal_exclusive_alias/main.omg`, `tests/omega/fail/facts/domain_selected_cleanup_blocking_hook_rejected/main.omg`, `tests/omega/fail/facts/inline_member_helper_through_other_machine_rejected/main.omg` |
| `contract clause that is not a comparison or conjunction` | 9 | `tests/omega/fail/facts/crash_entry_nan_equality_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/projection_call_premise_through_other_state_rejected/main.omg`, `tests/omega/fail/facts/quotient_closed_literal_through_local_rejected/main.omg` |
| `machine invokes` | 9 | `tests/omega/fail/build/installed_arrival_without_context/main.omg`, `tests/omega/fail/facts/provider_contract_endpoint_guarded_crash_through_parameter_rejected/main.omg`, `tests/omega/fail/facts/provider_contract_endpoint_pending_call_through_other_state_rejected/main.omg` |
| `type generic application` | 8 | `tests/omega/fail/generics/runtime_index_carrier_array_mismatch/main.omg`, `tests/omega/fail/generics/runtime_index_carrier_metadata_mismatch/main.omg`, `tests/omega/fail/generics/runtime_index_carrier_nested_stale/main.omg` |
| `contract ranking witness` | 6 | `tests/omega/fail/facts/state_transition_ranking_bound_through_local_rejected/main.omg`, `tests/omega/fail/termination/endpoint_folded_into_payload/main.omg`, `tests/omega/fail/termination/runtime_recursive_guard_occurrence/main.omg` |
| `machine target selection` | 6 | `tests/omega/fail/domains/target_issuer_same_name_other_owner/main.omg`, `tests/omega/fail/facts/bounded_products_through_target_body_rejected/main.omg`, `tests/omega/fail/facts/fixed_array_twofiftysix_elem_index_bound_through_target_body_rejected/main.omg` |
| `contract clause that projects a formal or result` | 5 | `tests/omega/fail/domains/array_constructor_distinct_call_results/main.omg`, `tests/omega/fail/facts/nested_three_level_record_field_bound_through_call_result_rejected/main.omg`, `tests/omega/fail/facts/sret_aggregate_return_bound_through_match_join_rejected/main.omg` |
| `expression type parameter` | 5 | `tests/omega/fail/generics/open_const_completed_exit_wrong_return/main.omg`, `tests/omega/pass/facts/open_const_implementation_through_call_result_exit/main.omg`, `tests/omega/pass/generics/open_const_distinct_applications/main.omg` |
| `machine operator spelling` | 5 | `tests/omega/fail/core/scoped_integer_call_primitive_operator_requires_formation/main.omg`, `tests/omega/fail/facts/body_bearing_comparison_overwritten_field_result_rejected/main.omg`, `tests/omega/fail/operators/selected_crash_invocation/main.omg` |
| `machine supply mode other than a checked body` | 5 | `tests/omega/fail/proofs/accepted_axiom_engine_veto/main.omg`, `tests/omega/fail/targets/called_opaque_family_without_provider/main.omg`, `tests/omega/pass/effects/finite_requirement_block_acknowledged_exit/main.omg` |
| `call compiler operation call` | 4 | `tests/omega/fail/facts/slice_view_layout_geometry_through_local_rejected/main.omg`, `tests/omega/fail/facts/value_location_alignment_through_other_machine_rejected/main.omg`, `tests/omega/pass/facts/case_tag_discriminant_layout_through_match_join_exit/main.omg` |
| `contract domain membership` | 4 | `tests/omega/fail/domains/domain_alias_reports_atomic_requirement/main.omg`, `tests/omega/pass/domains/carrier_exact_same_carrier_exit/main.omg`, `tests/omega/pass/facts/domain_carrier_identity_through_local_exit/main.omg` |
| `call receiver, requirement or coercion call` | 3 | `tests/omega/fail/facts/dependency_type_qualified_call_through_local_rejected/main.omg`, `tests/omega/fail/float/primitive_comparison_no_named_helper_exposure/main.omg`, `tests/omega/pass/facts/dependency_type_qualified_call_through_local_exit/main.omg` |
| `contract disjunction` | 3 | `tests/omega/fail/calls/numeric_scalar_call_boolean_guard/main.omg`, `tests/omega/fail/proofs/completed_call_remainder_cast_bound/main.omg`, `tests/omega/pass/facts/nested_result_extent_through_field_exit/main.omg` |
| `contract result binding` | 3 | `tests/omega/fail/facts/outcome_structural_evidence_through_other_state_rejected/main.omg`, `tests/omega/pass/calls/outcome_specific_root_exit/main.omg`, `tests/omega/pass/facts/outcome_structural_evidence_through_match_join_exit/main.omg` |
| `declaration domain` | 3 | `tests/omega/fail/slices/predicate_scalar_store_invalid_membership/main.omg`, `tests/omega/pass/facts/domain_predicate_call_through_call_result_exit/main.omg`, `tests/omega/pass/facts/projection_receiver_call_through_local_exit/main.omg` |
| `incomplete` | 3 | `tests/omega/fail/facts/generic_comparison_body_through_field_rejected/main.omg`, `tests/omega/fail/proofs/conditional_boolean_return_is_not_first_arm/main.omg`, `tests/omega/pass/proofs/constant_boolean_return_join_exit/main.omg` |
| `type addr` | 3 | `tests/omega/fail/core/address_offset_named_wrong_carrier/main.omg`, `tests/omega/fail/facts/domain_geometry_through_local_rejected/main.omg`, `tests/omega/pass/facts/domain_geometry_through_field_exit/main.omg` |
| `typing literal landed type` | 3 | `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_other_machine_rejected/main.omg`, `tests/omega/fail/facts/unsigned_8bit_shift_right_bound_through_other_machine_rejected/main.omg`, `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_match_join_exit/main.omg` |
| `declaration trait` | 2 | `tests/omega/fail/facts/foreign_package_service_reach_through_other_machine_rejected/main.omg`, `tests/omega/pass/build/entry_schema_same_leaf_exit/main.omg` |
| `expression projected name path` | 2 | `tests/omega/pass/facts/resident_backing_coexisting_exit/main.omg`, `tests/omega/pass/proofs/declaration_view_call_requirement/main.omg` |
| `type dynamic trait` | 2 | `tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_other_machine_rejected/main.omg`, `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_other_machine_exit/main.omg` |
| `call operator` | 1 | `tests/omega/fail/float/named_boundary_transition_wrong_result/main.omg` |
| `contract named proposition` | 1 | `tests/omega/pass/asm/proposition_label_contracts_exit/main.omg` |
| `expression cast into a non-exact arithmetic domain` | 1 | `tests/omega/fail/arithmetic/policy_operator_zero_divisor_unproven/main.omg` |
| `expression match arm on a value pattern` | 1 | `tests/omega/pass/facts/default_domain_field_write_through_match_join_exit/main.omg` |
| `spelling name without a symbol` | 1 | `tests/omega/fail/control_flow/implicit_entry_parameter_capture/main.omg` |
| `statement assembly` | 1 | `tests/omega/pass/asm/checked_label_reverse_local_exit/main.omg` |
| `type data without a checked shape` | 1 | `tests/omega/fail/facts/bounded_activation_claim_split_through_parameter_rejected/main.omg` |
| `type nominal type` | 1 | `tests/omega/fail/generics/applied_implementation_qualifier_wrong_value/main.omg` |
| `typing argument types` | 1 | `tests/omega/fail/facts/nominal_declaration_owner_through_local/main.omg` |
## Hand verification (agreement vs current checker field)

Definition: `current_verdict` is `route_one_comparison`'s `current=` field (the ordinary checker at the typed frontier). `route_verdict` is the harness aggregate `route()` over all program roots (--every-machine on fail/). This is not the fixture tier and not the corpus ledger.

### Five route accepts, current rejects (soundness watch)

- `tests/omega/fail/arithmetic/bounded_call_unproven/main.omg`
  - `program	tests/omega/fail/arithmetic/bounded_call_unproven/main.omg	current=rejected	current_us=1305297	root=compilation	route=no-entry`
  - `program	tests/omega/fail/arithmetic/bounded_call_unproven/main.omg	current=rejected	current_us=1305297	root=machine:Main::main	route=outside	Unsupported: machine attached data`
  - `program	tests/omega/fail/arithmetic/bounded_call_unproven/main.omg	current=rejected	current_us=1305297	root=machine:wounded	route=accepted	route_us=211	discharge_us=64	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - Observation: current rejects the fixture; route aggregate is `accepted` because at least one machine closure accepted (often a minimal or auxiliary machine with no obligations judged).
- `tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg`
  - `program	tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg	current=rejected	current_us=1371846	root=compilation	route=no-entry`
  - `program	tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg	current=rejected	current_us=1371846	root=machine:scale	route=outside	Unsupported: machine generic parameters`
  - `program	tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg	current=rejected	current_us=1371846	root=machine:main	route=accepted	route_us=1051	discharge_us=848	reverify_us=455	reverified=3	obligations=3	judged=2	core_refused=1	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - `program	tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg	current=rejected	current_us=1371846	root=machine:done	route=accepted	route_us=61	discharge_us=2	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - Observation: current rejects the fixture; route aggregate is `accepted` because at least one machine closure accepted (often a minimal or auxiliary machine with no obligations judged).
- `tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=compilation	route=no-entry`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=machine:replace_offset	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=machine:advance	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=machine:inspect	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=machine:commit	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg	current=rejected	current_us=1316031	root=machine:done	route=accepted	route_us=232	discharge_us=81	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - Observation: current rejects the fixture; route aggregate is `accepted` because at least one machine closure accepted (often a minimal or auxiliary machine with no obligations judged).
- `tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg`
  - `program	tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg	current=rejected	current_us=1334695	root=compilation	route=no-entry`
  - `program	tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg	current=rejected	current_us=1334695	root=machine:Reader::advance	route=outside	Unsupported: machine attached data`
  - `program	tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg	current=rejected	current_us=1334695	root=machine:room	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg	current=rejected	current_us=1334695	root=machine:store	route=outside	Unsupported: parameter mutable parameter`
  - `program	tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg	current=rejected	current_us=1334695	root=machine:done	route=accepted	route_us=177	discharge_us=63	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - Observation: current rejects the fixture; route aggregate is `accepted` because at least one machine closure accepted (often a minimal or auxiliary machine with no obligations judged).
- `tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg`
  - `program	tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg	current=rejected	current_us=1228568	root=compilation	route=no-entry`
  - `program	tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg	current=rejected	current_us=1228568	root=machine:produce	route=accepted	route_us=231	discharge_us=64	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - `program	tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg	current=rejected	current_us=1228568	root=machine:bounded	route=accepted	route_us=209	discharge_us=161	reverify_us=34	reverified=1	obligations=2	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
  - `program	tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg	current=rejected	current_us=1228568	root=machine:main	route=outside	Unsupported: type constrained`
  - Observation: current rejects the fixture; route aggregate is `accepted` because at least one machine closure accepted (often a minimal or auxiliary machine with no obligations judged).

### Five route rejects, current accepts

- `tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg`
  - `program	tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg	current=accepted	current_us=3100707	root=compilation	route=rejected	route_us=4336	discharge_us=4024	reverify_us=3439	reverified=4	obligations=6	judged=4	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg`
  - `program	tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg	current=accepted	current_us=2980924	root=compilation	route=rejected	route_us=2537	discharge_us=1843	reverify_us=465	reverified=6	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg`
  - `program	tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg	current=accepted	current_us=3085864	root=compilation	route=rejected	route_us=2650	discharge_us=2058	reverify_us=619	reverified=8	obligations=13	judged=10	core_refused=0	unproved=3	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg`
  - `program	tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg	current=accepted	current_us=3942880	root=compilation	route=rejected	route_us=10085	discharge_us=9582	reverify_us=8125	reverified=7	obligations=8	judged=7	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=6	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg`
  - `program	tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg	current=accepted	current_us=3363409	root=compilation	route=rejected	route_us=2274	discharge_us=1559	reverify_us=1023	reverified=6	obligations=8	judged=6	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=5	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

## All disagreements

| fixture | tier | current | route | first_refusal |
| --- | --- | --- | --- | --- |
| `tests/omega/fail/arithmetic/bounded_call_unproven/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/arithmetic/chained_const_binder_without_ceiling/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/arithmetic/composite_bound_arrival_late_write/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/arithmetic/guarded_read_count_exceeds_target/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/arithmetic/implicit_call_result_policy_change_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/arithmetic/indexed_divisor_copied_selector_write/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/atomic_release_disposition_bypass/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/atomic_release_disposition_wrong_value/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/instruction_disposition_foreign_owner/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/instruction_effect_assertion_stale_operand/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/instruction_effect_unperformed_machine_ensure/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/asm/state_fact_written/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/exposed_selector_retires_entry_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/nested_aggregate_result_invalidated/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/signed_window_shared_mutation/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/unknown_selector_replacement_retires_entry_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/write_only_exclusive_replacement_old_child_conflict/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrow/write_only_nominal_cleanup_replacement/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/completed_selector_join_requires_all_arms/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/conditional_copied_slot_retains_parent_loan/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/dropped_reference_qualification_later_operand/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/dynamic_window_result_requires_bounds/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/mixed_record_shared_view_mutable_local_nonempty_range/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/read_positions_during_live_exclusive_loan/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/scalar_operand_consumes_reference_carrier_once/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/borrows/shared_call_hides_mutable_window_field/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/build/calling_policy_entry_premise/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/build/explicit_provider_scalar_completion/backing/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/calls/composed_index_case_selector_write_retires/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/calls/constructor_child_local_epochs_not_equal/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/calls/indexed_case_pool_transition_without_guard/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/calls/nested_call_footprint_retires_second_argument/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/calls/outcome_specific_nonmatching_scope/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/capabilities/native_qualified_slice_external_leaf_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/cases/constructor_subject_range_replaced_value/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/collections/non_integer_index_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/comptime/array_expression_unused_invalid_call_operand/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/call_write_retires_overlapping_guard/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/cyclic_policy_result_exceeds_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/nested_call_later_operand_clears_storage_fact/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/runtime_index_scalar_write_retires_payload_fact/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/saved_call_case_guarantee_replaced_by_call/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/contracts/target_family_unselected_write_retires_fact/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/address_partition_state_replacement/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/content_membership_return_adapter_weaker_guarantee/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/content_membership_return_parameter_guarantee/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/content_projected_duplicate_consumption/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/content_reunion_mixed_origin/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/core/resident_index_identity_mismatch_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/dependent/data_requires_upper_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/dependent/relational_loop_invariant_reassigned_index_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/guard_call_requires_established_membership/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/indexed_membership_rejects_stale_entry/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/module_domain_broad_import_hides_leaf/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/predicate_interval_guard_too_wide/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/qualified_explicit_buffer_write_only_replacement/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/routed_qualification_cast_wrong_family_instance/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/domains/scalar_domain_unauthorized_completed_result/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/drops/cleanup_owner_application_mismatch/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/drops/parent_cleanup_disjunction_not_conjunction/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/abstract_signature_call_generic_actual_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/abstract_signature_call_through_other_target_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/address_count_partition_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/affine_discard_root_cleanup_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/applied_requirement_indices_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/assumed_binding_input_authority_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/assumed_binding_result_bound_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/atomic_bounded_fetch/field/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/atomic_bounded_resident/const_name/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/body_bearing_comparison_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/call_result_bound_discharge_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/compound_index_guard_through_const_name_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/computed_record_window_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/copied_call_argument_snapshot_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/correlated_invariant_store_through_const_name_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/default_domain_field_write_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/descriptor_range_exclusive_loan_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/domain_predicate_call_through_const_name_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/erased_reference_contract_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/erased_reference_contract_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/exclusive_match_arm_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/exclusive_observation_exposure_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/float_comparison_route_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/float_range_delivery_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/float_result_equality_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/generic_requirement_statement_call_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/generic_requirement_statement_call_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/indexed_element_mut_loan_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/induction_call_guarantee_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/length_through_const_name_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/linear_owner_borrowed_projection_other_state/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/machine_tuple_dispatch_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/machine_tuple_dispatch_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_computed_root_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_effectful_subject_custody_through_other_state_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_join_place_read_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_loan_last_use_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_primitive_local_origin_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/match_runtime_view_extent_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/matched_field_windows_through_other_state_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/nested_result_extent_through_const_name_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/nominal_callback_reach_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/predicate_type_formation_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/projection_receiver_call_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/result_cleanup_guarantees_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_call_index_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_computed_result_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_contract_index_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_contract_index_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_mutating_operator_through_other_machine_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_ordinary_call_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/runtime_index_application_qualified_computed_through_field_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/saved_observation_snapshot_after_call_through_local_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/saved_observation_snapshot_through_parameter_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/scoped_scalar_initializer_through_call_result_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/scoped_scalar_initializer_through_target_body_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/selected_scalar_initializer_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_other_state_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/typed_run_nested_duplicate/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/unannotated_direct_boundary_call_through_match_join_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/facts/write_only_match_join_through_other_state_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/float/guarded_range_requires_builtin_boolean_wrapper/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/float/indexed_ieee_i64_upper_ceiling_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/float/math_minimum_identity_cannot_follow_maximum/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/float/nested_call_requirement_after_sibling_write/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/float/requirement_record_field_trapping_requires_coverage/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/captured_map_machine_contract_nominal_type/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/concrete_applied_argument_local_mismatch/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/contextual_static_machine_ambiguous_carried_result/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/contextual_static_machine_forwarded_evidence_mismatch/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/contextual_static_machine_returned_view_local_escape/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/cross_module_const_alias_shadowed_premise/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/generic_field_read_rigid_nested_mismatch/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/returned_generic_result_parameter_mismatch/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/runtime_value_recipe_evidence_mutation_stale/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/generics/static_callable_excess_local_type_argument/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/layouts/layout_contract_selected_equality/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/layouts/placed_external_nested_array_shared_write/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/layouts/placed_return_alternative_wrong_lifetime/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/memory/bump_allocator_place_into_occupied/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/memory/native_allocation_direct_issuance_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/memory/typed_run_backing_geometry/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/modules/array_extent_private_constant/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/modules/duplicate_imported_machine_overload_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/operators/named_unit_result_argument_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/ownership/registration_claim_cannot_reconstruct/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/ownership/selected_constructed_argument_double_consumption/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/assignment_alias_slot_retargeted/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/constructor_value_precedes_slot_replacement/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/declaration_view_call_wrong_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/machine_parameterized_runtime_data_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/proof_integer_embedding_reference/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/proof_integer_undefined_remainder/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/quotient_case_arm_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/quotient_missing_symmetry/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/replaced_scalar_cannot_reuse_prior_bound/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/ring_rearrange_unlicensed_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/scalar_field_snapshot_after_alias_store/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/scalar_field_snapshot_distinct_selectors/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/proofs/static_machine_result_wrong_field/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/providers/carry_external_issuance_unadmitted/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/providers/named_target_family_sibling_guarantee_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/recast/non_owning_erasure_preserves_projected_view/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/relevance/static_proof_borrow_requires_storage/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/slices/captured_length_after_descriptor_replacement/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/slices/rebound_slice_local_index_uses_new_extent/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/slices/returned_record_window_lengths_disagree/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/slices/stored_view_alias_replacement_bounds/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/slices/written_nested_subslice_loses_byte_predicate/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/targets/portable_family_private_result_fact_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/termination/bounded_distance_stalled_cursor_exit_state/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/termination/computed_result_rank_nondecreasing/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/termination/nested_helper_record_measure_result_equation_missing/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/termination/proof_joint_machine_cycle_nondecreasing/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/termination/rankless_family_member_reentry/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/text/borrowed_byte_snapshot_reseated_descriptor/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/traits/const_selector_declared_application/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/traits/exact_application_keeps_parameter_types/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/traits/requirement_application_guarantee_identity/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/traits/structural_identity_private_overwrite/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/fail/types/address_sum_past_model_bound_rejected/main.omg` | fail | rejected | accepted | `` |
| `tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/register_stack_arg_calling_plan_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/saturating_integer_arithmetic_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_field_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_division_remainder_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_shift_right_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |

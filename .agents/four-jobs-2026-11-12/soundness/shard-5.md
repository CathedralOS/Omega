# Soundness mutations shard 5 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

## Scope
- Fixtures in shard: **1130**
- Fixtures with ≥1 mutant run: **1054**
- `no-mutation` (fixture yielded zero mutants): **76**
- Mutant rows (deduped fixture×kind): **1067**
- Fixtures with >1 mutant kind in TSV: **4** (early harness tried two priorities before single-mut cap).
- First run logged **1052** checks; final deduped total **1067** = one row per attempted mutant kind.

## Bucket table

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 5 |
| `known-gap-q7-mut` | 69 |
| `disagreement-valid` | 121 |
| `discarded-valid` | 2 |
| `rejected-both` | 868 |
| `failed-run` | 2 |
| **Total mutant rows** | **1067** |
| Meta: `no-mutation` fixtures | 76 |

Validity is decided by **reading the mutant** (fix3-B). `route_rejection_hint` is recorded for `drop_requires` but does not select buckets.

**Confirmed holes:** 5
| kind | count |
| --- | ---: |
| `drop_requires` | 5 |
| accepted_by | count |
| --- | ---: |
| `current` | 5 |

## `drop_requires` — current accepts: route hints (not validity)

| Fixture | route | bucket | route_rejection_hint |
| --- | --- | --- | --- |
| `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg` | rejected | `disagreement-valid` | cannot prove requires contract for call consume_high from unsigned_bits: xor_result == 18446744073709551600 (needs fact `value == 18446744073709551600` here; establish it before the call via a prior c |
| `tests/omega/pass/borrows/runtime_bounded_and_shared_field_loans_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg` | outside | `confirmed-hole` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg` | outside | `confirmed-hole` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/collections/runtime_stored_reference_array_owned_record_contract_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/constants/owned_record_argument_snapshot_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/constraints/guarded_parameter_result_contract/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/dependent/case_where_zero_decided_first_case/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/dependent/declaration_predicate_requires_proved/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_borrowed_selected_cleanup_postcondition_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_empty_array_owner_cleanup_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_local_nominal_cleanup_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_nested_cleanup_postcondition_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/drops/runtime_shared_nominal_state_loan_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_applied_callback_forwarded_type_argument_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_applied_callback_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_applied_callback_type_bound_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_constant_initializer_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_constant_initializer_unexecuted_body_dependency_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/abstract_signature_call_through_other_state_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/affine_discard_root_cleanup_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/applied_requirement_indices_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/as_slice_views_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/body_bearing_comparison_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/borrowed_case_result_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/call_result_bound_discharge_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/captured_storage_bounds_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/captured_storage_bounds_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/case_payload_exclusive_loan_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/dependency_type_qualified_call_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg` | outside | `confirmed-hole` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/erased_reference_contract_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/exclusive_match_arm_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/exclusive_observation_exposure_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg` | outside | `confirmed-hole` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/float_range_delivery_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/generic_comparison_body_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/generic_comparison_body_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/ieee_float_ordered_comparison_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/ieee_float_ordered_comparison_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/induction_call_guarantee_through_other_machine_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/match_join_place_read_through_other_state_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/match_loan_last_use_through_other_machine_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/match_primitive_local_origin_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/match_reference_referent_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/match_runtime_view_extent_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/may_write_codec_argument_through_other_state_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg` | outside | `confirmed-hole` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/open_index_distributivity_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/positional_implementation_binder_through_other_state_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/progress_selector_nested_arrival_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/progress_selector_owned_arrival_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/progress_selector_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/projection_call_premise_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/projection_call_premise_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/qualified_field_capacity_through_call_result_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/qualified_field_capacity_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/range_fact_delivery_through_other_state_exit/main.omg` | accepted | `discarded-valid` | route=accepted (hint only; not used for validity) |
| `tests/omega/pass/facts/recast_parent_restoration_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/reflection_declaration_inspection_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/returned_indexed_linear_residual_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/returned_linear_residual_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/saturating_integer_arithmetic_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_other_machine_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/saved_observation_snapshot_through_parameter_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/selected_native_code_layout_through_other_machine_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_state_exit/main.omg` | rejected | `disagreement-valid` | exact arithmetic in machine `perform_shift` state `entry` local `diff` may overflow `i64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation) (no proof foun |
| `tests/omega/pass/facts/signed_exact_addition_bound_through_local_exit/main.omg` | accepted | `discarded-valid` | route=accepted (hint only; not used for validity) |
| `tests/omega/pass/facts/signed_exact_multiplication_bound_through_field_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/static_evidence_premise_through_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/typed_reflection_mutation_through_match_join_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_local_exit/main.omg` | rejected | `disagreement-valid` | cannot prove requires contract for call verify_xored from main: local_xored >= 40 && local_xored <= 63 (needs fact `xored >= 40 && xored <= 63` here; establish it before the call via a prior contract  |
| `tests/omega/pass/facts/unsigned_8bit_shift_right_bound_through_target_body_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/facts/write_only_match_join_through_local_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/float/authored_heterogeneous_float_operator/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/generics/const_index_active_case_predicates_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/generics/inferred_equation_computed_satisfaction_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/generics/inferred_equation_typed_const_const_name_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/generics/runtime_finite_recursive_reference_catalog_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/generics/runtime_machine_equation_selected_const_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/proofs/applied_dependent_const_requires_transitive_operand/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/proofs/nested_machine_inferred_requirement/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/proofs/numeric_state_contract_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/recast/write_footprint/local/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/slices/runtime_matched_window_coordinate_capture_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/terminal_psi/member_crash_contract_boundary/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/termination/progress_selector_owned_arrival_reference_boundary_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/pass/types/address_comparison_symbolic_bound_compile/main.omg` | no-entry | `disagreement-valid` | route=no-entry (hint only; not used for validity) |
| `tests/omega/pass/wire/reflection_active_mutation_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/run/domains/semantic_predicate_domain_admission/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/run/facts/double_indexed_rmw_through_local_traps/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |
| `tests/omega/run/proofs/embed_collection_length_exit/main.omg` | outside | `disagreement-valid` | route=outside (hint only; not used for validity) |

## Commands
```bash
cd /agent/repos/omega
python3 .agents/four-jobs-2026-11-12/soundness/reconcile_fix_shard5.py
```

## Hand verification: 5 confirmed-hole

### `drop_requires` — `tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: **current**
- Validity (read mutant): invalid: body uses fact not re-established at entry
- Obligation / quote: cells[cursor.index].value = sample(cursor); (needs `requires cursor.index >= 0 && cursor.index < cells.len`)
- Route hint (not used for validity): route=outside (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_collections_runtime_finite_element_view_call_write_exit_main_omg_9b8d41b651__drop_requires/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_collections_runtime_finite_element_view_call_write_exit_main_omg_9b8d41b651__drop_requires/main.omg	current=accepted	current_us=2897167	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### `drop_requires` — `tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: **current**
- Validity (read mutant): invalid: body uses fact not re-established at entry
- Obligation / quote: cells[cursor.index].value = value(cursor); (needs `requires cursor.index >= 0 && cursor.index < cells.len`)
- Route hint (not used for validity): route=outside (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_collections_runtime_refined_element_view_call_write_exit_main_omg_31dae98b08__drop_requires/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_collections_runtime_refined_element_view_call_write_exit_main_omg_31dae98b08__drop_requires/main.omg	current=accepted	current_us=2861871	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### `drop_requires` — `tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: **current**
- Validity (read mutant): invalid: body uses fact not re-established at entry
- Obligation / quote: holder.grid[i][j] = holder.grid[i][j] + 1; (needs `requires i <= 2`)
- Route hint (not used for validity): route=outside (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_double_indexed_rmw_through_field_exit_main_omg_04e24d9734__drop_requires/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_double_indexed_rmw_through_field_exit_main_omg_04e24d9734__drop_requires/main.omg	current=accepted	current_us=2879998	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### `drop_requires` — `tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: **current**
- Validity (read mutant): invalid: body uses fact not re-established at entry
- Obligation / quote: main.arr[index + 1] (needs `requires 0 <= index && index <= 3`)
- Route hint (not used for validity): route=outside (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_field_where_index_through_other_state_exit_main_omg_d9aa784ee5__drop_requires/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_field_where_index_through_other_state_exit_main_omg_d9aa784ee5__drop_requires/main.omg	current=accepted	current_us=2885272	root=compilation	route=outside	Unsupported: type reference

```

### `drop_requires` — `tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg`
- Bucket: `confirmed-hole`; accepted_by: **current**
- Validity (read mutant): invalid: body uses fact not re-established at entry
- Obligation / quote: let prefix: &mut [u8] = &mut buffer[0..end]; (needs `requires end <= buffer.len`)
- Route hint (not used for validity): route=outside (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_nested_result_extent_through_other_state_exit_main_omg_59252ddd66__drop_requires/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_nested_result_extent_through_other_state_exit_main_omg_59252ddd66__drop_requires/main.omg	current=accepted	current_us=7929724	root=compilation	route=outside	Unsupported: contract disjunction

```

## Hand verification: 5 discarded-valid

### `drop_requires` — `tests/omega/pass/facts/range_fact_delivery_through_other_state_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: **both**
- Validity (read mutant): valid: removed entry requires; body/callers still establish the fact
- Obligation / quote: 
- Route hint (not used for validity): route=accepted (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_range_fact_delivery_through_other_state_exit_main_omg_e762ef210e__drop_requires/main.omg; wrote_output=false
```
- route: **accepted**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_range_fact_delivery_through_other_state_exit_main_omg_e762ef210e__drop_requires/main.omg	current=accepted	current_us=2905859	root=compilation	route=accepted	route_us=2234	discharge_us=1560	reverify_us=750	reverified=7	obligations=7	judged=7	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	close
```

### `drop_requires` — `tests/omega/pass/facts/signed_exact_addition_bound_through_local_exit/main.omg`
- Bucket: `discarded-valid`; accepted_by: **both**
- Validity (read mutant): valid: removed entry requires; body/callers still establish the fact
- Obligation / quote: 
- Route hint (not used for validity): route=accepted (hint only; not used for validity)
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_signed_exact_addition_bound_through_local_exit_main_omg_c639dcd803__drop_requires/main.omg; wrote_output=false
```
- route: **accepted**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_signed_exact_addition_bound_through_local_exit_main_omg_c639dcd803__drop_requires/main.omg	current=accepted	current_us=3055864	root=compilation	route=accepted	route_us=25346	discharge_us=24760	reverify_us=22644	reverified=5	obligations=5	judged=5	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0
```

## Hand verification: 5 known-gap-q7-mut

### `remove_mut` — `tests/omega/pass/atomics/placed_atomic_load/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: **current**
- Validity (read mutant): invalid: writable use without mut binding
- Obligation / quote: binding `entries` line 6: `entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Route hint (not used for validity): 
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut/main.omg; wrote_output=false
```
- route: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut/main.omg	current=accepted	current_us=3775408	root=compilation	route=no-entry

```

### `remove_mut` — `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: **current**
- Validity (read mutant): invalid: writable use without mut binding
- Obligation / quote: binding `other` line 10: `let flag: bool = Integer::touch(&mut other, 9);`
- Route hint (not used for validity): 
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut/main.omg; wrote_output=false
```
- route: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut/main.omg	current=accepted	current_us=1421223	root=compilation	route=no-entry

```

### `remove_mut` — `tests/omega/pass/borrow/write_only_whole_scalar_replace/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: **current**
- Validity (read mutant): invalid: writable use without mut binding
- Obligation / quote: binding `destination` line 14: `forward(&write destination, 42);`
- Route hint (not used for validity): 
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut/main.omg; wrote_output=false
```
- route: **no-entry**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut/main.omg	current=accepted	current_us=1405538	root=compilation	route=no-entry

```

### `remove_mut` — `tests/omega/pass/borrows/copied_image_descriptor_unrelated_write/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: **current**
- Validity (read mutant): invalid: writable use without mut binding
- Obligation / quote: binding `counter` line 8: `let length: u64 = receive(span, tick(&mut counter));`
- Route hint (not used for validity): 
- `omega --check` exit 0
```text
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut/main.omg	current=accepted	current_us=2955423	root=compilation	route=outside	Unsupported: parameter mutable parameter

```

### `remove_mut` — `tests/omega/pass/borrows/pending_registration_write_after_completion/main.omg`
- Bucket: `known-gap-q7-mut`; accepted_by: **current**
- Validity (read mutant): invalid: writable use without mut binding
- Obligation / quote: binding `buffer` line 41: `buffer.value = 99;`
- Route hint (not used for validity): 
- `omega --check` exit 0
```text
compiled 30 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_pending_registration_write_after_completion_main_omg_55e4dda937__remove_mut/main.omg; wrote_output=false
```
- route: **outside**
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_pending_registration_write_after_completion_main_omg_55e4dda937__remove_mut/main.omg	current=accepted	current_us=3589585	root=compilation	route=outside	Unsupported: machine service reach

```

## Confirmed-hole entries

### `drop_requires` — `tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg`
- accepted_by: **current**
- invalid: body uses fact not re-established at entry
- Obligation: cells[cursor.index].value = sample(cursor); (needs `requires cursor.index >= 0 && cursor.index < cells.len`)
- Spec (wiki/spec/language/state_contracts.md → (Machine `requires`)): Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg`
- accepted_by: **current**
- invalid: body uses fact not re-established at entry
- Obligation: cells[cursor.index].value = value(cursor); (needs `requires cursor.index >= 0 && cursor.index < cells.len`)
- Spec (wiki/spec/language/state_contracts.md → (Machine `requires`)): Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg`
- accepted_by: **current**
- invalid: body uses fact not re-established at entry
- Obligation: holder.grid[i][j] = holder.grid[i][j] + 1; (needs `requires i <= 2`)
- Spec (wiki/spec/language/state_contracts.md → (Machine `requires`)): Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg`
- accepted_by: **current**
- invalid: body uses fact not re-established at entry
- Obligation: main.arr[index + 1] (needs `requires 0 <= index && index <= 3`)
- Spec (wiki/spec/language/state_contracts.md → (Machine `requires`)): Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg`
- accepted_by: **current**
- invalid: body uses fact not re-established at entry
- Obligation: let prefix: &mut [u8] = &mut buffer[0..end]; (needs `requires end <= buffer.len`)
- Spec (wiki/spec/language/state_contracts.md → (Machine `requires`)): Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

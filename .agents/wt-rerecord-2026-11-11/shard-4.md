# Shard 4 — wt re-record (2026-11-11)

## Pin and slice

- **Pin:** `3bbe5c131a15666f68a700f499783f04910e8143` (detached checkout, clean tree)
- **Branch:** `work/wt-rerecord-shard-4`
- **Host:** linux/x86_64, 4 CPUs
- **Corpus jobs:** `OMEGA_CORPUS_JOBS=2` (one corpus run at a time; `nice -n 19`)
- **Shard rows:** `/tmp/shard3` from `split -n l/6` on:
  ```bash
  grep -E '^(pass|fail|run)/.*@[0-9a-f]{12}\+wt' tests/omega/corpus_outcomes.txt \
    | cut -d' ' -f1 | LC_ALL=C sort -u
  ```
- **Row count:** 215 (113 `fail/`, 102 `pass/`)

## Exact row list

```
fail/generics/runtime_value_recipe_const_runtime_rejected
fail/generics/runtime_value_recipe_prior_write_overflow
fail/generics/runtime_value_recipe_typed_narrow_overflow
fail/generics/runtime_value_recipe_unused_overflow
fail/generics/selected_boolean_policy_applied_contract_wrong_argument
fail/generics/selected_operator_preserves_blocking_ceiling
fail/generics/static_dispatch_private_ensures_explicit_requirement
fail/generics/static_dispatch_private_ensures_nested_generic
fail/generics/static_dispatch_private_ensures_through_generic
fail/generics/typed_const_probe_trapping_extent
fail/generics/unused_selected_value_unproved_where
fail/inline_asm/asm_msr_destination_predicate
fail/inline_asm/asm_system_register_destination_predicate
fail/inline_asm/asm_where_retired
fail/layouts/fixed_layout_dormant_invalid_policy_rejected
fail/layouts/fixed_layout_finite_dynamic_policy_rejected
fail/layouts/fixed_layout_overridden_invalid_policy_rejected
fail/layouts/generic_requirement_implementation_integer_at_requires_fit
fail/layouts/layout_contract_selected_equality
fail/layouts/layout_query_captured_result_join_unknown
fail/layouts/private_callback_fragments_container_outside
fail/layouts/private_callback_fragments_destination_overlap
fail/layouts/private_callback_fragments_incomplete_source
fail/layouts/private_callback_slot_wrong_layout
fail/layouts/resident_external_array_requires_restoration
fail/layouts/resident_extraction_requires_plan
fail/layouts/resident_initialization_requires_plan
fail/layouts/resident_selected_lifecycle_requires_plan
fail/layouts/resident_shared_view_cannot_write
fail/machines/private_frontier_guarded_counter_cycle_invalid
fail/machines/private_where_store_through_cycle_control
fail/memory/bump_allocator_borrowed_oversized
fail/memory/bump_allocator_cast_minted_vacant
fail/memory/bump_allocator_inflated_capacity
fail/memory/bump_allocator_oversized_partition
fail/memory/bump_allocator_projected_claim_duplicate
fail/memory/bump_allocator_reset_missing_prefix
fail/memory/bump_allocator_reset_with_live_resident
fail/memory/bump_allocator_retired_extent_unavailable
fail/memory/bump_allocator_stale_capacity
fail/memory/bump_allocator_truncation_beyond_buffer
fail/modules/duplicate_imported_machine_overload_rejected
fail/modules/literal_constant_array_range_rejected
fail/modules/runtime_fixed_array_index
fail/modules/selected_constant_array_alias_range_rejected
fail/operators/domain_qualified_boundary_operator_unestablished_literal
fail/operators/range_extent_requires_builtin_equality
fail/operators/range_extent_requires_builtin_subtraction
fail/ownership/registration_claim_cannot_duplicate_cases
fail/parse/retired_trait_operator_requirement
fail/proofs/borrowed_boolean_policy_changed_storage
fail/proofs/borrowed_boolean_policy_wrong_validator
fail/proofs/declaration_view_call_stale_field
fail/proofs/exact_widening_without_source_bound
fail/proofs/numeric_state_selected_operator
fail/proofs/outcome_selector_on_record_result
fail/proofs/result_index_abstract_bound_operator
fail/proofs/result_index_lexical_operator
fail/proofs/selected_boolean_policy_wrong_validator
fail/proofs/specification_case_exclusion_ambiguous_payload
fail/providers/assumed_result_bound_target_body_rejected
fail/providers/checked_view_provider_wrong_extent
fail/providers/finite_requirement_forwarded_const_without_predicate
fail/providers/foreign_address_fixed_carrier_mismatch
fail/providers/provider_receiver_applied_trait_rejected
fail/providers/provider_receiver_field_rejected
fail/providers/provider_receiver_generic_argument_rejected
fail/providers/provider_receiver_parameter_rejected
fail/providers/provider_receiver_requirement_signature_rejected
fail/providers/raw_boundary_trait_ambiguous_whole_supply
fail/range/element_view_field_domain_occurrence
fail/range/refined_element_view_outside_field_domain
fail/ranges/correlated_store_all_predecessors
fail/ranges/correlated_store_rhs_retires_bound
fail/ranges/correlated_store_selector_retires_bound
fail/ranges/sum_payload_non_case_guard_unproven
fail/recast/recast_mut_record_where_write_out_of_range
fail/recast/recast_slice_where_invalid_indexed_store
fail/targets/target_body_call_to_unimported_std_leaf_named
fail/termination/computed_measure_non_monotone_body
fail/termination/declared_measure_hidden_overflow_rejected
fail/termination/forwarding_state_in_ranked_loop
fail/termination/nested_helper_field_rank_subject_not_decreasing
fail/termination/nested_machine_distance_missing_endpoint_bound
fail/termination/nested_machine_distance_missing_index_bound
fail/traits/computed_static_carrier
fail/traits/computed_static_effect
fail/traits/computed_static_fractional
fail/traits/computed_static_overflow
fail/traits/computed_static_runtime
fail/traits/dyn_finite_family_narrowed_provider
fail/traits/dyn_finite_family_non_member
fail/traits/dyn_finite_family_wrong_tuple
fail/traits/dyn_selected_reach_callee_rebind_rejected
fail/traits/dyn_selected_reach_public_helper_rejected
fail/traits/dyn_selected_reach_rebound_selector_rejected
fail/traits/dyn_selected_reach_recursive_rejected
fail/traits/dyn_selected_reach_two_contexts_rejected
fail/traits/dyn_selected_reach_unknown_parameter_rejected
fail/traits/dyn_selector_leading_crash_uncovered
fail/traits/dyn_slot_metadata_unqualified
fail/traits/dyn_statement_call_operand_positions_rejected
fail/traits/dyn_suspending_borrowed_parameter_precondition
fail/traits/dyn_suspending_unit_call_precondition
fail/traits/family_carrier_pack_dependent_storage_rejected
fail/traits/finite_family_machine_facts_do_not_bind_state
fail/traits/finite_family_mutable_state_subject
fail/traits/finite_family_sibling_state_facts
fail/traits/inherited_nominal_requirement_conflicting_parents
fail/traits/inherited_nominal_requirement_overload
fail/traits/inherited_nominal_requirement_refinement_missing
fail/traits/inherited_nominal_requirement_wrong_owner
fail/types/address_sum_past_model_bound_rejected
pass/arithmetic/runtime_composite_bound_arrivals_exit
pass/arrays/computed_fixed_array_length_exit
pass/borrows/copied_scalar_field_with_restricted_peer
pass/borrows/generic_record_slice_referent_exit
pass/borrows/runtime_reference_result_alias_case_payload_exit
pass/borrows/runtime_reference_result_case_payload_exit
pass/build/provider_package_default_exit
pass/build/selected_const_array_length_exit
pass/calls/attached_parameter_overload_bound
pass/calls/attached_parameter_overload_exit
pass/calls/converted_call_value_private_arrival_exit
pass/calls/runtime_guarded_reference_return_before_crash_exit
pass/calls/runtime_guarded_reference_return_crash
pass/calls/runtime_guarded_reference_return_machine_crash
pass/comptime/const_recursive_counter_survives_unrelated_writes
pass/comptime/runtime_const_array_call_arguments_exit
pass/concurrency/host_boundary_readiness_exit
pass/contracts/normal_return_contextual_call_arguments_exit
pass/contracts/returned_case_geometry_domain_exit
pass/contracts/runtime_float_case_crash_guard_exit
pass/control_flow/arm_pattern_rest_optout_exit
pass/control_flow/case_pattern_rename_waive_exit
pass/control_flow/pattern_arm_guarded_index_argument_exit
pass/control_flow/qualified_case_payload_copy_exit
pass/core/integer_query_public_refinement_exit
pass/core/integer_query_reference_equation_exit
pass/data/runtime_case_payload_call_result_window_exit
pass/dependent/nested_case_equations_two_applications_exit
pass/domains/applied_generic_carrier
pass/domains/bodyless_owner_establishment
pass/domains/borrow_of_cast_exit
pass/domains/carried_indexed_payload_qualification
pass/domains/carried_nested_payload_qualification
pass/domains/carried_runtime_index_payload_qualification
pass/domains/case_payload_widened_call_result_exit
pass/domains/domain_established_by
pass/domains/domain_pattern_match_bound_generic_equation_same_carrier_exit
pass/domains/domain_pattern_match_bound_generic_reference_equation_exit
pass/domains/domain_pattern_match_bound_generic_transitive_equation_exit
pass/domains/domain_pattern_match_bound_generic_write_forward_exit
pass/domains/explicit_domain_erasure
pass/domains/indexed_domain_known_disjoint_call_exit
pass/domains/inherited_requirement_issuer_exit
pass/domains/mutable_domain_parameter_exit
pass/domains/private_arrival_guarded_qualification_exit
pass/domains/refinement_chain_routed_provenance
pass/effects/finite_requirement_block_acknowledged_exit
pass/effects/opaque_leaf_inherits_requirement_contract
pass/facts/abstract_signature_call_generic_actual_through_call_result_exit
pass/facts/abstract_signature_call_generic_actual_through_const_name_exit
pass/facts/abstract_signature_call_generic_actual_through_field_exit
pass/facts/abstract_signature_call_generic_actual_through_local_exit
pass/facts/abstract_signature_call_generic_actual_through_match_join_exit
pass/facts/abstract_signature_call_generic_actual_through_other_machine_exit
pass/facts/abstract_signature_call_generic_actual_through_other_state_exit
pass/facts/abstract_signature_call_generic_actual_through_parameter_exit
pass/facts/abstract_signature_call_generic_actual_through_target_body_exit
pass/facts/applied_requirement_indices_through_call_result_exit
pass/facts/applied_requirement_indices_through_const_name_exit
pass/facts/applied_requirement_indices_through_field_exit
pass/facts/applied_requirement_indices_through_local_exit
pass/facts/applied_requirement_indices_through_other_machine_exit
pass/facts/applied_requirement_indices_through_other_state_exit
pass/facts/applied_requirement_indices_through_parameter_exit
pass/facts/assumed_binding_input_authority_through_target_body_exit
pass/facts/assumed_binding_result_bound_through_target_body_exit
pass/facts/boundary_zero_origin_through_target_body_exit
pass/facts/case_type_equation_recovery_through_call_result_exit
pass/facts/case_type_equation_recovery_through_const_name_exit
pass/facts/case_type_equation_recovery_through_field_exit
pass/facts/case_type_equation_recovery_through_local_exit
pass/facts/case_type_equation_recovery_through_match_join_exit
pass/facts/case_type_equation_recovery_through_other_machine_exit
pass/facts/case_type_equation_recovery_through_other_state_exit
pass/facts/case_type_equation_recovery_through_parameter_exit
pass/facts/case_type_equation_recovery_through_target_body_exit
pass/facts/crash_entry_ieee_view_through_call_result_exit
pass/facts/crash_entry_ieee_view_through_const_name_exit
pass/facts/crash_entry_ieee_view_through_field_exit
pass/facts/crash_entry_ieee_view_through_local_exit
pass/facts/crash_entry_ieee_view_through_match_join_exit
pass/facts/crash_entry_ieee_view_through_other_machine_exit
pass/facts/crash_entry_ieee_view_through_other_state_exit
pass/facts/crash_entry_ieee_view_through_parameter_exit
pass/facts/crash_entry_ieee_view_through_target_body_exit
pass/facts/crash_entry_nan_equality_through_const_name_exit
pass/facts/crash_entry_nan_equality_through_target_body_exit
pass/facts/crash_entry_nonempty_path_through_target_body_exit
pass/facts/crash_entry_representation_view_through_target_body_exit
pass/facts/crash_entry_signed_zero_through_target_body_exit
pass/facts/data_where_standing_mutable_recast_through_call_result_exit
pass/facts/data_where_standing_nested_borrowed_field_incoming_compatibility
pass/facts/data_where_standing_recast_owner_restoration
pass/facts/domain_predicate_call_through_call_result_exit
pass/facts/domain_predicate_call_through_const_name_exit
pass/facts/domain_predicate_call_through_field_exit
pass/facts/domain_predicate_call_through_local_exit
pass/facts/domain_predicate_call_through_match_join_exit
pass/facts/domain_predicate_call_through_other_machine_exit
pass/facts/domain_predicate_call_through_other_state_exit
pass/facts/domain_predicate_call_through_parameter_exit
pass/facts/domain_predicate_call_through_target_body_exit
```

(Full list also at commit: `grep` slice row file in repo if needed.)

## Commands

```bash
git checkout 3bbe5c131a15666f68a700f499783f04910e8143
git checkout -b work/wt-rerecord-shard-4
bash tools/agent_setup.sh && mbx xtask doctor
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null

# Batch 1 (200 fixtures) — filter comma-joined from head -200 of shard list
OMEGA_CORPUS_JOBS=2 nice -n 19 mbx xtask corpus --record --filter '<batch1.csv>' < /dev/null

# Batch 2 (15 fixtures)
OMEGA_CORPUS_JOBS=2 nice -n 19 mbx xtask corpus --record --filter '<batch2.csv>' < /dev/null
```

After each batch: restored prior `millis` on rows whose status, expected fragment, facts, and diagnostics matched the pin (per `tools/testing.md` § Attributing a move). No `timeout` rows introduced.

## Outcomes (vs pin `3bbe5c131a15`)

| | Count |
| --- | --- |
| Re-recorded (measured) | 214 |
| Unchanged (stamp/millis only or identical) | 184 |
| Changed (status and/or diagnostics) | 30 |
| Left unsettled | 1 |

### Changed rows by cause

**Attribution (all changed rows):** Ledger-only re-measurement on a clean checkout of the pin; prior `+wt` stamps are not commit-attributable. Recorded outcomes are what the pin compiler produces on this host (`linux_x86_64`). Spot-check: `mbx xtask corpus --filter pass/facts/abstract_signature_call_generic_actual_through_local_exit` → clean after record (not flaky).

1. **fail: expected fragment lost → `unexpected` (13)**  
   Dirty `expected` pins did not match the pin compiler's reject site/message (e.g. asm, machines, range, recast, traits/dyn_finite_family_narrowed_provider, runtime_value_recipe_prior_write_overflow).

2. **pass/facts/abstract_signature*: `checked` → `rejected` (9)**  
   `check-target:macos_arm64` rows; pin compiler rejects with `regressed-from` token carried from prior golden.

3. **fail: same `rejected` tier, diagnostic shift (6)**  
   Still fail at pin; pinned message/location updated (layouts, bump_allocator×2, traits dyn_selector/dyn_slot, types/address_sum).

4. **fail/generics/selected_boolean_policy_applied_contract_wrong_argument (1)**  
   Rejection unchanged; diagnostic now prefixed with `cannot prove requires contract` (pin wording).

5. **pass/build/selected_const_array_length_exit (1)**  
   Already `rejected` with `regressed-from`; restamped at pin (timing/message stable aside from stamp).

## Unsettled

| Row | Reason |
| --- | --- |
| `fail/termination/forwarding_state_in_ranked_loop` | No `tests/omega/fail/termination/forwarding_state_in_ranked_loop/` tree at pin; `mbx xtask corpus --record --filter fail/termination/forwarding_state_in_ranked_loop` → `selection matches no fixtures`. Row still carries `@a9ae30e1c334+wt` in `corpus_outcomes.txt`. |

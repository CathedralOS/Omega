# Ledger regression triage (2026-11-11)

The sweep at `759c80404428` found 1,296 corpus rows that matched the ledger on
paper but failed the compiler at that commit. This report re-runs those rows on
current `main` and classifies them for repair queueing only (no ledger or
compiler edits).

## Counts

| Class | Count |
| --- | ---: |
| fixed on main | 347 |
| still broken | 941 |
| host only | 8 |
| total sweep rows | 1296 |

## Host-only (timeout)

These rows timed out under `mbx xtask corpus` (1800s batch cap) on this
host but are not necessarily still broken; re-check with direct
`corpus_runner` and lower parallelism if needed.

- `fail/build/installed_arrival_without_context`
- `fail/build/installed_arrival_wrong_controller`
- `pass/calls/runtime_composed_index_case_pool_transition_exit`
- `pass/calls/runtime_indexed_case_pool_transition_exit`
- `pass/filesystem/create_dir_all_symlink_kind_exit`
- `pass/filesystem/cross_linux_value_syscalls`
- `pass/filesystem/dir_walk_wrappers_exit`
- `pass/filesystem/runtime_portable_unknown_directory_kinds_exit`

## Fail fixtures still accepted on main

These `fail/` programs still reach `checked` or `built` on `main` while the
ledger records them as rejected (soundness gaps).

Count: **34**

```bash
cd /agent/repos/omega
FILTER=$(paste -sd, /tmp/accepted_fail_34.txt)
nice -n 19 mbx xtask corpus --filter "$FILTER" < /dev/null
```

Still accepted:

- `fail/borrows/dynamic_store_then_fixed_retains_replacement`
- `fail/borrows/reference_carrier_store_while_shared`
- `fail/borrows/region_store_live_sibling_reference`
- `fail/boundary/named_foreign_provider_wrong_table`
- `fail/build/component_assumption_malformed`
- `fail/build/component_assumption_short`
- `fail/calls/copied_argument_routed_ensures_cannot_restore_callee_mutated_source`
- `fail/calls/copied_argument_routed_ensures_cannot_restore_mutated_source`
- `fail/calls/copied_constructor_child_ensures_cannot_restore_mutated_source`
- `fail/capabilities/cyclic_synchronous_invocation`
- `fail/collections/wrapping_loop_increment_bound`
- `fail/constraints/computed_local_value_invalidated`
- `fail/contracts/scalar_copy_guard_after_source_restore`
- `fail/core/scoped_integer_call_guarantees_cannot_form_cycle`
- `fail/domains/bodyless_routed_primitive_receiver_requires_issuer`
- `fail/domains/domain_pattern_match_bound_generic_unconstrained_rejected`
- `fail/domains/domain_pattern_match_bound_generic_unrelated_equation_rejected`
- `fail/facts/data_where_standing_mutable_recast_through_other_machine_rejected`
- `fail/facts/data_where_standing_mutable_recast_through_other_state_rejected`
- `fail/facts/data_where_standing_mutable_recast_through_parameter_rejected`
- `fail/facts/data_where_standing_mutable_recast_through_target_body_rejected`
- `fail/layouts/direct_unselected_ordinary_layout_contract`
- `fail/layouts/fixed_layout_policy_borrowed_content`
- `fail/layouts/placed_reference_results/stale_after_write`
- `fail/layouts/unselected_target_callback_layout_underflow`
- `fail/layouts/unselected_target_helper_layout_underflow`
- `fail/providers/inherited_receiver_same_calling_owner_conflict`
- `fail/providers/named_provider_wrong_requirement_owner`
- `fail/recast/borrowed_byte_collection_predicate`
- `fail/recast/pending_layout_view_call_requires_establishment`
- `fail/termination/bounded_distance_moving_call_bound`
- `fail/termination/private_stored_reference_rank_no_ambient_helper_bound`
- `fail/traits/structural_identity_executable_equality`
- `fail/wire/codec_reference_array_overlapping`

## Clusters still broken

Largest clusters first (member sets deduplicated). **Rows** counts sweep members
still worse than the ledger on measured `main`. **Breaking commit** uses
witness-only checkpoint bisect from `759c80404428`, propagated by sweep
diagnostic prefix when a witness was not bisected directly.

| Rows | Kind | Diagnostic on main | Witness | Breaking commit |
| ---: | --- | --- | --- | --- |
| 70 | fail wrong reason | selected map disagrees with its supplying evidence bound | `fail/facts/open_array_const_implementation_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/open_array_const_implementation_through_call_result_rejected` `fail/facts/open_array_const_implementation_through_const_name_rejected` `fail/facts/open_array_const_implementation_through_field_rejected` … (+67 more)

| 70 | pass/run rejected | terminal-artifact production failed: Lowering(Unsupported("retained-borrow callable lost its sch | `pass/facts/foreign_nested_record_backing_call_result` | `e28ae5cbd977057069e496bd26ed226d5f0ab156` |
Members: `fail/facts/foreign_nested_record_backing_call_result` `fail/facts/foreign_nested_record_backing_const_name` `fail/facts/foreign_nested_record_backing_field` … (+67 more)

| 38 | pass/run rejected | cannot establish assignment invariants: partial write enclosure has no declared type | `pass/borrows/runtime_reassigned_view_return_exit` | `e28ae5cbd977057069e496bd26ed226d5f0ab156` |
Members: `fail/facts/descriptor_range_exclusive_loan_through_call_result_rejected` `fail/facts/descriptor_range_exclusive_loan_through_const_name_rejected` `fail/facts/descriptor_range_exclusive_loan_through_field_rejected` … (+35 more)

| 33 | fail wrong reason | cannot complete return from machine `corrupt` state `entry`: this write left the `requires` fact | `fail/arithmetic/widened_result_predicate_invalid_write` | `e28ae5cbd977057069e496bd26ed226d5f0ab156` |
Members: `fail/arithmetic/widened_result_predicate_invalid_write` `fail/borrows/indexed_exclusive_primitive_loan_preserves_field_invariant` `fail/borrows/reference_record_field_predicate_invalid_store` … (+30 more)

| 27 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity | `fail/float/state_constructor_ieee_reflexivity` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/float/state_constructor_ieee_reflexivity` `fail/proofs/accepted_axiom_engine_veto` `fail/proofs/citation_requires_bearing_rejected` … (+24 more)

| 25 | pass/run rejected | machine `firmware_entry` state `entry` statement 2: stored call has no complete statement-succes | `pass/build/uefi_image_residence_observation` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/applied_provider_output_requires_backing` `fail/borrows/copied_active_case_snapshot_retains_backing` `fail/borrows/nested_machine_unit_output_local_escape_rejected` … (+22 more)

| 22 | pass/run rejected | no provider is selected for boundary requirement `Sink::write`, which the program entry reaches  | `pass/borrow/runtime_write_only_boundary_satisfier_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/build/native_table_non_address_field` `fail/calls/callback_shared_program_excluded_trap` `fail/calls/foreign_applied_callback_requires_selected_entry_plan` … (+19 more)

| 18 | fail wrong reason | source constructor predicate derivation failed: Kernel(UnboundLevelParameter { index: 0, arity:  | `fail/facts/source_indexed_type_parameter_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/source_indexed_type_parameter_through_call_result_rejected` `fail/facts/source_indexed_type_parameter_through_const_name_rejected` `fail/facts/source_indexed_type_parameter_through_field_rejected` … (+15 more)

| 17 | fail wrong reason | machine `bounded` cannot prove ensures contract proof fact `result >= -9223372036854775806` on t | `fail/arithmetic/remainder_postcondition_signed_minimum_underbound_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/remainder_postcondition_signed_minimum_underbound_rejected` `fail/contracts/recursive_result_bound_changed_actual` `fail/facts/signed_8bit_exact_subtraction_bound_through_call_result_rejected` … (+14 more)

| 16 | pass/run rejected | statement 2 creates local borrow `leading` while local borrow `entry` is still active (borrowed  | `pass/facts/computed_record_window_through_target_body_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/computed_record_window_through_target_body_exit` `pass/facts/may_write_computed_receiver_through_call_result_exit` `pass/facts/may_write_computed_receiver_through_const_name_exit` … (+13 more)

| 15 | pass/run rejected | native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(2 | `pass/build/default_component_abi_extents` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/build/default_component_abi_extents` `pass/build/native_component_case_cycle` `pass/build/native_component_case_loan_cycle` … (+12 more)

| 14 | fail wrong reason | selected map disagrees with its supplying evidence bound | `fail/facts/inferred_array_equation_call_mixed_binders_evidence_application_mismatch_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/inferred_array_equation_call_mixed_binders_evidence_application_mismatch_rejected` `fail/facts/static_callback_reach_captured_shadow` `fail/generics/cross_module_const_alias_shadowed_evidence` … (+11 more)

| 13 | pass/run rejected | cannot prove ensures contract for exit from whole_view at statement 0: result.len == Count | `pass/facts/as_slice_views_generic_offset_selector_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/as_slice_views_generic_offset_selector_exit` `pass/facts/as_slice_views_generic_root_exit` `pass/facts/as_slice_views_generic_shadowed_extent_exit` … (+10 more)

| 13 | fail wrong reason | exact arithmetic in machine `pick` state `entry` return value may overflow `u64`: the operands a | `fail/contracts/ruled_out_arm_value_does_not_reach_join` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/contracts/ruled_out_arm_value_does_not_reach_join` `fail/core/numeric_widened_range_shift_overflow_rejected` `fail/facts/write_only_borrow_struct_destination_through_call_result_rejected` … (+10 more)

| 10 | fail wrong reason | normal call `fill` (Handle { arena_index: 13523, generation: 1 }) in machine `select` state `ent | `fail/facts/foreign_reference_result_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/foreign_reference_result_through_call_result_rejected` `fail/facts/foreign_reference_result_through_const_name_rejected` `fail/facts/foreign_reference_result_through_field_rejected` `fail/facts/foreign_reference_result_through_local_rejected` `fail/facts/foreign_reference_result_through_match_join_rejected` `fail/facts/foreign_reference_result_through_other_machine_rejected` `fail/facts/foreign_reference_result_through_other_state_rejected` `fail/facts/foreign_reference_result_through_parameter_rejected` `fail/facts/foreign_reference_result_through_target_body_rejected` `pass/facts/fixed_layout_byte_roundtrip_through_target_body_exit`

| 10 | pass/run rejected | cannot prove ensures contract for exit from from_bits at statement 2: to_bits(result) == bits | `pass/facts/core_float_representation_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/core_float_representation_through_const_name_rejected` `pass/facts/core_float_representation_through_call_result_exit` `pass/facts/core_float_representation_through_const_name_exit` `pass/facts/core_float_representation_through_field_exit` `pass/facts/core_float_representation_through_local_exit` `pass/facts/core_float_representation_through_match_join_exit` `pass/facts/core_float_representation_through_other_machine_exit` `pass/facts/core_float_representation_through_other_state_exit` `pass/facts/core_float_representation_through_parameter_exit` `pass/facts/core_float_representation_through_target_body_exit`

| 9 | pass/run rejected | statement 3 mutates `packet.first.counter` while local borrow `packet` is still active (borrowed | `pass/atomics/runtime_stored_reference_atomic_events_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/atomics/runtime_stored_reference_atomic_events_exit` `pass/borrows/runtime_selected_reference_record_helper_exit` `pass/facts/write_only_indexed_reborrow_through_call_result_exit` `pass/facts/write_only_indexed_reborrow_through_const_name_exit` `pass/facts/write_only_indexed_reborrow_through_field_exit` `pass/facts/write_only_indexed_reborrow_through_local_exit` `pass/facts/write_only_indexed_reborrow_through_match_join_exit` `pass/recast/runtime_aggregate_slice_representation_recast_exit` `pass/slices/saved_byte_predicate_fields_disjoint_exit`

| 9 | pass/run rejected | machine-parameter requirement `Observe` requires contract constant expression call argument `kep | `pass/facts/abstract_signature_call_generic_actual_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/abstract_signature_call_generic_actual_through_call_result_exit` `pass/facts/abstract_signature_call_generic_actual_through_const_name_exit` `pass/facts/abstract_signature_call_generic_actual_through_field_exit` `pass/facts/abstract_signature_call_generic_actual_through_local_exit` `pass/facts/abstract_signature_call_generic_actual_through_match_join_exit` `pass/facts/abstract_signature_call_generic_actual_through_other_machine_exit` `pass/facts/abstract_signature_call_generic_actual_through_other_state_exit` `pass/facts/abstract_signature_call_generic_actual_through_parameter_exit` `pass/facts/abstract_signature_call_generic_actual_through_target_body_exit`

| 9 | incomplete | `pick_named$specialized$4497254f4b4724b8$1$4753` has no executable body: this compiler does not  | `pass/facts/case_dependent_default_domain_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/case_dependent_default_domain_through_call_result_exit` `pass/facts/case_dependent_default_domain_through_const_name_exit` `pass/facts/case_dependent_default_domain_through_field_exit` `pass/facts/case_dependent_default_domain_through_local_exit` `pass/facts/case_dependent_default_domain_through_match_join_exit` `pass/facts/case_dependent_default_domain_through_other_machine_exit` `pass/facts/case_dependent_default_domain_through_other_state_exit` `pass/facts/case_dependent_default_domain_through_parameter_exit` `pass/facts/case_dependent_default_domain_through_target_body_exit`

| 9 | pass/run rejected | statement 0 creates local borrow `entry` while local borrow `entry` is still active (borrowed at | `pass/facts/finite_family_exclusive_result/call_result` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/finite_family_exclusive_result/call_result` `pass/facts/finite_family_exclusive_result/const_name` `pass/facts/finite_family_exclusive_result/field` `pass/facts/finite_family_exclusive_result/local` `pass/facts/finite_family_exclusive_result/match_join` `pass/facts/finite_family_exclusive_result/other_machine` `pass/facts/finite_family_exclusive_result/other_state` `pass/facts/finite_family_exclusive_result/parameter` `pass/facts/finite_family_exclusive_result/target_body`

| 9 | pass/run rejected | native artifact target lowering failed: UnknownStructuralType(StructuralTypeId(111)) | `pass/facts/recast_parent_restoration_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/recast_parent_restoration_through_call_result_exit` `pass/facts/recast_parent_restoration_through_const_name_exit` `pass/facts/recast_parent_restoration_through_field_exit` `pass/facts/recast_parent_restoration_through_local_exit` `pass/facts/recast_parent_restoration_through_match_join_exit` `pass/facts/recast_parent_restoration_through_other_machine_exit` `pass/facts/recast_parent_restoration_through_other_state_exit` `pass/facts/recast_parent_restoration_through_parameter_exit` `pass/facts/recast_parent_restoration_through_target_body_exit`

| 8 | pass/run rejected | cannot prove default-domain field requirement for call write from main::entry: parameter storage | `pass/facts/non_owning_reference_erasure_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/non_owning_reference_erasure_through_call_result_exit` `pass/facts/non_owning_reference_erasure_through_const_name_exit` `pass/facts/non_owning_reference_erasure_through_field_exit` `pass/facts/non_owning_reference_erasure_through_local_exit` `pass/facts/non_owning_reference_erasure_through_other_machine_exit` `pass/facts/non_owning_reference_erasure_through_other_state_exit` `pass/facts/non_owning_reference_erasure_through_parameter_exit` `pass/facts/non_owning_reference_erasure_through_target_body_exit`

| 8 | fail wrong reason | returned projection source permission is unavailable: the saved descriptor has no complete ch... | `fail/borrows/returned_stored_view_reborrow_local_escape_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/returned_stored_view_reborrow_parent_write_rejected` `fail/slices/returned_inclusive_window_extent_mismatch` `fail/slices/window_extent_does_not_establish_bounds` `fail/wire/returned_preserving_remainder_input_write` `pass/borrows/returned_slice_record_reference_custody_exit` `pass/borrows/runtime_returned_stored_view_reborrow_exit` `pass/slices/runtime_mutable_slice_result_whole_forward_exit` `pass/wire/wire_preserving_decode_relay_exit`

| 7 | pass/run rejected | cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Coun | `pass/borrows/runtime_dependent_callback_window_byte_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_dependent_callback_window_byte_exit` `pass/borrows/runtime_dependent_forwarded_window_exit` `pass/borrows/runtime_dependent_window_byte_payload_exit` `pass/borrows/runtime_dependent_window_image_sibling_exit` `pass/borrows/runtime_dependent_window_region_disjoint_write_exit` `pass/generics/runtime_dependent_callback_renamed_formals_exit` `pass/generics/runtime_dependent_window_parameter_index_exit`

| 7 | pass/run rejected | returned view has no complete formation frontier | `pass/slices/runtime_indexed_descriptor_replacement_call_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/slices/stored_view_backing_mutated_while_returned_live` `fail/slices/stored_view_container_read_while_returned_live` `pass/slices/runtime_indexed_descriptor_replacement_call_exit` `pass/slices/runtime_nested_stored_view_return_exit` `pass/slices/runtime_nested_view_candidate_backings_exit` `pass/slices/runtime_stored_view_direct_window_argument_exit` `pass/slices/runtime_stored_view_dynamic_selection_exit`

| 7 | fail wrong reason | machine `Counter::replace` state `replace` statement 1: reading `counter` crosses an open defaul | `fail/arithmetic/bounded_store_consumed_before_repair` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/bounded_store_consumed_before_repair` `fail/dependent/data_requires_cyclic_handoff_store_rejected` `fail/machines/private_frontier_guarded_counter_cycle_invalid` `fail/machines/private_where_store_through_cycle_control` `fail/time/instant_mutation_leaves_invalid_return` `pass/recast/runtime_mutable_equivalent_range_recast_exit` `pass/recast/runtime_mutable_equivalent_record_recast_exit`

| 7 | fail wrong reason | unknown canonical target profile `reserved_target` | `fail/contracts/target_family_recursive_guarantee` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/contracts/target_family_recursive_guarantee` `fail/contracts/target_family_unshared_result` `fail/targets/sibling_body_type_error_rejected` `fail/targets/unrealized_conformance_rejected` `pass/targets/private_target_supplies_remain_disclosed` `pass/targets/retained_target_family_bodies` `pass/targets/retained_target_provider_family_exit`

| 7 | pass/run rejected | terminating machine iterate: cannot prove rank range `0..=5` for the rank produced by the select | `pass/borrows/runtime_projected_fixed_byte_array_local_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/termination/nested_helper_record_measure_result_equation_not_decreasing` `fail/termination/nested_helper_record_measure_result_equation_wrong_subject` `fail/termination/rank_arrival_public_result_wrong_subject` `pass/borrows/runtime_projected_fixed_byte_array_local_exit` `pass/termination/nested_helper_record_measure_result_equation_exit` `pass/termination/rank_arrival_unobserved_array_extent_compile` `pass/termination/runtime_public_result_field_rank_arrivals_exit`

| 6 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (statement seq... | `pass/borrows/runtime_stored_carrier_exchange_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/layouts/placed_reference_results/two_live` `pass/ownership/runtime_match_conditional_observation_cleanup_exit` `pass/recast/returned_qualification_preserves_representation_exit` `pass/recast/returned_qualified_reference_exit` `run/borrows/returned_descriptor_entry_snapshot` `run/slices/returned_record_fixed_array_window`

| 6 | pass/run rejected | state `read` receives `holder.counters[index].value` without an exact retained backing for its l | `pass/facts/data_where_standing_borrowed_field_array_const_name` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/data_where_standing_borrowed_field_array_const_name` `pass/facts/data_where_standing_borrowed_field_array_field` `pass/facts/data_where_standing_borrowed_field_array_other_machine` `pass/facts/data_where_standing_borrowed_field_array_other_state` `pass/facts/data_where_standing_borrowed_field_array_parameter` `pass/facts/data_where_standing_borrowed_field_array_target_body`

| 6 | incomplete | `main` calls `repair`; `repair` has no executable body: this compiler does not lower a statement | `pass/facts/recoverable_replacement_window_through_const_name_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/recoverable_replacement_window_through_const_name_exit` `pass/facts/recoverable_replacement_window_through_field_exit` `pass/facts/recoverable_replacement_window_through_local_exit` `pass/facts/recoverable_replacement_window_through_other_state_exit` `pass/facts/recoverable_replacement_window_through_parameter_exit` `pass/ownership/runtime_borrowed_match_repeated_repair_exit`

| 6 | fail wrong reason | Exact integer cast in machine `step` state `entry` from `u64` to `i32` is not provably represent | `fail/facts/live_across_call_spill_through_other_state_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/live_across_call_spill_through_other_state_rejected` `fail/facts/narrow_to_wide_sign_extension_through_other_state_rejected` `fail/facts/narrow_to_wide_zero_extension_through_other_state_rejected` `fail/facts/scalar_sign_zero_extension_through_other_state_rejected` `fail/facts/unsigned_exact_subtraction_interval_through_other_state_rejected` `pass/expressions/match_owned_computed_arm_arguments`

| 6 | fail wrong reason | machine `invalid` stores a borrowed value without a complete normal-exit backing relation | `fail/borrows/dynamic_store_local_backing_escape` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/dynamic_store_local_backing_escape` `fail/borrows/indexed_carrier_state_local_escape` `fail/facts/foreign_retained_shared_borrow_borrowed_slot_swap` `fail/facts/foreign_retained_shared_borrow_persistent_restore` `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` `pass/facts/foreign_retained_shared_borrow_persistent_restore`

| 6 | fail wrong reason | cannot establish assignment invariants: performed write has unknown destination origins in machi | `fail/borrows/conditional_reference_replacement_false_first_stale_index` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/conditional_reference_replacement_false_first_stale_index` `fail/borrows/conditional_reference_replacement_true_first_stale_index` `fail/slices/reborrowed_replaced_descriptor` `pass/borrows/runtime_selected_reference_mutation_complement_exit` `pass/borrows/runtime_uniform_projected_reference_selection_exit` `pass/write_only/returned_destination_transport_exit`

| 5 | incomplete | `entry_start` calls `Main::main`, which calls `reverse`; `reverse` has no executable body: this  | `pass/borrows/runtime_mutable_byte_loan_read_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_mutable_byte_loan_read_exit` `pass/borrows/runtime_recursive_mutable_array_store_exit` `pass/contracts/runtime_later_arm_call_requires_exit` `pass/termination/distance_constant_bound_exit` `pass/termination/runtime_call_result_short_circuit_rank_exit`

| 5 | fail wrong reason | cannot complete a call at statement 2 in machine `Main::main` state `main`: this write left the  | `fail/arithmetic/bounded_assignment_unproven` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/bounded_assignment_unproven` `fail/arithmetic/guarded_read_count_exceeds_target` `fail/arithmetic/guarded_read_count_unguarded_arrival` `fail/arithmetic/wrapping_guard_does_not_bound_original` `fail/range/indexed_field_range_runtime_write_rejected`

| 5 | fail wrong reason | cannot prove index `3` is within unknown slice length of `view` in read_guarded::entry | `fail/facts/slice_view_element_read_bound_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/slice_view_element_read_bound_through_call_result_rejected` `fail/facts/slice_view_element_read_bound_through_const_name_rejected` `fail/facts/slice_view_element_read_bound_through_field_rejected` `fail/facts/slice_view_element_read_bound_through_local_rejected` `fail/facts/slice_view_element_read_bound_through_match_join_rejected`

| 5 | pass/run rejected | state `read_reference` receives `value.inner.value` while local borrow `entry` is still active | `pass/borrows/nested_reference_call_checked_index_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/nested_reference_call_checked_index_exit` `pass/facts/case_match_payload_loan_through_other_machine_exit` `pass/facts/case_match_payload_loan_through_other_state_exit` `pass/facts/case_match_payload_loan_through_parameter_exit` `pass/facts/case_match_payload_loan_through_target_body_exit`

| 5 | pass/run rejected | cannot form specification index in machine `Main::main` requires contract: independent collectio | `pass/modules/selected_constant_array_alias_computed_extent_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/modules/selected_constant_array_alias_computed_extent_exit` `pass/modules/selected_constant_array_alias_match_qualification` `pass/modules/selected_constant_array_alias_named_extent_exit` `pass/modules/selected_constant_array_alias_qualification` `pass/slices/state_projected_selector_bounds`

| 4 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a statement of it yet... | `pass/drops/runtime_selected_linear_residual_death_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/opaque_independent_qualified_input_import_alias_exit` `pass/facts/opaque_independent_qualified_input_renumbered_catalog_exit` `pass/layouts/placed_reference_results/nested` `pass/ownership/runtime_match_expression_moves_selected_payload_exit`

| 4 | fail wrong reason | machine `incorrect` ensures contract proof fact `result == 4` is disproved by constant arithmeti | `fail/build/helper_contract_context` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/build/helper_contract_context` `fail/contracts/replacement_call_child_false_snapshot` `fail/expressions/cast_result_range` `fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected`

| 4 | fail wrong reason | exact arithmetic in machine `compute` state `entry` local `total` may overflow `i32`: the operan | `fail/facts/data_where_standing_mutable_recast_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/data_where_standing_mutable_recast_through_call_result_rejected` `fail/facts/data_where_standing_mutable_recast_through_field_rejected` `fail/facts/provider_contract_endpoint_control_false_result_guarantee` `fail/facts/provider_contract_endpoint_control_mutated_scalar_return`

| 4 | pass/run rejected | cannot establish assignment invariants: source parent type is unavailable | `pass/borrows/runtime_reassigned_view_capture_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/reassigned_view_conflicts_with_captured_child` `pass/borrows/runtime_reassigned_view_capture_exit` `pass/slices/narrowed_view_window` `run/domains/qualified_record_view_reference_erased_storage_exit`

| 4 | pass/run rejected | cannot prove requires contract for call middle from Main::main: 1 <= 3 && 3 <= main.bytes.len (n | `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` `pass/slices/runtime_indexed_store_operand_effect_order_exit` `pass/slices/runtime_namespace_range_contract_exit` `pass/slices/runtime_returned_range_extent_contract_exit`

| 4 | pass/run rejected | machine structural equation remains unsolved in a noncall static argument | `pass/wire/library_compact_binary_packed_reflection_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/wire/library_compact_binary_packed_reflection_exit` `pass/wire/runtime_wire_encode_borrowed_scalar_slice_exit` `pass/wire/runtime_wire_encode_interior_scalar_slice_exit` `pass/wire/runtime_wire_exact_array_without_count_exit`

| 4 | pass/run rejected | terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the | `pass/proofs/kernel_integer_subtract_order` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/kernel_integer_subtract_order_false_twin` `pass/proofs/kernel_integer_subtract_order` `pass/termination/measure_computed_endpoint` `pass/termination/measure_field_computed_limit`

| 4 | fail wrong reason | data `Interval` literal cannot prove the default domain: a `requires` fact is not established by | `fail/dependent/case_where_unproved_bound_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/dependent/case_where_unproved_bound_rejected` `fail/facts/static_callback_reach_through_field` `pass/domains/private_case_payload_construction` `pass/facts/static_callback_reach_through_field`

| 4 | fail wrong reason | terminal-artifact production failed: Lowering(CompilerEvaluationAdmission("machine `@constant-UN | `fail/layouts/declaration_only_initializer_formation` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/declaration_only_initializer_formation` `fail/terminal_psi/inspection_scalar_library` `pass/layouts/declaration_only_initializer_formation` `pass/terminal_psi/inspection_scalar_library`

| 3 | pass/run rejected | machine `_` ensures contract proof fact `_` speaks about proof-only `_`, which no entailment ... | `pass/cases/case_call_premises` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/float/runtime_meaning_contract_forwarding` `pass/proofs/float_guarded_negated_result_contract` `pass/proofs/float_named_contract_literal_landing`

| 3 | pass/run rejected | specification call `_` has no observation-free, reach-free, crash-free value denotation | `pass/float/public_class_result_contract_import_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/float/public_class_contract_ieee_equality_retains_zero_sign` `fail/float/public_class_result_contract_retains_actual` `fail/float/public_class_state_contract_retains_entry_subject`

| 3 | fail wrong reason | domain constraint `Below` for the const leaf value is false; failed domain `Below` | `fail/build/retained_domain_and_service_rows_invalid_index` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/build/retained_domain_and_service_rows_invalid_index` `fail/domains/logical_subject_false_constant` `fail/facts/scoped_scalar_initializer_through_const_name_rejected`

| 3 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Main::main", re | `pass/arithmetic/modular_power_state_bounds_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/arithmetic/modular_power_state_bounds_exit` `pass/float/float_range_field_storage_exit` `pass/generics/runtime_const_extent_origin_exit`

| 3 | pass/run rejected | native artifact target lowering failed: UnsupportedControlFlow { machine: MachineId(32), site: w | `pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit` `pass/borrows/incoming_reference_slot_replacement_exit` `pass/borrows/runtime_owned_carrier_descriptor_replacement_exit`

| 3 | pass/run rejected | machine `probe` state `entry` statement 2: cannot prove selected case for payload access `choice | `pass/borrows/runtime_copied_reference_conditional_nominal_backing_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_copied_reference_conditional_nominal_backing_exit` `pass/borrows/runtime_copied_reference_conditional_structural_result_exit` `pass/providers/runtime_provider_reference_allocation_exit`

| 3 | incomplete | `main` calls `forward`; `forward` has no executable body: this compiler does not lower a transit | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_returned_nonzero_nested_window_exit` `pass/facts/matched_field_windows_through_other_state_exit` `pass/slices/runtime_returned_nested_array_window_home_exit`

| 3 | pass/run rejected | state `read` receives `count` without an exact retained backing for its loan: the access has no  | `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` `pass/data/runtime_mixed_case_nested_common_fields_exit` `run/borrows/transition_guard_descriptor_fallthrough`

| 3 | pass/run rejected | state `checked` receives `main` as both mutable and read-only | `pass/dependent/state_arrival_byte_predicate_guarded_compile` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/dependent/state_arrival_byte_predicate_guarded_compile` `pass/recast/runtime_inline_shared_slice_record_read_exit` `pass/text/runtime_owned_string_byte_view_exit`

| 3 | incomplete | `AddressBridgeProvider::classified_make` has no executable body: this compiler does not lower it | `pass/facts/opaque_independent_qualified_result_import_alias_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/opaque_independent_qualified_result_import_alias_exit` `pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit` `pass/facts/opaque_independent_qualified_result_through_local_exit`

| 3 | pass/run rejected | machine `traverse` static argument 1 requires its declared binder kind, not an implementation | `pass/facts/static_callback_reach_evidence_scope` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/static_callback_reach_evidence_scope` `pass/generics/runtime_captured_map_two_method_tuples` `run/calls/nested_static_implementation_shadow`

| 3 | pass/run rejected | boundary requirement `Comparison::equal` in package Some(PackageKeyIdentity([81, 251, 88, 226, 1 | `pass/operators/crash_routes` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/operators/crash_routes` `pass/providers/checked_fixed_operator_dispatch_exit` `pass/providers/checked_fixed_operator_physical_custody`

| 3 | pass/run rejected | boundary operator `operator::Actions::set(ref-mut(named(name(u32))))->unit` in package Some(Pack | `pass/operators/runtime_named_unit_statement_effects_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/operators/runtime_named_unit_statement_effects_exit` `pass/providers/checked_boundary_operator_physical_custody` `pass/providers/module_named_operator_result_domain_satisfaction`

| 3 | pass/run rejected | cannot prove every subslice range endpoint of `0..alias.length` is non-negative | `run/ranges/borrowed_case_payload_alias` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `run/ranges/borrowed_case_payload_alias` `run/ranges/borrowed_case_payload_bound` `run/ranges/guarded_case_payload_identity`

| 3 | fail wrong reason | anonymous address value cannot prove fit in `addr::Bound` in machine `SelectedImage::retained_su | `fail/build/explicit_provider_scalar_budget` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/build/explicit_provider_scalar_budget` `fail/build/explicit_provider_storage_extent` `pass/build/explicit_provider_storage_extent_exit`

| 3 | pass/run rejected | machine `Main::main` state `main` expression type application uses undeclared lifetime argument  | `pass/borrows/carried_loan_replacement` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/replaced_carrier_keeps_new_backing_loan` `pass/borrows/carried_loan_replacement` `pass/ownership/runtime_carried_loan_field_replacement_exit`

| 3 | pass/run rejected | returned projection does not match this invocation and result path | `pass/borrows/dynamic_window_return_custody_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/runtime_slice_result_retains_backing` `pass/borrows/dynamic_window_return_custody_exit` `pass/borrows/runtime_slice_reference_result_exit`

| 3 | pass/run rejected | state `selected` receives `main` while local borrow `pair` is still active (borrowed at statemen | `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/aliased_forwarded_placed_views` `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` `run/borrows/terminal_case_completed_observation`

| 3 | pass/run rejected | constant checking: machine `@array-extent-1-9` is not build-time admissible: the checked Trap cr | `pass/build/component_declaration_scalar_conditions_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/declaration_only_array_formation` `pass/build/component_declaration_scalar_conditions_exit` `pass/layouts/declaration_only_array_formation`

| 3 | pass/run rejected | source induction case arithmetic is unsupported: Kernel(UnboundLevelParameter { index: 0, arity: | `pass/facts/source_indexed_type_parameter_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected` `pass/facts/source_indexed_type_parameter_exit` `pass/facts/source_indexed_type_parameter_u64_payload_exit`

| 3 | pass/run rejected | returned projection exceeds its source presentation | `pass/slices/runtime_stored_element_view_replacement_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/owned_carrier_result_retains_whole_operand` `pass/slices/runtime_stored_element_view_replacement_exit` `run/borrows/computed_element_view_state_extent`

| 2 | pass/run rejected | store in machine `_` state `_` argument `_` implicitly converts `_` to `_` (implicit domain w... | `pass/comptime/runtime_const_array_length_dependency_order_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/float/runtime_named_policy_result_exit` `pass/float/symbolic_integer_conversion_finite_requires_exit`

| 2 | pass/run rejected | selected provider requirement `named-callable(path(nominal\\(package-owner\\(32:1f0393f3a76baf55 | `pass/facts/generic_calling_policy/ordinary_parent_fused_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/generic_calling_policy/ordinary_parent_fused_exit` `pass/traits/public_method_const_prefix_inference_exit`

| 2 | incomplete | `AddressBridgeProvider::classified_make` has no executable body: this compiler does not lower a  | `pass/facts/opaque_independent_inherited_result_through_local_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/opaque_independent_inherited_result_through_local_exit` `pass/traits/runtime_qualified_owned_requirement_calls_exit`

| 2 | pass/run rejected | cannot prove published termination for machine `_`: its checked body reaches an operation wit... | `pass/termination/progress_selector_owned_arrival_replacement_helper_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/traits/inline_member_seals_target_family_provider` `run/ownership/sequence_owned_local_cleanup`

| 2 | fail wrong reason | `<structural type>` implements `Other<>`, which neither is nor inherits `RawSource`, the trait t | `fail/calls/static_boundary_statement_unrelated_evidence` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/calls/static_boundary_statement_unrelated_evidence` `fail/traits/explicit_implementation_unrelated_trait`

| 2 | fail wrong reason | machine `compute` state `entry`: recast source has no fixed representation inventory | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` `fail/facts/data_where_standing_mutable_recast_stronger_rejected`

| 2 | fail wrong reason | constructor obligations have no exact prepared application for `Pair`; unresolved generic member | `fail/generics/native_locator_inconsistent_byte_extents` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/generics/native_locator_inconsistent_byte_extents` `fail/generics/native_locator_undefined_user_coordinate`

| 2 | fail wrong reason | exact arithmetic in value argument `Limit` of `Index` may overflow `u32`: the operands are not p | `fail/generics/runtime_value_recipe_join_bound_loss` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/generics/runtime_value_recipe_join_bound_loss` `fail/generics/runtime_value_recipe_prior_write_overflow`

| 2 | fail wrong reason | cannot prove index `column` is within unknown slice length of `spectrum.samples` in radiance::en | `fail/proofs/field_view_index_missing_bound` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/field_view_index_missing_bound` `fail/proofs/field_view_index_wrong_sibling`

| 2 | fail wrong reason | assumed binding requirement absent from canonical artifact | `fail/providers/assumed_result_bound_equal_count_lifetime_mapping_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/providers/assumed_result_bound_equal_count_lifetime_mapping_rejected` `fail/providers/assumed_result_bound_lifetime_mapping_rejected`

| 2 | pass/run rejected | call to `execute` has operational envelope `suspend block` but acknowledges neither suspension n | `pass/blockexec/blocking_executor_custody_claims_compile` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/blockexec/blocking_executor_custody_claims_compile` `pass/blockexec/runtime_selected_worker_body_exit`

| 2 | pass/run rejected | temporary backing lifetime coverage is unavailable for an operand of `entry` in state `entry` at | `pass/borrows/inline_carried_reference_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/inline_carried_reference_holds_backing` `pass/borrows/inline_carried_reference_exit`

| 2 | pass/run rejected | returned projection has no matching explicit argument borrow | `pass/borrows/runtime_computed_view_observation_composition_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_computed_view_observation_composition_exit` `pass/slices/runtime_mutable_slice_result_nested_reborrow_exit`

| 2 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity | `pass/build/const_index_dependency_order_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/build/const_index_dependency_order_exit` `pass/generics/runtime_computed_nested_const_call_exit`

| 2 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "consume", reaso | `pass/calls/nonreturning_record_match_normal_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/calls/nonreturning_record_match_normal_exit` `pass/calls/nonreturning_record_match_stopped_exit`

| 2 | pass/run rejected | machine call cycle: `FirstStage::read` can re-enter itself through a selector call or boundary r | `pass/capabilities/selected_applied_synchronous_chain_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/capabilities/selected_applied_synchronous_chain_exit` `pass/traits/applied_requirement_lexical_isolation_exit`

| 2 | pass/run rejected | construction of `Selected` field `value` is not proven in domain `u64::AtLeast73` in consume sta | `pass/domains/domain_pattern_match_bound_full_width_field_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/domains/domain_pattern_match_bound_full_width_field_exit` `pass/facts/domain_pattern_match_bound_through_field_exit`

| 2 | incomplete | `main` calls `exercise`, which calls `window`; `window` has no executable body: this compiler do | `pass/facts/computed_record_window_through_other_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/computed_record_window_through_other_machine_exit` `pass/facts/computed_record_window_through_other_state_exit`

| 2 | pass/run rejected | cannot prove requires contract for call compute from apply: visits == 0 (needs fact `counter ==  | `pass/facts/match_computed_root_through_other_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/match_computed_root_through_other_machine_exit` `pass/facts/match_computed_root_through_target_body_exit`

| 2 | pass/run rejected | native artifact component entry ABI failed: native entry transport: native entry bridge failed:  | `pass/facts/opaque_independent_closed_carrier_through_other_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/opaque_independent_closed_carrier_through_other_machine_exit` `pass/providers/runtime_foreign_allocation_across_component_exit`

| 2 | incomplete | `main` calls `prepare`, which calls `repair`; `repair` has no executable body: this compiler doe | `pass/facts/recoverable_replacement_window_through_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/recoverable_replacement_window_through_call_result_exit` `pass/facts/recoverable_replacement_window_through_other_machine_exit`

| 2 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/other_machine` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/other_machine` `pass/facts/target_family_callback_guarantee/other_state`

| 2 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/other_machine` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/other_machine` `pass/facts/target_family_contract_operand/other_state`

| 2 | pass/run rejected | duplicate machine `_` | `pass/layouts/runtime_recast_reference_captures_index_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/target_layout_call_checks_all_widths` `pass/recast/runtime_byte_region_state_transfer_exit`

| 2 | pass/run rejected | cannot prove ensures contract for exit from HeapProvider::allocate at statement 0: result in All | `pass/memory/runtime_linear_native_allocation_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/memory/runtime_linear_native_allocation_call_result_exit` `pass/memory/runtime_linear_native_allocation_exit`

| 2 | pass/run rejected | state `write_pairs` receives `main` as mutable more than once | `pass/recast/runtime_inline_slice_view_mutable_write_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/recast/runtime_inline_slice_view_mutable_write_exit` `pass/recast/runtime_interior_slice_view_mutable_write_exit`

| 2 | fail wrong reason | normal call `forward` (Handle { arena_index: 13385, generation: 1 }) in machine `invalid` state  | `fail/borrows/evidence_bound_record_result_keeps_loan` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/evidence_bound_record_result_keeps_loan` `pass/generics/runtime_inherited_composite_requirement_exit`

| 2 | fail wrong reason | cannot prove requires contract for call observe from main: cells[0] in Cell::Issued in state `en | `fail/borrows/owned_capture_weaker_formal_qualification` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/owned_capture_weaker_formal_qualification` `pass/borrow/owned_capture_weaker_formal`

| 2 | fail wrong reason | applied callable has an unrepresented static type operand | `fail/facts/selected_value_application_through_field_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/selected_value_application_through_field_rejected` `pass/facts/selected_value_application_through_field_exit`

| 2 | fail wrong reason | `quiet` does not satisfy `StepContract::step`: helper `Requirement` requires `Anchor::anchor`, b | `fail/facts/static_callback_reach_requirement_telescope_requirement` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/static_callback_reach_requirement_telescope_requirement` `pass/facts/static_callback_reach_requirement_telescope`

| 1 | fail wrong reason | terminal call `climb` does not select a declared machine entry or callable requirement | `fail/proofs/inductive_climbing_sum_unbounded_accumulator` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/inductive_climbing_sum_unbounded_accumulator`

| 1 | pass/run rejected | selected contract has no unique supplying map application | `pass/constants/direct_named_implementation_initializer_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/positional_implementation_binder_through_const_name_exit`

| 1 | incomplete | `chained::scan` has no executable body: this compiler does not lower a transition of it yet (sta | `pass/slices/runtime_cut_byte_view_cycle_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/slices/runtime_cut_byte_view_cycle_exit`

| 1 | fail wrong reason | store in machine `Main::main` state `main` assignment implicitly converts `i32` to `i32 in Wrapp | `fail/arithmetic/exact_overflow_value_call_hint` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/exact_overflow_value_call_hint`

| 1 | fail wrong reason | division by zero in machine `quotient` state `entry` return value: the divisor is provably zero, | `fail/arithmetic/indexed_divisor_copied_selector_storage_write` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/indexed_divisor_copied_selector_storage_write`

| 1 | fail wrong reason | store in machine `Main::pick` state `pick` local `v` implicitly converts `i32` to `i32 in Wrappi | `fail/arithmetic/wrapping_target_plain_operands_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/wrapping_target_plain_operands_rejected`

| 1 | fail wrong reason | partial exact remainder in machine `main` state `entry` local `remainder`: the divisor must be p | `fail/arithmetic/zii_range_excludes_zero_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/arithmetic/zii_range_excludes_zero_rejected`

| 1 | fail wrong reason | statement 6 reads `held.value` while local borrow `view` is still active (borrowed at statement  | `fail/borrows/nested_machine_carried_live_child_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/borrows/nested_machine_carried_live_child_rejected`

| 1 | fail wrong reason | `RawSource::fill` expects 0 static invocation argument(s), got 2; explicit selection requires a  | `fail/calls/static_boundary_statement_misplaced_evidence` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/calls/static_boundary_statement_misplaced_evidence`

| 1 | fail wrong reason | fixed-array length `[u8; observed()]`: const evaluation of `observed` failed: data `Bounded` lit | `fail/comptime/generic_constructor_field_demand_extent_domain_violation` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/comptime/generic_constructor_field_demand_extent_domain_violation`

| 1 | fail wrong reason | cannot prove requires contract for call demand_identity from guarded: FloatingEquality.equals(le | `fail/contracts/executable_float_box_equality_not_identity` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/contracts/executable_float_box_equality_not_identity`

| 1 | fail wrong reason | match structural pattern must mention every declared field or explicitly use `..` | `fail/control_flow/arm_pattern_missing_field` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/control_flow/arm_pattern_missing_field`

| 1 | fail wrong reason | match field `typo` is missing, duplicated, or belongs to another record/case | `fail/control_flow/arm_pattern_rest_unknown_field` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/control_flow/arm_pattern_rest_unknown_field`

| 1 | fail wrong reason | cannot form specification operator `>=` in data `Pair` case `B` predicate: no exact selected mea | `fail/dependent/case_where_sibling_payload_scope` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/dependent/case_where_sibling_payload_scope`

| 1 | fail wrong reason | machine `_` cannot lend restricted field `_` to `_`: the selected scalar referent's normal-re... | `fail/dependent/data_where_live_borrow_pins_witness_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit`

| 1 | fail wrong reason | cannot prove ensures contract for exit from extract_payload at statement 3: result >= 40 && resu | `fail/facts/four_variant_enum_payload_bound_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/four_variant_enum_payload_bound_through_call_result_rejected`

| 1 | fail wrong reason | conformance `Selected` parameter `Setting` requires a const argument, but `<structural type>` ha | `fail/facts/open_record_const_implementation_specialization_agreement_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/open_record_const_implementation_specialization_agreement_rejected`

| 1 | fail wrong reason | constant checking: inconsistent captured contract bounds | `fail/facts/size_align_layout_geometry_through_match_join_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/size_align_layout_geometry_through_match_join_rejected`

| 1 | fail wrong reason | cannot prove index `3` is within unknown slice length of `view` in target_reader::entry | `fail/facts/slice_view_element_read_bound_through_other_machine_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/slice_view_element_read_bound_through_other_machine_rejected`

| 1 | fail wrong reason | cannot prove index `3` is within unknown slice length of `v` in check_view::entry | `fail/facts/slice_view_element_read_bound_through_other_state_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/slice_view_element_read_bound_through_other_state_rejected`

| 1 | fail wrong reason | cannot prove ensures contract for exit from produced at statement 0: result.value == 2 | `fail/facts/temporary_case_cleanup_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/temporary_case_cleanup_through_call_result_rejected`

| 1 | fail wrong reason | cannot prove ensures contract for exit from forward at statement 0: result.value == 2 | `fail/facts/temporary_case_cleanup_through_other_machine_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/facts/temporary_case_cleanup_through_other_machine_rejected`

| 1 | fail wrong reason | named operator `F64::is_finite` is not available in this source's import scope | `fail/float/primitive_comparison_no_named_helper_exposure` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/float/primitive_comparison_no_named_helper_exposure`

| 1 | fail wrong reason | case-membership operation lost its exact subject, carrier or case identity | `fail/generics/extraction_foreign_case` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/generics/extraction_foreign_case`

| 1 | fail wrong reason | const argument `Setting` has no static value with its exact declared carrier | `fail/generics/record_const_foreign_nominal_carrier` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/generics/record_const_foreign_nominal_carrier`

| 1 | fail wrong reason | accessor write application for `Accessor<Access<StablePlacement,Values>,words>` with value `u64` | `fail/layouts/placed_stable_array_readonly_write` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/placed_stable_array_readonly_write`

| 1 | fail wrong reason | generic data `NotAPolicy` expected 0 arguments but got 1 | `fail/layouts/plan_laid_policy_without_plan_machine` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/plan_laid_policy_without_plan_machine`

| 1 | fail wrong reason | failed scalar accounting probe missed its authored refusal | `fail/layouts/portable_layout_constant_composition` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/portable_layout_constant_composition`

| 1 | fail wrong reason | accessor write application for `Accessor<Access<WordPlacement,Word>,value>` with value `u64`: va | `fail/layouts/resident_field_write_preserves_qualification` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/layouts/resident_field_write_preserves_qualification`

| 1 | fail wrong reason | callable `rejoin` has an invalid content-conservation contract: content conservation requires th | `fail/ownership/selected_storage_join_unrelated_root` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/ownership/selected_storage_join_unrelated_root`

| 1 | fail wrong reason | exact arithmetic in machine `Main::climb` ensures contract may overflow `u64`: the operands are  | `fail/proofs/inductive_climbing_sum_step_false_twin` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/inductive_climbing_sum_step_false_twin`

| 1 | fail wrong reason | cannot form specification operator `[]` in machine `issue` ensures contract: no exact selected m | `fail/proofs/result_index_authored_operator` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/result_index_authored_operator`

| 1 | fail wrong reason | cannot prove requires contract for call bounded from main: selected(3) < 1 && 3 < 9 (needs fact  | `fail/proofs/structural_application_premise_false_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/proofs/structural_application_premise_false_rejected`

| 1 | fail wrong reason | constant checking: scalar evaluation requires exact authored selection before evaluation; missin | `fail/providers/checked_boundary_extent_index_out_of_bounds` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/providers/checked_boundary_extent_index_out_of_bounds`

| 1 | fail wrong reason | machine `Main::main` state `main`: byte-region recast target `Facted` must be recursively fact-f | `fail/recast/recast_mut_record_fact_fenced` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/recast/recast_mut_record_fact_fenced`

| 1 | fail wrong reason | machine call cycle: `Main::scan_a`, `Main::scan_b` -- a preserving cycle has no strict measure D | `fail/termination/lexicographic_component_zero_step_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/termination/lexicographic_component_zero_step_rejected`

| 1 | fail wrong reason | tests/omega/fail/termination/retired_block_form/main.omg:5:15: expected `;`, found identifier `r | `fail/termination/retired_block_form` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/termination/retired_block_form`

| 1 | fail wrong reason | cannot prove requires contract: machine `Primary::code$specialized$a14de39e48d0cdd3$2$5592` stat | `fail/traits/dyn_finite_family_narrowed_provider` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/traits/dyn_finite_family_narrowed_provider`

| 1 | fail wrong reason | machine `apply` static argument 2 requires a named implementation | `fail/traits/evidence_binder_given_data_type` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/traits/evidence_binder_given_data_type`

| 1 | fail wrong reason | machine `flip` state `entry` does not satisfy trait `Flipping` machine `flip`: conformance borro | `fail/traits/prepared_callable_wrong_member_source` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/traits/prepared_callable_wrong_member_source`

| 1 | fail wrong reason | machine `Wrong::set` proves no guarantee for the selected comparison law `Setting::set` | `fail/traits/public_guarantee_wrong_formal` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `fail/traits/public_guarantee_wrong_formal`

| 1 | pass/run rejected | cannot prove ensures contract for exit from from_field at statement 0: result >= reading.value - | `pass/arithmetic/truncating_reconstruction_bound` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/arithmetic/truncating_reconstruction_bound`

| 1 | pass/run rejected | cannot prove ensures contract for exit from Confirmation::finish at statement 2: proposition:fac | `pass/atomics/selected_reference_atomic_provider` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/atomics/selected_reference_atomic_provider`

| 1 | pass/run rejected | cannot prove requires contract for call observe from main: first[0] in Cell::Issued in state `en | `pass/borrow/owned_capture_replaced_reference` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrow/owned_capture_replaced_reference`

| 1 | pass/run rejected | view formation has incomplete permission suppliers at its captured source | `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit`

| 1 | incomplete | `main` has no executable body: this compiler does not lower a state of it yet (reference retirem | `pass/borrows/copied_intermediate_descriptor_snapshot_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/copied_intermediate_descriptor_snapshot_exit`

| 1 | pass/run rejected | completed output has no captured lifetime-input receipt; completed output from `again` in machin | `pass/borrows/foreign_output_captured_reference_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/foreign_output_captured_reference_exit`

| 1 | pass/run rejected | statement 0 cannot resolve the complete borrowed destination of `value.middle.items[0].value.cha | `pass/borrows/nested_reference_write_disjoint_facts_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/nested_reference_write_disjoint_facts_exit`

| 1 | pass/run rejected | cannot establish assignment invariants: partial write enclosure has no exact active-case frontie | `pass/borrows/returned_record_case_field_store_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/returned_record_case_field_store_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "swap::rotate",  | `pass/borrows/runtime_copyable_carrier_retirement_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_copyable_carrier_retirement_exit`

| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value from Main::main: observed >= 0 && obs | `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(OverlappingProjectedCustody { reason: "projected c | `pass/borrows/runtime_partial_reference_partition_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_partial_reference_partition_exit`

| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value from Main::main: first >= 0 && first  | `pass/borrows/runtime_shared_reference_array_cohort_restoration_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_shared_reference_array_cohort_restoration_exit`

| 1 | pass/run rejected | native artifact identity physical pipeline failed: common physical staging failed: Selection(Leg | `pass/borrows/runtime_shared_view_record_presentation_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/borrows/runtime_shared_view_record_presentation_exit`

| 1 | pass/run rejected | copied call lost its ordinary and realized Value operand roster | `pass/calls/native_code_layout_forward_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/calls/native_code_layout_forward_machine_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "main", reason:  | `pass/calls/runtime_borrowed_parameter_ensures_issuer_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/calls/runtime_borrowed_parameter_ensures_issuer_exit`

| 1 | pass/run rejected | cannot prove requires contract for call compare from walk: bytes[offset].number_of_pages <= 4503 | `pass/contracts/borrowed_descriptor_guarded_end_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/contracts/borrowed_descriptor_guarded_end_exit`

| 1 | pass/run rejected | cannot prove asm `requires` fact at block entry in main state `entry` (statement 5): returned == | `pass/contracts/runtime_mutable_owned_call_poststate_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/contracts/runtime_mutable_owned_call_poststate_exit`

| 1 | pass/run rejected | statement 4 reads `view` without an exact retained backing for its loan | `pass/contracts/runtime_mutable_owned_collection_poststate_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/contracts/runtime_mutable_owned_collection_poststate_exit`

| 1 | pass/run rejected | terminating machine drain reaches recursive component `step`, which has no `terminates by` ranki | `pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit`

| 1 | pass/run rejected | terminating machine spin reaches recursive component `step`, which has no `terminates by` rankin | `pass/control_flow/runtime_ranked_state_cycle_owned_record_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/control_flow/runtime_ranked_state_cycle_owned_record_exit`

| 1 | pass/run rejected | cannot prove requires contract for call read_selected from Main::main: valid_values(main.rows.va | `pass/dependent/runtime_symbolic_slice_cell_theorem_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/dependent/runtime_symbolic_slice_cell_theorem_exit`

| 1 | pass/run rejected | cannot prove requires contract for call replace from Main::main: valid_values(main.rows.values[. | `pass/dependent/runtime_symbolic_slice_predicate_store_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/dependent/runtime_symbolic_slice_predicate_store_exit`

| 1 | pass/run rejected | cannot prove default-domain field requirement for return from issue_handle at statement 0: Handl | `pass/domains/qualified_record_case_predicate` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/domains/qualified_record_case_predicate`

| 1 | pass/run rejected | cannot prove default-domain field requirement for call clear from capture::entry: parameter numb | `pass/domains/routed_record_capture_before_sibling_mutation` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/domains/routed_record_capture_before_sibling_mutation`

| 1 | pass/run rejected | returned view lost its completed invocation capture | `pass/domains/runtime_selected_nested_field_domain_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/domains/runtime_selected_nested_field_domain_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidCleanupPartition { reason: "cleanup partiti | `pass/drops/runtime_selected_partial_owner_cleanup_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/drops/runtime_selected_partial_owner_cleanup_exit`

| 1 | pass/run rejected | machine `pace` omits `invokes DarwinClock` from its published synchronous-invocation ceiling, bu | `pass/effects/gui_semantic_services` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/effects/gui_semantic_services`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 9690 (`*` in tests/omega/pass/expressions/mat | `pass/expressions/match_operand_anonymous_operation_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/expressions/match_operand_anonymous_operation_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "exercise", reas | `pass/expressions/runtime_match_copy_descriptor_call_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/expressions/runtime_match_copy_descriptor_call_exit`

| 1 | pass/run rejected | machine `apply` state `entry` statement 0: cannot prove selected case for payload access `chosen | `pass/facts/case_payload_exclusive_loan_through_other_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/case_payload_exclusive_loan_through_other_machine_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "width", reason: | `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 571 (`==` in tests/omega/pass/facts/indexed_c | `pass/facts/indexed_crash_guard_through_target_body_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/indexed_crash_guard_through_target_body_exit`

| 1 | pass/run rejected | cannot prove requires contract for call compute from write: visits == 0 (needs fact `counter ==  | `pass/facts/match_computed_root_through_other_state_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/match_computed_root_through_other_state_exit`

| 1 | pass/run rejected | cannot prove subslice range end bound `0..bound.end` is within unknown slice length of `buffer`  | `pass/facts/nested_result_extent_through_field_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/nested_result_extent_through_field_exit`

| 1 | pass/run rejected | cannot prove default-domain field requirement for call write from main::entry: parameter first r | `pass/facts/non_owning_reference_erasure_through_match_join_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/non_owning_reference_erasure_through_match_join_exit`

| 1 | incomplete | `main` calls `choose`, which calls `prepare`, which calls `repair`; `repair` has no executable b | `pass/facts/recoverable_replacement_window_through_match_join_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/recoverable_replacement_window_through_match_join_exit`

| 1 | incomplete | `X86Repair::repair` has no executable body: this compiler does not lower a statement of it yet ( | `pass/facts/recoverable_replacement_window_through_target_body_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/recoverable_replacement_window_through_target_body_exit`

| 1 | pass/run rejected | cannot prove default-domain field requirement for call check from main::entry: parameter pair re | `pass/facts/resident_backing_plain_ram_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/resident_backing_plain_ram_exit`

| 1 | pass/run rejected | cannot prove requires contract for call agree from main: saved == holder.items[0] && saved == 7  | `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit`

| 1 | pass/run rejected | cannot prove requires contract for call agree from read_kept: saved == holder.items[0] && saved  | `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/call_result` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/call_result`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 568 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/const_name` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/const_name`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/field` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/field`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/local` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/local`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/match_join` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/match_join`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 566 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/parameter` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/parameter`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 590 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_callback_guarantee/target_body` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_callback_guarantee/target_body`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/call_result` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/call_result`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/const_name` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/const_name`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/field` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/field`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/local` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/local`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/match_join` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/match_join`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 562 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/parameter` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/parameter`

| 1 | pass/run rejected | authored Operator declaration selection occurrence 578 (`<=` in tests/omega/pass/facts/target_fa | `pass/facts/target_family_contract_operand/target_body` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/facts/target_family_contract_operand/target_body`

| 1 | pass/run rejected | machine `received` state `entry` statement 6: operand has no complete completed call-slot relati | `pass/generics/generic_result_parameter_receipt` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/generics/generic_result_parameter_receipt`

| 1 | pass/run rejected | index compatibility condition `index-equality:Indexed:forward_indexed::entry:call-1-0:Indexed<Le | `pass/generics/open_computed_quantity_result` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/generics/open_computed_quantity_result`

| 1 | pass/run rejected | realization contract lost its complete capture identity: selected=Selected, call_owner=main, reg | `pass/generics/runtime_captured_map_borrowed_type` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/generics/runtime_captured_map_borrowed_type`

| 1 | pass/run rejected | const argument expression in generic data `ForwardedBuffer` is invalid: const operator applicati | `pass/generics/runtime_const_data_symbolic_expression_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/generics/runtime_const_data_symbolic_expression_exit`

| 1 | pass/run rejected | cannot prove requires contract for call consume_inferred from main: Count == 7 (needs fact `Coun | `pass/generics/runtime_inferred_index_statement_call_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/generics/runtime_inferred_index_statement_call_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "finish", reason | `pass/host/process_exit_selected_frontier` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/host/process_exit_selected_frontier`

| 1 | pass/run rejected | a value call to generic machine `macos_window_create` cannot derive a complete type/const/machin | `pass/host/runtime_semantic_gui_reach_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/host/runtime_semantic_gui_reach_exit`

| 1 | incomplete | `main` calls `update`; `update` has no executable body: this compiler does not lower its declara | `pass/layouts/placed_primitive_returned_backing` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/layouts/placed_primitive_returned_backing`

| 1 | pass/run rejected | IntegerAt store in machine `guarded_write` state `entry` assignment requires a value proven to f | `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit`

| 1 | incomplete | `entry_start` calls `Main::main`; `Main::main` has no executable body: this compiler does not lo | `pass/locals/runtime_local_qualified_view_member_read_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/locals/runtime_local_qualified_view_member_read_exit`

| 1 | pass/run rejected | selected optimizations `SelectedSameBlockCopyI64RemovalV1`, `SelectedRedundantExtensionRemovalV1 | `pass/optimizer/pre_allocation_joint_frontier_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/optimizer/pre_allocation_joint_frontier_exit`

| 1 | pass/run rejected | cannot prove requires contract for call drop from main: retained in Capacity::Held in state `ent | `pass/ownership/registration_owned_claim_retains_provenance_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/ownership/registration_owned_claim_retains_provenance_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(ConflictingStructuralValueHome { root: Expression( | `pass/ownership/runtime_match_nested_observation_cleanup_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/ownership/runtime_match_nested_observation_cleanup_exit`

| 1 | pass/run rejected | cannot plan contextual call cleanup against the executable hook roster: machine `structural_resu | `pass/ownership/runtime_match_observation_cleanup_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/ownership/runtime_match_observation_cleanup_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "binding_subject | `pass/ownership/runtime_match_projected_subject_cleanup_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/ownership/runtime_match_projected_subject_cleanup_exit`

| 1 | pass/run rejected | machine `Owner::inspect` static type argument uses undeclared lifetime argument `'storage' in `O | `pass/packages/runtime_attached_owner_crash_guard_scope_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/packages/runtime_attached_owner_crash_guard_scope_exit`

| 1 | pass/run rejected | provider completion for `named-callable(path(nominal\\(package-owner\\(32:78d666a3ef4ce09f7a7e4a | `pass/progress/provider_receiver_progress_installation` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/progress/provider_receiver_progress_installation`

| 1 | pass/run rejected | construction value is not proven in domain `Accepted` in main state `entry` at statement 12; a c | `pass/proofs/declaration_predicate_calls_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/proofs/declaration_predicate_calls_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "work::received" | `pass/proofs/exhaustive_nonreturn_contract` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/proofs/exhaustive_nonreturn_contract`

| 1 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity | `pass/proofs/family_carrier_relation_laws` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/proofs/family_carrier_relation_laws`

| 1 | pass/run rejected | checked machine `related_reflexive` cannot establish proposition ensure `boolean:value == value` | `pass/proofs/proposition_relation_hierarchy_compile` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/proofs/proposition_relation_hierarchy_compile`

| 1 | pass/run rejected | cannot prove requires contract for call capacity from geometry: pages >= 1 && pages <= 512 && __ | `pass/proofs/reviewed_mathematical_data_contract_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/proofs/reviewed_mathematical_data_contract_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Policy::identit | `pass/providers/checked_operator_declaration_without_application` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/providers/checked_operator_declaration_without_application`

| 1 | pass/run rejected | cannot transfer a non-copy value out of borrowed storage without replacing its owner in `capture | `pass/recast/returned_same_type_view_capture_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/recast/returned_same_type_view_capture_exit`

| 1 | pass/run rejected | cannot prove requires contract for call consume from inspect: offset <= 8 && bytes[offset].pages | `pass/recast/runtime_captured_guard_conjunction_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/recast/runtime_captured_guard_conjunction_exit`

| 1 | pass/run rejected | cannot prove requires contract for call inspect_after from probe: ascii_only(bytes[0..1]) && byt | `pass/slices/saved_byte_predicate_operand_disjoint_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/slices/saved_byte_predicate_operand_disjoint_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "churn::next", r | `pass/structural/runtime_scalar_record_reclamation_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/structural/runtime_scalar_record_reclamation_exit`

| 1 | pass/run rejected | inspection constant omitted its expected debug companion | `pass/terminal_psi/debug_identity_complete_module` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/terminal_psi/debug_identity_complete_module`

| 1 | pass/run rejected | terminating machine walk reaches recursive component `iterate`, which has no `terminates by` ran | `pass/termination/runtime_named_state_remainder_endpoint_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/termination/runtime_named_state_remainder_endpoint_exit`

| 1 | pass/run rejected | machine `Reader::read` state `read` does not satisfy trait `Reading` machine `read`: conformance | `pass/traits/applied_conformance_captured_fresh_regions_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/traits/applied_conformance_captured_fresh_regions_exit`

| 1 | incomplete | `main` has no executable body: this compiler does not lower a state of it yet (statement sequenc | `pass/traits/prepared_callable_nested_borrow_paths_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/traits/prepared_callable_nested_borrow_paths_exit`

| 1 | pass/run rejected | call `read` has an invalid argument correspondence: selected call has no exact applied public si | `pass/traits/refined_borrowed_implementation_exit` | `f3612364ffbb9489d7be1409397df6867fcbebc6` |
Members: `pass/traits/refined_borrowed_implementation_exit`

## Measurement provenance

- Measured commit: `741270df60f7eab218e879eb6771d2bc75e64c00`
- Baseline sweep: `759c80404428` (`work/ledger-sweep-regressions`)
- Host: linux x86_64 (x86_64)
- Corpus: `OMEGA_CORPUS_JOBS=1`; `mbx xtask corpus --filter` in batches of ≤200
  plus `corpus_runner` direct runs for fixtures dropped by the 1800s xtask
  runner cap on large batches.
- Classification: ledger comparison matches `cargo xtask corpus` regression
  rules in `tools/xtask/src/corpus.rs`.


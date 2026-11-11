# Corpus rows that regressed, measured at 759c80404428

1296 rows the ledger on main records as good but the compiler at `759c80404428` does not. Measured by a full sweep (120 shards, linux_x86_64, jobs=3). Not re-checked at current main.

- pass/run rejected: 684
- fail wrong reason: 486
- incomplete: 69
- fail accepted: 34
- timeout: 23

## Fail programs the compiler accepts (unsound)

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

## Clusters by first diagnostic

| Rows | Kind | First diagnostic | Example |
|---|---|---|---|
| 42 | fail wrong reason | static operand changed its retained selected application | `fail/facts/open_array_const_implementation_through_call_result_rejected` |
| 37 | pass/run rejected | static operand changed its retained selected application | `pass/facts/open_array_const_implementation_through_call_result_exit` |
| 36 | pass/run rejected | terminal-artifact production failed: Lowering(Unsupported("retained-borrow callable lost its schema owner")) | `pass/facts/foreign_nested_record_backing_call_result` |
| 34 | fail wrong reason | terminal-artifact production failed: Lowering(Unsupported("retained-borrow callable lost its schema owner")) | `fail/facts/foreign_nested_record_backing_call_result` |
| 33 | fail wrong reason | cannot complete return from machine `_` state `_`: this write left the `_` facts of `_` (data `_`) unproved; restore the | `fail/arithmetic/widened_result_predicate_invalid_write` |
| 23 | timeout | (no diagnostic) | `fail/build/installed_arrival_without_context` |
| 23 | pass/run rejected | machine `_` state `_` statement N: stored call has no complete statement-successor effects: an invocation relation is un | `pass/build/uefi_image_residence_observation` |
| 22 | pass/run rejected | no provider is selected for boundary requirement `_`, which the program entry reaches on target `_` | `pass/borrow/runtime_write_only_boundary_satisfier_exit` |
| 20 | fail wrong reason | machine `_` state `_` statement N: stored call has no complete statement-successor effects: an invocation relation is un | `fail/borrows/applied_provider_output_requires_backing` |
| 20 | pass/run rejected | cannot establish assignment invariants: partial write enclosure has no declared type | `pass/borrows/runtime_reassigned_view_return_exit` |
| 20 | pass/run rejected | statement N creates local borrow `_` while local borrow `_` is still active (borrowed at statement N; its last use is at | `pass/facts/computed_record_window_through_target_body_exit` |
| 18 | fail wrong reason | cannot establish assignment invariants: partial write enclosure has no declared type | `fail/facts/descriptor_range_exclusive_loan_through_call_result_rejected` |
| 17 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "canno | `fail/float/state_constructor_ieee_reflexivity` |
| 15 | fail wrong reason | machine `_` cannot prove ensures contract proof fact `_` on the transition arm guarded by `_` | `fail/arithmetic/remainder_postcondition_signed_minimum_underbound_rejected` |
| 15 | pass/run rejected | native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(N) has no admitted mechan | `pass/build/default_component_abi_extents` |
| 15 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "canno | `pass/modules/rational_boolean_indices` |
| 14 | fail wrong reason | selected map disagrees with its supplying evidence bound | `fail/facts/inferred_array_equation_call_mixed_binders_evidence_application_mismatch_rejected` |
| 13 | pass/run rejected | store in machine `_` state `_` argument `_` implicitly converts `_` to `_` (implicit domain weakening); a typed value ke | `pass/comptime/runtime_const_array_length_dependency_order_exit` |
| 13 | pass/run rejected | cannot prove ensures contract for exit from whole_view at statement N: result.len == Count | `pass/facts/as_slice_views_generic_offset_selector_exit` |
| 12 | fail wrong reason | exact arithmetic in machine `_` state `_` return value may overflow `_`: the operands are not provably in range (decisio | `fail/contracts/ruled_out_arm_value_does_not_reach_join` |
| 10 | pass/run rejected | statement N mutates `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N) | `pass/atomics/runtime_stored_reference_atomic_events_exit` |
| 9 | fail wrong reason | store in machine `_` state `_` argument `_` implicitly converts `_` to `_` (implicit domain weakening); a typed value ke | `fail/comptime/recipe_extent_index_refused_at_closure` |
| 9 | fail wrong reason | normal call `_` (Handle { arena_index: N, generation: N }) in machine `_` state `_` statement N: completed call result h | `fail/facts/foreign_reference_result_through_call_result_rejected` |
| 9 | fail wrong reason | data `_` literal violates the default domain: a `_` fact evaluates FALSE at this construction (chN: construction is the  | `fail/facts/predicate_type_formation_through_call_result_rejected` |
| 9 | fail wrong reason | source constructor predicate derivation failed: Kernel(UnboundLevelParameter { index: N, arity: N }) | `fail/facts/source_indexed_type_parameter_through_call_result_rejected` |
| 9 | pass/run rejected | machine-parameter requirement `_` requires contract constant expression call argument `_` does not form parameter `_` of | `pass/facts/abstract_signature_call_generic_actual_through_call_result_exit` |
| 9 | incomplete | `_` has no executable body: this compiler does not lower a state of it yet (normal result selection omits a completed so | `pass/facts/case_dependent_default_domain_through_call_result_exit` |
| 9 | pass/run rejected | cannot prove ensures contract for exit from from_bits at statement N: to_bits(result) == bits | `pass/facts/core_float_representation_through_call_result_exit` |
| 9 | pass/run rejected | statement N creates local borrow `_` while local borrow `_` is still active (borrowed at statement N) | `pass/facts/finite_family_exclusive_result/call_result` |
| 9 | pass/run rejected | cannot prove default-domain field requirement for call write from main::entry: parameter storage requires Storage::Issue | `pass/facts/non_owning_reference_erasure_through_call_result_exit` |
| 9 | pass/run rejected | native artifact target lowering failed: UnknownStructuralType(StructuralTypeId(N)) | `pass/facts/recast_parent_restoration_through_call_result_exit` |
| 9 | pass/run rejected | selected map disagrees with its supplying evidence bound | `pass/facts/representation_agreement_through_call_result_exit` |
| 9 | pass/run rejected | source constructor predicate derivation failed: Kernel(UnboundLevelParameter { index: N, arity: N }) | `pass/facts/source_indexed_type_parameter_through_call_result_exit` |
| 8 | pass/run rejected | cannot prove requires contract for call receive_inferred from main: Count == N (needs fact `_` here; establish it before | `pass/borrows/runtime_dependent_callback_window_byte_exit` |
| 8 | incomplete | `_` calls `_`, which calls `_`; `_` has no executable body: this compiler does not lower its declaration of it yet (stat | `pass/borrows/runtime_mutable_byte_loan_read_exit` |
| 8 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (statement sequence: local data (last dec | `pass/borrows/runtime_stored_carrier_exchange_exit` |
| 7 | fail wrong reason | no provider is selected for boundary requirement `_`, which the program entry reaches on target `_` | `fail/build/native_table_non_address_field` |
| 7 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a statement of it yet (statement sequence: local | `pass/drops/runtime_selected_linear_residual_death_exit` |
| 6 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "main", reason: "selected edge lost its  | `pass/calls/runtime_cyclic_state_reference_result_exit` |
| 6 | pass/run rejected | machine `_` ensures contract proof fact `_` speaks about proof-only `_`, which no entailment tier judges yet -- acceptin | `pass/cases/case_call_premises` |
| 6 | pass/run rejected | generic machine `_` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot  | `pass/expressions/seq_consuming_combinators` |
| 6 | pass/run rejected | state `_` receives `_` without an exact retained backing for its loan: the incoming descriptor has no retained loan occu | `pass/facts/data_where_standing_borrowed_field_array_const_name` |
| 6 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a statement of it yet (statement sequence: assig | `pass/facts/recoverable_replacement_window_through_const_name_exit` |
| 6 | pass/run rejected | returned view has no complete formation frontier | `pass/slices/runtime_indexed_descriptor_replacement_call_exit` |
| 5 | fail wrong reason | cannot complete a call at statement N in machine `_` state `_`: this write left the `_` facts of `_` (data `_`) unproved | `fail/arithmetic/bounded_assignment_unproven` |
| 5 | fail wrong reason | machine `_` state `_` statement N: reading `_` crosses an open default-domain invariant window before its declared quali | `fail/arithmetic/bounded_store_consumed_before_repair` |
| 5 | fail wrong reason | returned projection source permission is unavailable: the saved descriptor has no complete checked borrowing source; sou | `fail/borrows/returned_stored_view_reborrow_local_escape_rejected` |
| 5 | fail wrong reason | Exact integer cast in machine `_` state `_` from `_` to `_` is not provably representable; constrain the source with a r | `fail/facts/live_across_call_spill_through_other_state_rejected` |
| 5 | fail wrong reason | cannot prove index `_` is within unknown slice length of `_` in read_guarded::entry | `fail/facts/slice_view_element_read_bound_through_call_result_rejected` |
| 5 | pass/run rejected | state `_` receives `_` while local borrow `_` is still active | `pass/borrows/nested_reference_call_checked_index_exit` |
| 5 | pass/run rejected | call from `_` to `_` at statement N call N has an uncovered Trap crash route; publish a same-cause route whose guard cov | `pass/contracts/float_guard_disjoint_operand_write` |
| 5 | pass/run rejected | cannot form specification index in machine `_` requires contract: independent collection bounds are not established by p | `pass/modules/selected_constant_array_alias_computed_extent_exit` |
| 4 | fail wrong reason | machine `_` stores a borrowed value without a complete normal-exit backing relation | `fail/borrows/dynamic_store_local_backing_escape` |
| 4 | fail wrong reason | machine `_` ensures contract proof fact `_` is disproved by constant arithmetic on the transition arm guarded by `_` | `fail/build/helper_contract_context` |
| 4 | fail wrong reason | unknown canonical target profile `_` | `fail/contracts/target_family_recursive_guarantee` |
| 4 | fail wrong reason | exact arithmetic in machine `_` state `_` local `_` may overflow `_`: the operands are not provably in range (decision N | `fail/facts/data_where_standing_mutable_recast_through_call_result_rejected` |
| 4 | pass/run rejected | shift count in machine `_` state `_` return value is not provably below the operand width for `_`: exact shifts prove `_ | `pass/arithmetic/runtime_boolean_alias_shift_count_exit` |
| 4 | pass/run rejected | returned projection source permission is unavailable: the saved descriptor has no complete checked borrowing source; sou | `pass/borrows/returned_slice_record_reference_custody_exit` |
| 4 | pass/run rejected | terminating machine iterate: cannot prove rank range `_` for the rank produced by the selected view | `pass/borrows/runtime_projected_fixed_byte_array_local_exit` |
| 4 | pass/run rejected | cannot establish assignment invariants: source parent type is unavailable | `pass/borrows/runtime_reassigned_view_capture_exit` |
| 4 | pass/run rejected | selected provider requirement `_` has no emitted exact declaration candidate | `pass/facts/generic_calling_policy/ordinary_parent_fused_exit` |
| 4 | pass/run rejected | constant checking: scalar evaluation requires exact authored selection before evaluation; missing providers: [], externa | `pass/float/build_runtime_semantics_twins_windows_x64` |
| 4 | pass/run rejected | specification call `_` has no observation-free, reach-free, crash-free value denotation | `pass/float/public_class_result_contract_import_exit` |
| 4 | pass/run rejected | cannot prove ensures contract for exit from descend at statement N: result <= previous | `pass/proofs/runtime_ranked_forwarding_state_recursion_exit` |
| 4 | pass/run rejected | cannot prove requires contract for call middle from Main::main: N <= N && N <= main.bytes.len (needs fact `_` here; esta | `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` |
| 4 | pass/run rejected | machine structural equation remains unsolved in a noncall static argument | `pass/wire/library_compact_binary_packed_reflection_exit` |
| 3 | fail wrong reason | cannot use `_` in machine `_` state `_` (statement N) after `_` was transferred or consumed: it holds no value; read wha | `fail/borrows/case_payload_duplicate_transfer` |
| 3 | fail wrong reason | cannot establish assignment invariants: performed write has unknown destination origins in machine `_` state `_` at stat | `fail/borrows/conditional_reference_replacement_false_first_stale_index` |
| 3 | fail wrong reason | domain constraint `_` for the const leaf value is false; failed domain `_` | `fail/build/retained_domain_and_service_rows_invalid_index` |
| 3 | fail wrong reason | machine `_` state `_` terminal expression expects `_`, got `_` | `fail/facts/nominal_record_representation_through_call_result_rejected` |
| 3 | fail wrong reason | specification call `_` has no observation-free, reach-free, crash-free value denotation | `fail/float/public_class_contract_ieee_equality_retains_zero_sign` |
| 3 | fail wrong reason | terminating machine iterate: cannot prove rank range `_` for the rank produced by the selected view | `fail/termination/nested_helper_record_measure_result_equation_not_decreasing` |
| 3 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Main::main", reason: "selected edge los | `pass/arithmetic/modular_power_state_bounds_exit` |
| 3 | pass/run rejected | machine `_` cannot prove ensures contract proof fact `_` on the transition arm guarded by `_` | `pass/arithmetic/runtime_immutable_unsigned_bitwise_bounds_exit` |
| 3 | pass/run rejected | native artifact target lowering failed: UnsupportedControlFlow { machine: MachineId(N), site: withheld; set OMEGA_COMPIL | `pass/borrow/runtime_write_only_exclusive_reference_owned_wrapper_exit` |
| 3 | pass/run rejected | machine `_` state `_` statement N: cannot prove selected case for payload access `_`: no captured source place is availa | `pass/borrows/runtime_copied_reference_conditional_nominal_backing_exit` |
| 3 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a transition of it yet (state graph: terminator: | `pass/borrows/runtime_returned_nonzero_nested_window_exit` |
| 3 | pass/run rejected | cannot establish assignment invariants: performed write has unknown destination origins in machine `_` state `_` at stat | `pass/borrows/runtime_selected_reference_mutation_complement_exit` |
| 3 | pass/run rejected | native artifact callback entry ABI failed: native entry transport: native entry bridge failed: complete entry code uses  | `pass/build/descriptor_table_round_trip` |
| 3 | pass/run rejected | state `_` receives `_` without an exact retained backing for its loan: the access has no unique completed operand | `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` |
| 3 | pass/run rejected | state `_` receives `_` as both mutable and read-only | `pass/dependent/state_arrival_byte_predicate_guarded_compile` |
| 3 | pass/run rejected | Exact integer cast in machine `_` state `_` from `_` to `_` is not provably representable; constrain the source with a r | `pass/expressions/match_owned_computed_arm_arguments` |
| 3 | incomplete | `_` has no executable body: this compiler does not lower a state of it yet (state graph: structural return value: retain | `pass/facts/opaque_independent_inherited_result_through_local_exit` |
| 3 | incomplete | `_` has no executable body: this compiler does not lower its declaration of it yet (signature: authored result domains) | `pass/facts/opaque_independent_qualified_result_import_alias_exit` |
| 3 | pass/run rejected | machine `_` static argument N requires its declared binder kind, not an implementation | `pass/facts/static_callback_reach_evidence_scope` |
| 3 | pass/run rejected | boundary requirement `_` in package Some(PackageKeyIdentity([N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, | `pass/operators/crash_routes` |
| 3 | pass/run rejected | boundary operator `_` in package Some(PackageKeyIdentity([N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, | `pass/operators/runtime_named_unit_statement_effects_exit` |
| 3 | pass/run rejected | terminating machine walk: cannot prove rank range `_` for the rank produced by the selected view | `pass/proofs/kernel_integer_subtract_order` |
| 3 | pass/run rejected | unknown canonical target profile `_` | `pass/targets/private_target_supplies_remain_disclosed` |
| 3 | pass/run rejected | cannot prove published termination for machine `_`: its checked body reaches an operation without a usable termination g | `pass/termination/progress_selector_owned_arrival_replacement_helper_exit` |
| 3 | pass/run rejected | machine `_` state `_` uses `_`, which is not a declared local, parameter, field, or type in this state (check the spelli | `run/calls/outcome_specific_selected_guarantee_input_exit` |
| 3 | pass/run rejected | cannot prove every subslice range endpoint of `_` is non-negative | `run/ranges/borrowed_case_payload_alias` |
| 2 | fail wrong reason | call `_` has an invalid argument correspondence: call has no retained parameter telescope | `fail/borrows/predicate_named_call_keeps_installed_loan` |
| 2 | fail wrong reason | anonymous address value cannot prove fit in `_` in machine `_` state `_` return value | `fail/build/explicit_provider_scalar_budget` |
| 2 | fail wrong reason | `_` implements `_`, which neither is nor inherits `_`, the trait that declares `_` | `fail/calls/static_boundary_statement_unrelated_evidence` |
| 2 | fail wrong reason | machine call cycle: `_`, `_` -- no exact private-coordinate assignment proves every edge and complete cycle; runtime cal | `fail/contracts/generic_component_nondecreasing_window` |
| 2 | fail wrong reason | call from `_` to `_` at statement N call N has an uncovered Trap crash route; publish a same-cause route whose guard cov | `fail/core/guarded_trap_ceiling_other_site_rejected` |
| 2 | fail wrong reason | data `_` literal cannot prove the default domain: a `_` fact is not established by the values observed at this construct | `fail/dependent/case_where_unproved_bound_rejected` |
| 2 | fail wrong reason | machine `_` state `_`: recast source has no fixed representation inventory | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` |
| 2 | fail wrong reason | state `_` expects N argument(s), got N | `fail/facts/source_indexed_family_authored_terminating_state_cycle` |
| 2 | fail wrong reason | machine `_` state `_` cannot prove Exact float-to-`_` cast operand `_` preserves its denotation; prove an integral repre | `fail/float/exact_integer_coercion_requires_denotation` |
| 2 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "integ | `fail/generics/const_data_argument_out_of_range` |
| 2 | fail wrong reason | constructor obligations have no exact prepared application for `_`; unresolved generic members cannot waive field or cas | `fail/generics/native_locator_inconsistent_byte_extents` |
| 2 | fail wrong reason | exact arithmetic in value argument `_` of `_` may overflow `_`: the operands are not provably in range (decision N -- ex | `fail/generics/runtime_value_recipe_join_bound_loss` |
| 2 | fail wrong reason | terminal-artifact production failed: Lowering(CompilerEvaluationAdmission("machine `_` is not build-time admissible: a c | `fail/layouts/declaration_only_initializer_formation` |
| 2 | fail wrong reason | machine `_` state `_` local `_` uses runtime value `_` for an inline fixed-array extent; a value index cannot determine  | `fail/modules/array_extent_local_shadow` |
| 2 | fail wrong reason | statement N moves `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N) | `fail/ownership/selected_source_forwarded_live_call_carrier` |
| 2 | fail wrong reason | cannot prove index `_` is within unknown slice length of `_` in radiance::entry | `fail/proofs/field_view_index_missing_bound` |
| 2 | fail wrong reason | terminal call `_` does not select a declared machine entry or callable requirement | `fail/proofs/inductive_climbing_sum_unbounded_accumulator` |
| 2 | fail wrong reason | cannot form specification operator `_` in trait `_` state `_` ensures contract: no exact selected meaning for operand ty | `fail/proofs/result_index_abstract_bound_operator` |
| 2 | fail wrong reason | integer node `_` does not fit exact carrier `_` | `fail/providers/applied_binding_unused_const_overflow` |
| 2 | fail wrong reason | assumed binding requirement absent from canonical artifact | `fail/providers/assumed_result_bound_equal_count_lifetime_mapping_rejected` |
| 2 | fail wrong reason | returned view has no complete formation frontier | `fail/slices/stored_view_backing_mutated_while_returned_live` |
| 2 | fail wrong reason | cannot prove published termination for machine `_`: its checked body reaches an operation without a usable termination g | `fail/termination/native_layout_family_circular` |
| 2 | fail wrong reason | proof-only machine call cycle: `_`, `_` -- no complete structural ranking establishes strict descent on every cycle; exa | `fail/termination/proof_joint_machine_cycle_nondecreasing` |
| 2 | pass/run rejected | call to `_` has operational envelope `_` but acknowledges neither suspension nor blocking; call acknowledgements must ma | `pass/blockexec/blocking_executor_custody_claims_compile` |
| 2 | pass/run rejected | machine `_` state `_` expression type application uses undeclared lifetime argument `_`Carrier`; declare it in the owner | `pass/borrows/carried_loan_replacement` |
| 2 | pass/run rejected | returned projection does not match this invocation and result path | `pass/borrows/dynamic_window_return_custody_exit` |
| 2 | pass/run rejected | temporary backing lifetime coverage is unavailable for an operand of `_` in state `_` at statement N | `pass/borrows/inline_carried_reference_exit` |
| 2 | pass/run rejected | returned projection has no matching explicit argument borrow | `pass/borrows/runtime_computed_view_observation_composition_exit` |
| 2 | pass/run rejected | state `_` receives `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N) | `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` |
| 2 | pass/run rejected | constant checking: machine `_` is not build-time admissible: the checked Trap crash route can occur; constant evaluation | `pass/build/component_declaration_scalar_conditions_exit` |
| 2 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "gener | `pass/build/const_index_dependency_order_exit` |
| 2 | pass/run rejected | native artifact physical program entry failed: semantic entry requires an explicitly selected checked physical adapter | `pass/build/uefi_entry_schema_same_leaf` |
| 2 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "consume", reason: "selected edge lost i | `pass/calls/nonreturning_record_match_normal_exit` |
| 2 | pass/run rejected | machine call cycle: `_` can re-enter itself through a selector call or boundary requirement call; termination across imp | `pass/capabilities/selected_applied_synchronous_chain_exit` |
| 2 | pass/run rejected | selected contract has no unique supplying map application | `pass/constants/direct_named_implementation_initializer_exit` |
| 2 | pass/run rejected | construction of `_` field `_` is not proven in domain `_` in consume state `_` at statement N; a construction value must | `pass/domains/domain_pattern_match_bound_full_width_field_exit` |
| 2 | pass/run rejected | data `_` literal cannot prove the default domain: a `_` fact is not established by the values observed at this construct | `pass/domains/private_case_payload_construction` |
| 2 | incomplete | `_` calls `_`, which calls `_`; `_` has no executable body: this compiler does not lower a transition of it yet (state g | `pass/facts/computed_record_window_through_other_machine_exit` |
| 2 | pass/run rejected | cannot prove requires contract for call choose from apply: visits == N (needs fact `_` here; establish it before the cal | `pass/facts/exclusive_match_arm_through_other_machine_exit` |
| 2 | pass/run rejected | machine `_` stores a borrowed value without a complete normal-exit backing relation | `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` |
| 2 | pass/run rejected | cannot prove requires contract for call compute from apply: visits == N (needs fact `_` here; establish it before the ca | `pass/facts/match_computed_root_through_other_machine_exit` |
| 2 | pass/run rejected | cannot prove requires contract for call read_field from apply: match choose() { true => chosen, false => spare }.kept == | `pass/facts/match_join_place_read_through_other_machine_exit` |
| 2 | pass/run rejected | cannot prove requires contract for call read_origin from apply: joined == N (needs fact `_` here; establish it before th | `pass/facts/match_primitive_local_origin_through_other_machine_exit` |
| 2 | pass/run rejected | cannot prove requires contract for call read_through from apply: match choose() { true => chosen, false => spare } == N  | `pass/facts/match_reference_referent_through_other_machine_exit` |
| 2 | pass/run rejected | selected descriptor has no evaluated carrier custody | `pass/facts/mutable_slice_element_write_bound_through_match_join_exit` |
| 2 | pass/run rejected | native artifact component entry ABI failed: native entry transport: native entry bridge failed: complete entry code uses | `pass/facts/opaque_independent_closed_carrier_through_other_machine_exit` |
| 2 | incomplete | `_` calls `_`, which calls `_`; `_` has no executable body: this compiler does not lower a statement of it yet (statemen | `pass/facts/recoverable_replacement_window_through_call_result_exit` |
| 2 | pass/run rejected | source induction case arithmetic is unsupported: Kernel(UnboundLevelParameter { index: N, arity: N }) | `pass/facts/source_indexed_type_parameter_exit` |
| 2 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/oth | `pass/facts/target_family_callback_guarantee/other_machine` |
| 2 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/other | `pass/facts/target_family_contract_operand/other_machine` |
| 2 | pass/run rejected | cannot prove requires contract for call finite_value from main: FN.is_finite(((N as uN ) + (N as uN ) as fN)) (needs fac | `pass/float/closed_integer_conversion_trapping_source_no_normal_result` |
| 2 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (call operation: scalar arguments); in st | `pass/float/runtime_exact_format_narrowing_exit` |
| 2 | pass/run rejected | terminal-artifact production failed: Lowering(CompilerEvaluationAdmission("machine `_` is not build-time admissible: a c | `pass/layouts/declaration_only_initializer_formation` |
| 2 | pass/run rejected | cannot determine the dispatch subject storage type in `_` | `pass/layouts/runtime_plan_laid_erased_field_exit` |
| 2 | pass/run rejected | duplicate machine `_` | `pass/layouts/runtime_recast_reference_captures_index_exit` |
| 2 | pass/run rejected | cannot prove ensures contract for exit from HeapProvider::allocate at statement N: result in Allocation::Held; the retur | `pass/memory/runtime_linear_native_allocation_call_result_exit` |
| 2 | pass/run rejected | state `_` receives `_` as mutable more than once | `pass/recast/runtime_inline_slice_view_mutable_write_exit` |
| 2 | pass/run rejected | machine `_` state `_` statement N: reading `_` crosses an open default-domain invariant window before its declared quali | `pass/recast/runtime_mutable_equivalent_range_recast_exit` |
| 2 | incomplete | `_` has no executable body: this compiler does not lower a transition of it yet (state loan parents: incomplete captured | `pass/slices/runtime_cut_byte_view_cycle_exit` |
| 2 | pass/run rejected | returned projection exceeds its source presentation | `pass/slices/runtime_stored_element_view_replacement_exit` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from rejected at statement N: result <= values.len | `fail/arithmetic/collection_result_changed_predecessor` |
| 1 | fail wrong reason | store in machine `_` state `_` assignment implicitly converts `_` to `_`; a typed value keeps its type and arithmetic po | `fail/arithmetic/exact_overflow_value_call_hint` |
| 1 | fail wrong reason | division by zero in machine `_` state `_` return value: the divisor is provably zero, which always traps at runtime. Rem | `fail/arithmetic/indexed_divisor_copied_selector_storage_write` |
| 1 | fail wrong reason | store in machine `_` state `_` local `_` implicitly converts `_` to `_`; a typed value keeps its type and arithmetic pol | `fail/arithmetic/wrapping_target_plain_operands_rejected` |
| 1 | fail wrong reason | partial exact remainder in machine `_` state `_` local `_`: the divisor must be proven nonzero; discharge every definedn | `fail/arithmetic/zii_range_excludes_zero_rejected` |
| 1 | fail wrong reason | state `_` receives `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N) | `fail/borrows/aliased_forwarded_placed_views` |
| 1 | fail wrong reason | normal call `_` (Handle { arena_index: N, generation: N }) in machine `_` state `_` statement N: completed result refere | `fail/borrows/evidence_bound_record_result_keeps_loan` |
| 1 | fail wrong reason | temporary backing lifetime coverage is unavailable for an operand of `_` in state `_` at statement N | `fail/borrows/inline_carried_reference_holds_backing` |
| 1 | fail wrong reason | statement N reads `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N) | `fail/borrows/nested_machine_carried_live_child_rejected` |
| 1 | fail wrong reason | cannot prove requires contract for call observe from main: cells[N] in Cell::Issued in state `_` | `fail/borrows/owned_capture_weaker_formal_qualification` |
| 1 | fail wrong reason | returned projection exceeds its source presentation | `fail/borrows/owned_carrier_result_retains_whole_operand` |
| 1 | fail wrong reason | cannot establish assignment invariants: source parent type is unavailable | `fail/borrows/reassigned_view_conflicts_with_captured_child` |
| 1 | fail wrong reason | machine `_` state `_` expression type application uses undeclared lifetime argument `_`Carrier`; declare it in the owner | `fail/borrows/replaced_carrier_keeps_new_backing_loan` |
| 1 | fail wrong reason | returned projection does not match this invocation and result path | `fail/borrows/runtime_slice_result_retains_backing` |
| 1 | fail wrong reason | cannot use `_`: this selected region was transferred or has not been established in machine `_` state `_` | `fail/borrows/scalar_operand_consumes_reference_carrier_once` |
| 1 | fail wrong reason | checked borrow resource requires exactly one activation and one weakening (state Handle { arena_index: N, generation: N  | `fail/borrows/selected_reference_record_helper_wrong_lifetime` |
| 1 | fail wrong reason | Terminal Build `_` failed: artifact-only Build requires at least one completed output | `fail/build/authored_resolution_source_location` |
| 1 | fail wrong reason | failed to resolve tests/omega/fail/build/build_effects_undeclared/omega_language_std/console.omg: captured entry is abse | `fail/build/build_effects_undeclared` |
| 1 | fail wrong reason | Terminal Build `_` failed: Build activation has unexpected scalar parameters | `fail/build/build_machine_wrong_arity` |
| 1 | fail wrong reason | machine satisfaction target `_` does not resolve to an exact trait requirement or top-level `_` | `fail/build/build_satisfier_trait_signature_mismatch` |
| 1 | fail wrong reason | calling policy demands: TargetMachineSelection(MissingMember { head: Boundary(BoundaryMachineId(N)), target: "linux_xN_N | `fail/build/calling_policy_missing_target_member` |
| 1 | fail wrong reason | failed to resolve tests/omega/fail/build/product_entry_query_foreign_helper_private_entry/support/configure.omg: capture | `fail/build/product_entry_query_foreign_helper_private_entry` |
| 1 | fail wrong reason | failed to resolve tests/omega/fail/build/root_binding_foreign_helper_private_entry/support/configure.omg: captured entry | `fail/build/root_binding_foreign_helper_private_entry` |
| 1 | fail wrong reason | N authored behavior exclusion(s) could not be satisfied | `fail/build/static_service_exclusion_invoked` |
| 1 | fail wrong reason | source adapter failed for product dependency row N of package `_`: cannot establish canonical external source path `_`:  | `fail/build/unjoined_executed_dependency` |
| 1 | fail wrong reason | `_` here is NON-TAIL self-recursion: the result feeds the surrounding computation, so the frame must survive the call. R | `fail/calls/final_value_recursive_operand_is_not_tail` |
| 1 | fail wrong reason | `_` expects N static invocation argument(s), got N; explicit selection requires a named implementation as the final stat | `fail/calls/static_boundary_statement_misplaced_evidence` |
| 1 | fail wrong reason | call to `_` has operational envelope neither suspension nor blocking but acknowledges `_`; call acknowledgements must ma | `fail/calls/terminal_state_jump_block_acknowledgement` |
| 1 | fail wrong reason | cannot form specification operator `_` in data `_` field `_`: no exact selected meaning for operand types [uN, bool] | `fail/comptime/array_expression_unused_invalid_call_operand` |
| 1 | fail wrong reason | fixed-array length `_`: const evaluation of `_` failed: data `_` literal violates the default domain: a `_` fact evaluat | `fail/comptime/generic_constructor_field_demand_extent_domain_violation` |
| 1 | fail wrong reason | constant checking: scalar predicate is false: requires contract for call take_bounded from main state entry statement N | `fail/comptime/recipe_endpoint_membership_refused_at_closure` |
| 1 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "const | `fail/constants/predicate_constant_generic_static_index_fence` |
| 1 | fail wrong reason | cannot prove requires contract for call demand_identity from guarded: FloatingEquality.equals(left, right) (needs fact ` | `fail/contracts/executable_float_box_equality_not_identity` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from wrong at statement N: result.code == N && original.value != N | `fail/contracts/normal_return_written_state_premise` |
| 1 | fail wrong reason | match structural pattern must mention every declared field or explicitly use `_` | `fail/control_flow/arm_pattern_missing_field` |
| 1 | fail wrong reason | match field `_` is missing, duplicated, or belongs to another record/case | `fail/control_flow/arm_pattern_rest_unknown_field` |
| 1 | fail wrong reason | machine `_` state `_` terminal call to `_` completes with its `_` result, but the state returns `_`; a terminal machine  | `fail/control_flow/state_result_differs_from_machine_result` |
| 1 | fail wrong reason | machine `_` state `_` transition returns a value, but the state declares no result; a terminal arm is `_` | `fail/control_flow/transition_value_without_result` |
| 1 | fail wrong reason | uncovered Trap crash in machine `_` state `_` cast expression: Trapping `_` of `_` and `_` in `_` traps when its result  | `fail/core/guarded_trap_ceiling_unbounded_addend_rejected` |
| 1 | fail wrong reason | cannot form specification operator `_` in data `_` case `_` predicate: no exact selected meaning for operand types [<una | `fail/dependent/case_where_sibling_payload_scope` |
| 1 | fail wrong reason | machine `_` cannot lend restricted field `_` to `_`: the selected scalar referent's normal-return guarantees do not pres | `fail/dependent/data_where_live_borrow_pins_witness_rejected` |
| 1 | fail wrong reason | domain `_` predicate constant expression call argument `_` does not form parameter `_` of `_`: actual [uN], required [uN | `fail/domains/byte_predicate_declared_callee_crash` |
| 1 | fail wrong reason | domain `_` proof fact `_` must have type `_` | `fail/domains/nonboolean_member_predicate` |
| 1 | fail wrong reason | cannot prove the value assigned to `_` in replace is in domain `_`; a place declared `_` requires every write to be esta | `fail/domains/routed_primitive_receiver_requires_issuer` |
| 1 | fail wrong reason | named machine call `_` has no overload for result dispatch set `_` and parameter signature `_` | `fail/domains/terminal_result_domain_missing_overload` |
| 1 | fail wrong reason | cannot prove scalar result domain for return from missing at statement N: iN::Issued | `fail/domains/terminal_result_domain_unestablished` |
| 1 | fail wrong reason | cannot prove scalar result domain for return from missing at statement N: iN::Positive | `fail/domains/terminal_result_predicate_unproved` |
| 1 | fail wrong reason | cannot prove automatic cleanup requires at selected edge from formed state entry after statement N: Token does not estab | `fail/drops/temporary_cleanup_later_state_initializer` |
| 1 | fail wrong reason | machine `_` state `_` uses `_`, which is not a declared local, parameter, field, or type in this state (check the spelli | `fail/expressions/nested_i32_mul_overflow` |
| 1 | fail wrong reason | selected claim disagrees with its evaluated source custody | `fail/expressions/owned_match_duplicated_projected_claim` |
| 1 | fail wrong reason | selected descriptor has no live evaluated carrier custody | `fail/expressions/owned_match_subject_consumed_source` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from from_bits at statement N: to_bits(result) == bits | `fail/facts/core_float_representation_through_const_name_rejected` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from extract_payload at statement N: result >= N && result <= N | `fail/facts/four_variant_enum_payload_bound_through_call_result_rejected` |
| 1 | fail wrong reason | data `_` cannot bind implementation `_`: conformance application `_` supplies undeclared lifetime argument `'selected';  | `fail/facts/implementation_region_forwarding_through_target_body_rejected` |
| 1 | fail wrong reason | conformance `_` parameter `_` requires a const argument, but `_` has another static category | `fail/facts/open_record_const_implementation_specialization_agreement_rejected` |
| 1 | fail wrong reason | cannot prove requires contract for call inspect_bytes from XNEvaluateBytes::evaluate_bytes: [N, N, N, N].len >= N && [N, | `fail/facts/readonly_byte_pool_relocation_bound_through_target_body_rejected` |
| 1 | fail wrong reason | applied callable has an unrepresented static type operand | `fail/facts/selected_value_application_through_field_rejected` |
| 1 | fail wrong reason | cannot prove requires contract for call verify_shifted from XNVerifyTarget::verify_target: shifted >= N && shifted <= N  | `fail/facts/signed_8bit_shift_right_bound_through_target_body_rejected` |
| 1 | fail wrong reason | constant checking: inconsistent captured contract bounds | `fail/facts/size_align_layout_geometry_through_match_join_rejected` |
| 1 | fail wrong reason | cannot prove index `_` is within unknown slice length of `_` in target_reader::entry | `fail/facts/slice_view_element_read_bound_through_other_machine_rejected` |
| 1 | fail wrong reason | cannot prove index `_` is within unknown slice length of `_` in check_view::entry | `fail/facts/slice_view_element_read_bound_through_other_state_rejected` |
| 1 | fail wrong reason | cannot prove index `_` is within unknown slice length of `_` in XNReadGuarded::read_guarded::read_guarded | `fail/facts/slice_view_element_read_bound_through_target_body_rejected` |
| 1 | fail wrong reason | source induction case arithmetic is unsupported: Kernel(UnboundLevelParameter { index: N, arity: N }) | `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected` |
| 1 | fail wrong reason | `_` does not satisfy `_`: helper `_` requires `_`, but the implementation promises `_` | `fail/facts/static_callback_reach_requirement_telescope_requirement` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from produced at statement N: result.value == N | `fail/facts/temporary_case_cleanup_through_call_result_rejected` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from forward at statement N: result.value == N | `fail/facts/temporary_case_cleanup_through_other_machine_rejected` |
| 1 | fail wrong reason | cannot prove requires contract for call verify_prod from XNEvaluateProd::evaluate_prod: prod >= N && prod <= N (needs fa | `fail/facts/unsigned_64bit_wrapping_multiplication_bound_through_target_body_rejected` |
| 1 | fail wrong reason | implicit domain weakening in argument `_` for state `_` drops `_`; semantic meaning, routed provenance, and arithmetic p | `fail/float/authored_heterogeneous_float_operator_requires` |
| 1 | fail wrong reason | cannot prove call argument `_` satisfies `_` for bounded parameter `_` for `_` in `_` | `fail/float/finite_store_invalidated` |
| 1 | fail wrong reason | builtin `_` cannot combine `_` and `_`; a landed float retains its format, so changing format requires an explicit conve | `fail/float/mixed_float_comparison_formats` |
| 1 | fail wrong reason | constant checking: scalar evaluation requires exact authored selection before evaluation; missing providers: [], externa | `fail/float/named_nan_runtime_materialization` |
| 1 | fail wrong reason | named operator `_` is not available in this source's import scope | `fail/float/primitive_comparison_no_named_helper_exposure` |
| 1 | fail wrong reason | store in construction of `_` field `_` implicitly converts `_` to `_`; a typed value keeps its type and arithmetic polic | `fail/float/requirement_record_field_trapping_requires_coverage` |
| 1 | fail wrong reason | generic machine `_` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot  | `fail/generics/const_container_mismatched_witness` |
| 1 | fail wrong reason | terminating machine does_not_finish reaches recursive component `_`, which has no `_` ranking -- spell one on that compo | `fail/generics/core_slice_for_each_callback_termination` |
| 1 | fail wrong reason | case-membership operation lost its exact subject, carrier or case identity | `fail/generics/extraction_foreign_case` |
| 1 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "anony | `fail/generics/grouped_decimal_const_fraction_rejected` |
| 1 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "close | `fail/generics/grouped_decimal_const_suffix_rejected` |
| 1 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "divis | `fail/generics/grouped_decimal_const_zero_division_rejected` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from forwarded at statement N: result >= N | `fail/generics/open_const_wrong_literal_tuple` |
| 1 | fail wrong reason | const argument `_` has no static value with its exact declared carrier | `fail/generics/record_const_foreign_nominal_carrier` |
| 1 | fail wrong reason | machine `_` state `_` cannot prove runtime index subject equality at statement N; an indexed value retains its captured  | `fail/generics/runtime_index_carrier_match_mismatch` |
| 1 | fail wrong reason | constant checking: machine `_` is not build-time admissible: the checked Trap crash route can occur; constant evaluation | `fail/layouts/declaration_only_array_formation` |
| 1 | fail wrong reason | constant checking: scalar predicate is false: requires contract for call require_geometry from program: size_of() + alig | `fail/layouts/generic_constant_layout_contract` |
| 1 | fail wrong reason | `_` names an unknown access policy `_` | `fail/layouts/placed_primitive_private_unused` |
| 1 | fail wrong reason | accessor write application for `_` with value `_`: the access plan does not permit this operation | `fail/layouts/placed_stable_array_readonly_write` |
| 1 | fail wrong reason | generic data `_` expected N arguments but got N | `fail/layouts/plan_laid_policy_without_plan_machine` |
| 1 | fail wrong reason | failed scalar accounting probe missed its authored refusal | `fail/layouts/portable_layout_constant_composition` |
| 1 | fail wrong reason | accessor write application for `_` with value `_`: value type does not match the accessor's exact value type | `fail/layouts/resident_field_write_preserves_qualification` |
| 1 | fail wrong reason | constant checking: scalar predicate is false: requires contract for call require_layout from main: size_of() < N // size | `fail/layouts/retained_layout_predicate_decisions` |
| 1 | fail wrong reason | construction of `_` field `_` lost its exact declared owner | `fail/layouts/size_of_uninstantiated_plan_laid` |
| 1 | fail wrong reason | duplicate machine `_` | `fail/layouts/target_layout_call_checks_all_widths` |
| 1 | fail wrong reason | constant checking: scalar predicate is false: requires contract for call impossible from scoped: size_of() == N | `fail/layouts/unselected_target_layout_contract` |
| 1 | fail wrong reason | named call `_` is ambiguous between declaration namespaces | `fail/modules/ambiguous_numeric_callable_not_builtin` |
| 1 | fail wrong reason | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "index | `fail/modules/packaged_structured_const_wrong_carrier` |
| 1 | fail wrong reason | constant initializer: constant initializer result does not match its declared carrier | `fail/modules/selected_constant_nominal_call_mismatch` |
| 1 | fail wrong reason | callable `_` has an invalid content-conservation contract: content conservation requires the sealed typed core::Identity | `fail/ownership/selected_storage_join_unrelated_root` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from wrong at statement N: result <= count | `fail/proofs/changed_arrival_cannot_rebind_original_bound` |
| 1 | fail wrong reason | cannot prove subslice range end bound `_` is within slice length N | `fail/proofs/field_view_constructor_geometry` |
| 1 | fail wrong reason | exact arithmetic in machine `_` ensures contract may overflow `_`: the operands are not provably in range (decision N -- | `fail/proofs/inductive_climbing_sum_step_false_twin` |
| 1 | fail wrong reason | terminating machine walk: cannot prove rank range `_` for the rank produced by the selected view | `fail/proofs/kernel_integer_subtract_order_false_twin` |
| 1 | fail wrong reason | cannot form specification operator `_` in machine `_` ensures contract: no exact selected meaning for operand types [[Va | `fail/proofs/result_index_authored_operator` |
| 1 | fail wrong reason | cannot form specification operator `_` in machine `_` ensures contract: no exact selected meaning for operand types [<un | `fail/proofs/result_index_lexical_operator` |
| 1 | fail wrong reason | cannot prove requires contract for call bounded from main: selected(N) < N && N < N (needs fact `_` here; establish it b | `fail/proofs/structural_application_premise_false_rejected` |
| 1 | fail wrong reason | state `_` receives `_` as both mutable and read-only | `fail/providers/borrowed_membership_current_subject` |
| 1 | fail wrong reason | boundary qualification is refuted at exit of Direct::issue in state `_` for requirement `_`: Token::Issued | `fail/providers/boundary_issuance_refuted_completed_value` |
| 1 | fail wrong reason | boundary qualification is refuted at exit of Provider::issue in state `_` for requirement `_`: Token::Issued | `fail/providers/boundary_issuance_refuted_mutable_value` |
| 1 | fail wrong reason | constant checking: scalar evaluation requires exact authored selection before evaluation; missing providers: [BoundaryMa | `fail/providers/checked_boundary_extent_index_out_of_bounds` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower its declaration of it yet (boundary requirement: open trait app | `fail/providers/inherited_provider_erased_regions` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower its declaration of it yet (boundary requirement: result shape o | `fail/providers/qualification_inherited_grants_ambiguous` |
| 1 | fail wrong reason | cannot prove requires contract for call read from wrong: value in package:NdNefeNbbNcNcNbbNeNefNbfNaNecNeNaNbNbNaNabN:pr | `fail/providers/qualified_foreign_result_forwarding_instance` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from wrong at statement N: result.code == N | `fail/providers/qualified_foreign_result_forwarding_state` |
| 1 | fail wrong reason | Product provider operand `_` does not resolve to one visible declaration of role BoundarySubject | `fail/providers/service_nonboundary_requirement_rejected` |
| 1 | fail wrong reason | machine `_` state `_`: byte-region recast target `_` must be recursively fact-free; unchecked bytes cannot establish con | `fail/recast/recast_mut_record_fact_fenced` |
| 1 | fail wrong reason | argument `_` for state `_` expects `_`, got `_`; expected carrier `_`, declared place carrier `_` | `fail/slices/empty_fixed_array_slice_wrong_element` |
| 1 | fail wrong reason | cannot prove ensures contract for exit from from_join at statement N: result.len == N | `fail/slices/matched_window_end_join_disagrees` |
| 1 | fail wrong reason | cannot prove requires contract for call middle from Main::main: N <= N && N <= main.bytes.len (needs fact `_` here; esta | `fail/slices/namespace_range_keeps_bounds` |
| 1 | fail wrong reason | cannot prove requires contract for call require_all from Main::main: main.bytes[N..main.limit.len].len == N (needs fact  | `fail/slices/record_length_field_is_not_collection_extent` |
| 1 | fail wrong reason | build selects provider `_` for unknown boundary slot `_`; the slot must exist in the loaded dependency closure | `fail/targets/reached_foreign_body_through_selected_provider_rejected` |
| 1 | fail wrong reason | machine call cycle: `_`, `_`, `_` -- a private member has no compatible natural rank input; runtime calls must be tail;  | `fail/termination/joint_rankless_finish_reenters_component` |
| 1 | fail wrong reason | machine call cycle: `_`, `_` -- a preserving cycle has no strict measure DECREASE; runtime calls must be tail; a termina | `fail/termination/lexicographic_component_zero_step_rejected` |
| 1 | fail wrong reason | terminating machine iterate: cannot prove strict rank decrease on a recurrent arrival: the transition from `_` back to i | `fail/termination/rank_arrival_sibling_does_not_replace_subject` |
| 1 | fail wrong reason | tests/omega/fail/termination/retired_block_form/main.omg:N:N: expected `_`, found identifier `_` | `fail/termination/retired_block_form` |
| 1 | fail wrong reason | machine call cycle: `_`, `_` -- an internal call occurrence lacks a classified tail edge; runtime calls must be tail; a  | `fail/termination/runtime_non_tail_joint_machine_cycle` |
| 1 | fail wrong reason | machine call cycle: `_`, `_`, `_` -- no exact private-coordinate assignment proves every edge and complete cycle; runtim | `fail/termination/stale_state_rank_premise_rejected` |
| 1 | fail wrong reason | cannot prove default-domain field requirement for call corrupt from main::entry: parameter bytes requires [uN; N]::Ascii | `fail/text/borrowed_bounded_byte_invariant_write` |
| 1 | fail wrong reason | machine call cycle: `_` can re-enter itself through a selector call or boundary requirement call; termination across imp | `fail/traits/applied_requirement_lexical_isolation_missing` |
| 1 | fail wrong reason | cannot prove requires contract: machine `_` state `_` value call to `_` violates required fact `_`: the instantiated fac | `fail/traits/dyn_finite_family_narrowed_provider` |
| 1 | fail wrong reason | machine `_` static argument N requires a named implementation | `fail/traits/evidence_binder_given_data_type` |
| 1 | fail wrong reason | `_` is bound as `_`, not the `_` this call of `_` names; expected [named(name(nominal\\(package-owner\\(N:NaceNebNfNfNaN | `fail/traits/explicit_implementation_binder_trait_arguments` |
| 1 | fail wrong reason | `_` implements `_`, not the `_` this call of `_` names; expected [named(name(iN)), generic(name(nominal\\(package-owner\ | `fail/traits/explicit_implementation_const_identity` |
| 1 | fail wrong reason | `_` implements `_`, not the `_` this call of `_` names; expected [named(name(nominal\\(package-owner\\(N:NcNfNccdNdNeNfN | `fail/traits/explicit_implementation_inherited_trait_arguments` |
| 1 | fail wrong reason | `_` implements `_`, not the `_` this call of `_` names; expected [named(name(nominal\\(package-owner\\(N:NcNfNdNedNdfNac | `fail/traits/explicit_implementation_wrong_trait_arguments` |
| 1 | fail wrong reason | machine `_` state `_` does not satisfy trait `_` machine `_`: conformance borrow contract: callable type does not preser | `fail/traits/prepared_callable_wrong_member_source` |
| 1 | fail wrong reason | machine `_` proves no guarantee for the selected comparison law `_` | `fail/traits/public_guarantee_wrong_formal` |
| 1 | pass/run rejected | shift count in machine `_` state `_` assignment is not provably below the operand width for `_`: exact shifts prove `_`  | `pass/arithmetic/runtime_exact_guarded_shift_count_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from from_field at statement N: result >= reading.value - N && result <= reading. | `pass/arithmetic/truncating_reconstruction_bound` |
| 1 | pass/run rejected | linear value `_` reaches scope exit without being consumed or transferred in machine `_` state `_` | `pass/asm/atomic_release_disposition_state_exit` |
| 1 | pass/run rejected | cannot prove asm `_` fact at block exit in compare$assembly$linux_xN_N$N$entry state `_` (statement N): match output { H | `pass/asm/checked_asm_saved_scalar_match_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from write_eoi_by_choice at statement N: proposition:fact:model_specific_written( | `pass/asm/instruction_disposition_native` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from Confirmation::finish at statement N: proposition:fact:atomic_release_written | `pass/atomics/selected_reference_atomic_provider` |
| 1 | pass/run rejected | cannot prove requires contract for call observe from main: first[N] in Cell::Issued in state `_` | `pass/borrow/owned_capture_replaced_reference` |
| 1 | pass/run rejected | cannot prove requires contract for call observe from main: cells[N] in Cell::Issued in state `_` | `pass/borrow/owned_capture_weaker_formal` |
| 1 | pass/run rejected | argument `_` for state `_` expects `_`, got `_`; expected carrier `_`, declared place carrier `_` | `pass/borrow/runtime_boundary_receiver_disjoint_loan_exit` |
| 1 | pass/run rejected | view formation has incomplete permission suppliers at its captured source | `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit` |
| 1 | pass/run rejected | tests/omega/pass/borrows/borrow_guarded_window_write_and_call/main.omg:N:N: expected one of `_`, `_`, `_`, `_`, `_`, `_` | `pass/borrows/borrow_guarded_window_write_and_call` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a state of it yet (reference retirement: stored occurrences); i | `pass/borrows/copied_intermediate_descriptor_snapshot_exit` |
| 1 | pass/run rejected | completed output has no captured lifetime-input receipt; completed output from `_` in machine `_`, state `_` (Handle { a | `pass/borrows/foreign_output_captured_reference_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "main", reason: "loan retirement require | `pass/borrows/generic_record_slice_referent_exit` |
| 1 | pass/run rejected | statement N cannot resolve the complete borrowed destination of `_`: the incoming descriptor has no retained loan occurr | `pass/borrows/nested_reference_write_disjoint_facts_exit` |
| 1 | pass/run rejected | statement N borrows `_` while local borrow `_` is still active (borrowed at statement N) in machine `_`, state `_`, stat | `pass/borrows/overwritten_intermediate_descriptor_releases_backing_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call replace from snapshot_before_mutation::entry: parameter resource  | `pass/borrows/qualified_owner_state_loan_exit` |
| 1 | pass/run rejected | cannot establish assignment invariants: partial write enclosure has no exact active-case frontier | `pass/borrows/returned_record_case_field_store_exit` |
| 1 | pass/run rejected | statement N borrows `_` without an exact retained backing for its loan in machine `_`, state `_`, statement N | `pass/borrows/runtime_computed_window_operand_order_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "swap::rotate", reason: "owned edge tran | `pass/borrows/runtime_copyable_carrier_retirement_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from refresh at statement N: ticket in Ticket::Issued | `pass/borrows/runtime_dropped_reference_qualification_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value from Main::main: observed >= N && observed <= N (needs fact ` | `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit` |
| 1 | pass/run rejected | machine `_` cannot lend restricted field `_` to `_`: the selected scalar referent's normal-return guarantees do not pres | `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(OverlappingProjectedCustody { reason: "projected custody overlaps an earli | `pass/borrows/runtime_partial_reference_partition_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value from Main::main: first >= N && first <= N (needs fact `_` her | `pass/borrows/runtime_shared_reference_array_cohort_restoration_exit` |
| 1 | pass/run rejected | native artifact identity physical pipeline failed: common physical staging failed: Selection(Legalization(DescriptorCall | `pass/borrows/runtime_shared_view_record_presentation_exit` |
| 1 | pass/run rejected | machine satisfaction target `_` does not resolve to an exact trait requirement or top-level `_` | `pass/build/build_satisfier_trait_retention_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call checked_value from build: no_wrap(N, N) (needs fact `_` here; establish it befor | `pass/build/checked_context_core_identity_exit` |
| 1 | pass/run rejected | quoted byte literal has N source byte(s), but owned fixed byte array `_` requires exactly N; fixed-array literal copying | `pass/build/dependency_generated_source_unimported` |
| 1 | pass/run rejected | access view `_`: schema data `_` field `_` has no reflected storage geometry: representation recipe has no admitted layo | `pass/build/dependency_implementation_closure` |
| 1 | pass/run rejected | source adapter failed for product dependency row N of package `_`: cannot retain Git pin for `_` at `_`: offline resolut | `pass/build/executed_git_source_join_exit` |
| 1 | pass/run rejected | anonymous address value cannot prove fit in `_` in machine `_` state `_` return value | `pass/build/explicit_provider_storage_extent_exit` |
| 1 | pass/run rejected | fixed-array length `_`: const evaluation of `_` failed: store in machine `_` state `_` argument `_` implicitly converts  | `pass/build/selected_const_array_length_exit` |
| 1 | pass/run rejected | copied call lost its ordinary and realized Value operand roster | `pass/calls/native_code_layout_forward_machine_exit` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower its declaration of it yet (state graph: result signature: scala | `pass/calls/outcome_specific_mixed_snapshot` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "main", reason: "owned membership transp | `pass/calls/runtime_borrowed_parameter_ensures_issuer_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call observe from exercise: constructor:N:package:NfNdNaNdbNdbNfNeNdNbNdfNbcdNdeaN::P | `pass/calls/runtime_constructor_child_inherited_captures_exit` |
| 1 | pass/run rejected | machine `_` state `_` expression type application uses undeclared lifetime argument `_`Note`; declare it in the owner's  | `pass/calls/runtime_projected_type_operand_scopes_exit` |
| 1 | pass/run rejected | machine call cycle: `_`, `_` -- an internal call occurrence lacks a classified tail edge; runtime calls must be tail; a  | `pass/calls/statement_call_recursive_overload_compile` |
| 1 | pass/run rejected | call to `_` may suspend while `_` remains live, but its effective policy is `_`; consume the value before the call or us | `pass/concurrency/runtime_returned_carrier_suspension_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from direct at statement N: result in Token::Ready; the returned value does not e | `pass/contracts/adapter_result_membership_forwarded` |
| 1 | pass/run rejected | cannot prove requires contract for call compare from walk: bytes[offset].number_of_pages <= N && bytes[offset].physical_ | `pass/contracts/borrowed_descriptor_guarded_end_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call kept from roundtrip: __transition_subject#N.neighbor == N (needs fact `_` here;  | `pass/contracts/match_payload_contract` |
| 1 | pass/run rejected | cannot prove asm `_` fact at block entry in main state `_` (statement N): returned == N && preserved == N && completed = | `pass/contracts/runtime_mutable_owned_call_poststate_exit` |
| 1 | pass/run rejected | statement N reads `_` without an exact retained backing for its loan | `pass/contracts/runtime_mutable_owned_collection_poststate_exit` |
| 1 | pass/run rejected | cannot plan contextual call cleanup against the executable hook roster: machine `_`, state `_`, statement N, call N: own | `pass/control_flow/field_view_transfer_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "dispatch::cycle", reason: "retained par | `pass/control_flow/runtime_cyclic_owned_result_local_arrival_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "rotate_envelopes", reason: "retained pa | `pass/control_flow/runtime_cyclic_reference_and_claim_rotation_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call render_row from prepare_line::entry: parameter line requires [uN; | `pass/control_flow/runtime_local_record_array_byte_render_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call selected from inspect_view: choices[__hoist_N].value < N (needs fact `_` here; e | `pass/control_flow/runtime_projected_case_payload_snapshots_exit` |
| 1 | pass/run rejected | terminating machine drain reaches recursive component `_`, which has no `_` ranking -- spell one on that component | `pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit` |
| 1 | pass/run rejected | terminating machine spin reaches recursive component `_`, which has no `_` ranking -- spell one on that component | `pass/control_flow/runtime_ranked_state_cycle_owned_record_exit` |
| 1 | pass/run rejected | state `_` receives `_` while local borrow `_` is still active (borrowed at statement N; its last use is at statement N a | `pass/control_flow/runtime_sum_tuple_matrix_exhaustive_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call read_selected from Main::main: valid_values(main.rows.values[..]) == true (needs | `pass/dependent/runtime_symbolic_slice_cell_theorem_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call replace from Main::main: valid_values(main.rows.values[..]) == true && ascending | `pass/dependent/runtime_symbolic_slice_predicate_store_exit` |
| 1 | pass/run rejected | cannot complete return from machine `_` state `_`: this write left the `_` facts of `_` (data `_`) unproved; restore the | `pass/dependent/valuation_order_cycle` |
| 1 | pass/run rejected | cannot prove requires contract for call relay from choose: picked in package:NcNfNdNbNfNbNaNaNcNacNaNecNccNeNddNfdNa:pro | `pass/domains/domain_axis_cast` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for return from issue_handle at statement N: Handle requires Handle::Open | `pass/domains/qualified_record_case_predicate` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call clear from capture::entry: parameter number requires Ticket::Issu | `pass/domains/routed_record_capture_before_sibling_mutation` |
| 1 | pass/run rejected | cannot prove scalar result domain for return from fresh at statement N: iN::Nonnegative | `pass/domains/runtime_scalar_domain_mixed_return_origins_exit` |
| 1 | pass/run rejected | returned view lost its completed invocation capture | `pass/domains/runtime_selected_nested_field_domain_exit` |
| 1 | pass/run rejected | statement N mutates `_` while local borrow `_` is still active (borrowed at statement N) | `pass/drops/runtime_owner_scoped_generic_cleanup_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidCleanupPartition { reason: "cleanup partition actions are interleav | `pass/drops/runtime_selected_partial_owner_cleanup_exit` |
| 1 | pass/run rejected | machine `_` omits `_` from its published synchronous-invocation ceiling, but its body may invoke that binding before ret | `pass/effects/gui_semantic_services` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/expressions/match_operand_anonymous_operat | `pass/expressions/match_operand_anonymous_operation_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "exercise", reason: "computed owned stru | `pass/expressions/runtime_match_copy_descriptor_call_exit` |
| 1 | incomplete | `_` calls `_`, which calls `_`; `_` has no executable body: this compiler does not lower a statement of it yet (structur | `pass/expressions/runtime_numeric_cast_exit` |
| 1 | pass/run rejected | cannot prove automatic cleanup requires at selected edge from select state entry after statement N: original.second does | `pass/expressions/runtime_owned_match_multiple_projected_claims_exit` |
| 1 | pass/run rejected | assumed compiler-evaluation control reached another refusal: machine `_` is not build-time admissible: the checked Trap  | `pass/facts/assumed_binding_result_bound_through_local_exit` |
| 1 | pass/run rejected | machine `_` state `_` statement N: cannot prove selected case for payload access `_`: captured source `_` has no live ca | `pass/facts/case_payload_exclusive_loan_through_other_machine_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "width", reason: "computed owned structu | `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit` |
| 1 | pass/run rejected | exact arithmetic in machine `_` state `_` return value may overflow `_`: the operands are not provably in range (decisio | `pass/facts/case_where_equality_data_predicate_through_field_exit` |
| 1 | pass/run rejected | exact arithmetic in machine `_` state `_` assignment may overflow `_`: the operands are not provably in range (decision  | `pass/facts/case_where_equality_through_call_result_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call choose from select: visits == N (needs fact `_` here; establish it before the ca | `pass/facts/exclusive_match_arm_through_call_result_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call choose from write: visits == N (needs fact `_` here; establish it before the cal | `pass/facts/exclusive_match_arm_through_other_state_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call choose from XNApply::apply: visits == N (needs fact `_` here; establish it befor | `pass/facts/exclusive_match_arm_through_target_body_exit` |
| 1 | pass/run rejected | normal call `_` (Handle { arena_index: N, generation: N }) in machine `_` state `_` statement N: completed call result h | `pass/facts/fixed_layout_byte_roundtrip_through_target_body_exit` |
| 1 | pass/run rejected | data `_` cannot bind implementation `_`: conformance application `_` supplies undeclared lifetime argument `'selected';  | `pass/facts/implementation_region_forwarding_through_target_body_exit` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/indexed_crash_guard_through_target_b | `pass/facts/indexed_crash_guard_through_target_body_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call compute from write: visits == N (needs fact `_` here; establish it before the ca | `pass/facts/match_computed_root_through_other_state_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call read_field from pick: match choose() { true => chosen, false => spare }.kept ==  | `pass/facts/match_join_place_read_through_other_state_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call read_field from XNApply::apply: match choose() { true => chosen, false => spare  | `pass/facts/match_join_place_read_through_target_body_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call read_origin from XNApply::apply: joined == N (needs fact `_` here; establish it  | `pass/facts/match_primitive_local_origin_through_target_body_exit` |
| 1 | pass/run rejected | cannot prove subslice range end bound `_` is within unknown slice length of `_` in trial::entry | `pass/facts/nested_result_extent_through_field_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call write from main::entry: parameter first requires Storage::Issued | `pass/facts/non_owning_reference_erasure_through_match_join_exit` |
| 1 | pass/run rejected | native artifact target lowering failed: StructuralStateBindingMismatch { machine: MachineId(N), reason: "binding=Abstrac | `pass/facts/readonly_byte_pool_relocation_bound_through_match_join_exit` |
| 1 | incomplete | `_` calls `_`, which calls `_`, which calls `_`; `_` has no executable body: this compiler does not lower a statement of | `pass/facts/recoverable_replacement_window_through_match_join_exit` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (statement sequence: assignment: structur | `pass/facts/recoverable_replacement_window_through_target_body_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call check from main::entry: parameter pair requires Pair::Label | `pass/facts/resident_backing_plain_ram_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call agree from main: saved == holder.items[N] && saved == N (needs fact `_` here; es | `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call agree from read_kept: saved == holder.items[N] && saved == N (needs fact `_` her | `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call agree from HostedObservationBody::observe: saved == holder.items[N] && saved ==  | `pass/facts/saved_observation_snapshot_indexed_handback_target_body_exit` |
| 1 | pass/run rejected | cannot prove scalar result domain for return from finish at statement N: iN::Nonnegative | `pass/facts/scalar_domain_call_guarantee_through_other_state_exit` |
| 1 | pass/run rejected | applied callable has an unrepresented static type operand | `pass/facts/selected_value_application_through_field_exit` |
| 1 | pass/run rejected | `_` does not satisfy `_`: helper `_` requires `_`, but the implementation promises `_` | `pass/facts/static_callback_reach_requirement_telescope` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/cal | `pass/facts/target_family_callback_guarantee/call_result` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/con | `pass/facts/target_family_callback_guarantee/const_name` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/fie | `pass/facts/target_family_callback_guarantee/field` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/loc | `pass/facts/target_family_callback_guarantee/local` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/mat | `pass/facts/target_family_callback_guarantee/match_join` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/par | `pass/facts/target_family_callback_guarantee/parameter` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_callback_guarantee/tar | `pass/facts/target_family_callback_guarantee/target_body` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/call_ | `pass/facts/target_family_contract_operand/call_result` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/const | `pass/facts/target_family_contract_operand/const_name` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/field | `pass/facts/target_family_contract_operand/field` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/local | `pass/facts/target_family_contract_operand/local` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/match | `pass/facts/target_family_contract_operand/match_join` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/param | `pass/facts/target_family_contract_operand/parameter` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/facts/target_family_contract_operand/targe | `pass/facts/target_family_contract_operand/target_body` |
| 1 | pass/run rejected | cannot prove requires contract for call finite_value from main: FN.is_finite((N as fN)) (needs fact `_` here; establish  | `pass/float/closed_integer_conversion_predicate_proof_exit` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (outer calls: ordered statement call); in | `pass/float/computed_comparison_literal_landing_exit` |
| 1 | pass/run rejected | store in construction of `_` field `_` implicitly converts `_` to `_`; a typed value keeps its type and arithmetic polic | `pass/float/constructor_requirement_result_policy_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call demand from forward: value == value (needs fact `_` here; establish it before th | `pass/float/nested_caller_snapshot_before_sibling_write_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call nonnegativeN from main: wide >= N.N (needs fact `_` here; establish it before th | `pass/float/primitive_storage_predicate_keeps_ieee_meaning` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from rounded at statement N: result == N.N | `pass/float/runtime_format_conversion_result_bounds_exit` |
| 1 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a state of it yet (state graph: structural retur | `pass/float/runtime_refined_conversion_target_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from through_state at statement N: result == N.N | `pass/float/runtime_result_predicate_custody_exit` |
| 1 | pass/run rejected | machine `_` proves no guarantee for the selected comparison law `_` | `pass/float/runtime_resultless_law_equality_custody_exit` |
| 1 | pass/run rejected | authored Operator declaration selection occurrence N (`_` in tests/omega/pass/generics/declared_range_endpoint_inference | `pass/generics/declared_range_endpoint_inference` |
| 1 | pass/run rejected | machine `_` state `_` statement N: operand has no complete completed call-slot relation | `pass/generics/generic_result_parameter_receipt` |
| 1 | pass/run rejected | generated generic application does not match its exact retained instance origin | `pass/generics/inferred_record_equation_repeated_equal_exit` |
| 1 | pass/run rejected | index compatibility condition `_` is not established: actual `_` and expected `_` are distinct normalized instances; clo | `pass/generics/open_computed_quantity_result` |
| 1 | pass/run rejected | conformance `_` parameter `_` requires a const argument, but `_` has another static category | `pass/generics/record_const_named_literal_equivalence` |
| 1 | pass/run rejected | realization contract lost its complete capture identity: selected=Selected, call_owner=main, region_supplier=main, lifet | `pass/generics/runtime_captured_map_borrowed_type` |
| 1 | pass/run rejected | machine argument `_` for `_` does not refine `_`: parameter N expects `_`, got `_` | `pass/generics/runtime_captured_map_const_machine` |
| 1 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "exact | `pass/generics/runtime_const_data_machine_call_exit` |
| 1 | pass/run rejected | const argument expression in generic data `_` is invalid: const operator application requires declaration selection | `pass/generics/runtime_const_data_symbolic_expression_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call main from entry_start::entry: parameter main.carrier.value requir | `pass/generics/runtime_generic_carrier_binder_equation_exit` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call main from entry_start::entry: parameter main.box.value requires T | `pass/generics/runtime_generic_carrier_equation_context_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call consume_inferred from main: Count == N (needs fact `_` here; establish it before | `pass/generics/runtime_inferred_index_statement_call_exit` |
| 1 | pass/run rejected | normal call `_` (Handle { arena_index: N, generation: N }) in machine `_` state `_` statement N: completed result refere | `pass/generics/runtime_inherited_composite_requirement_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "finish", reason: "selected edge lost it | `pass/host/process_exit_selected_frontier` |
| 1 | pass/run rejected | a value call to generic machine `_` cannot derive a complete type/const/machine/conformance specialization tuple from it | `pass/host/runtime_semantic_gui_reach_exit` |
| 1 | pass/run rejected | inspection attempt production: [Diagnostic { severity: Error, message: "`_` calls `_`; `_` has no executable body: this  | `pass/layouts/extent_view_external_content` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(Unsupported("closed reach call lost its exact emitted invocation")) | `pass/layouts/placed_primitive_callback` |
| 1 | pass/run rejected | native artifact fragment object publication failed: placed backing geometry at MachineId(N)/OperationId(N): actual backi | `pass/layouts/placed_primitive_mixed_backing_rejected` |
| 1 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower its declaration of it yet (statement sequence: c | `pass/layouts/placed_primitive_returned_backing` |
| 1 | pass/run rejected | bodyless boundary symbol `_` has no executable realization; use it only in contracts, or satisfy a boundary requirement  | `pass/layouts/resident_lifecycle_plan_discovery` |
| 1 | pass/run rejected | IntegerAt store in machine `_` state `_` assignment requires a value proven to fit N-bit signed storage | `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit` |
| 1 | incomplete | `_` calls `_`; `_` has no executable body: this compiler does not lower a call of it yet (statement sequence: call: miss | `pass/locals/runtime_local_qualified_view_member_read_exit` |
| 1 | pass/run rejected | cannot establish call-result qualification `_` in `_`, state `_` (statement N, call N): the qualified domain has no reta | `pass/memory/interrupt_table_canary` |
| 1 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "anony | `pass/modules/match_constant_indices` |
| 1 | pass/run rejected | machine `_` state `_` terminal expression expects `_`, got `_` | `pass/modules/module_array_constant_indices` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a transition of it yet (call operation: scalar arguments); in s | `pass/operators/runtime_constructed_operand_result_exit` |
| 1 | pass/run rejected | normal reference-slot invocation has more than one completion context | `pass/operators/runtime_declared_open_range_evaluates_collection_once_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "choose$specialized$cNbNaNaNeN$N$N", rea | `pass/operators/runtime_declared_open_range_preserves_operand_snapshot_exit` |
| 1 | pass/run rejected | selected optimizations `_`, `_`, `_`, `_`, `_`, `_`, `_`, `_`, `_`, `_` failed in the complete verified optimizer pipeli | `pass/optimizer/pre_allocation_joint_frontier_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call drop from main: retained in Capacity::Held in state `_` | `pass/ownership/registration_owned_claim_retains_provenance_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Main::main", reason: "completed linear  | `pass/ownership/runtime_declared_linear_local_assignment_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(ConflictingStructuralValueHome { root: Expression(Handle { arena_index: N, | `pass/ownership/runtime_match_nested_observation_cleanup_exit` |
| 1 | pass/run rejected | cannot plan contextual call cleanup against the executable hook roster: machine `_`, state `_`, statement N, call N: com | `pass/ownership/runtime_match_observation_cleanup_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "binding_subject", reason: "selection su | `pass/ownership/runtime_match_projected_subject_cleanup_exit` |
| 1 | pass/run rejected | machine `_` static type argument uses undeclared lifetime argument `_`Owner`; declare it in the owner's generic paramete | `pass/packages/runtime_attached_owner_crash_guard_scope_exit` |
| 1 | pass/run rejected | provider completion for `_` disagrees with its exact retained requirement contract | `pass/progress/provider_receiver_progress_installation` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from retain at statement N: result <= used | `pass/proofs/borrowed_descriptor_bound_local_result_exit` |
| 1 | pass/run rejected | construction value is not proven in domain `_` in main state `_` at statement N; a construction value must establish the | `pass/proofs/declaration_predicate_calls_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from filter at statement N: partition.count <= partition.records.len | `pass/proofs/descriptor_bound_after_element_call` |
| 1 | pass/run rejected | cannot prove requires contract for call demand from filter: partition.count <= partition.records.len (needs fact `_` her | `pass/proofs/descriptor_bound_call_after_element_call` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "work::received", reason: "selected edge | `pass/proofs/exhaustive_nonreturn_contract` |
| 1 | pass/run rejected | authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity: Error, message: "propo | `pass/proofs/family_carrier_relation_laws` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from retain at statement N: result <= count | `pass/proofs/loop_postcondition_after_element_store` |
| 1 | pass/run rejected | checked machine `_` cannot establish proposition ensure `_` in state `_`; require it, cite a checked/accepted proof that | `pass/proofs/proposition_relation_hierarchy_compile` |
| 1 | pass/run rejected | cannot prove requires contract for call capacity from geometry: pages >= N && pages <= N && __transition_subject#N.indic | `pass/proofs/reviewed_mathematical_data_contract_exit` |
| 1 | pass/run rejected | terminating machine descend reaches recursive component `_`, which has no `_` ranking -- spell one on that component | `pass/proofs/runtime_sequential_ranked_components_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call consume from Main::main: main.bytes in [uN]::UtfN (invalidated by prior mutation | `pass/proofs/scalar_state_return_keeps_byte_class` |
| 1 | pass/run rejected | tests/omega/pass/proofs/selected_specification_requires_state/main.omg:N:N: expected transition subject expression | `pass/proofs/selected_specification_requires_state` |
| 1 | pass/run rejected | tests/omega/pass/providers/authored_same_stack_demand/main.omg:N:N: expected one of `_`, `_`, `_`, `_`, `_`, `_`, `_`, ` | `pass/providers/authored_same_stack_demand` |
| 1 | pass/run rejected | tests/omega/pass/providers/authored_same_stack_import/main.omg:N:N: expected one of `_`, `_`, `_`, `_`, `_`, `_`, `_`, ` | `pass/providers/authored_same_stack_import` |
| 1 | pass/run rejected | anonymous source tests/omega/pass/providers/build_product_adapter_frontiers_exit/support/values.omg belongs to a differe | `pass/providers/build_product_adapter_frontiers_exit` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Policy::identity", reason: "attached Un | `pass/providers/checked_operator_declaration_without_application` |
| 1 | pass/run rejected | machine `_` state `_` cannot prove runtime index subject equality at statement N; an indexed value retains its captured  | `pass/providers/generic_provider_structural_result_composition_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from TokenProviderTokenFlowImplementation::forward at statement N: result in Carr | `pass/providers/linear_structural_provider_return_exit` |
| 1 | pass/run rejected | static type argument `_` selects generic data without its complete type/lifetime arguments | `pass/providers/runtime_adapter_dispatch_exit` |
| 1 | pass/run rejected | cannot transfer a non-copy value out of borrowed storage without replacing its owner in `_`, state `_` (statement N) | `pass/recast/returned_same_type_view_capture_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call consume from inspect: offset <= N && bytes[offset].pages <= N && bytes[offset].b | `pass/recast/runtime_captured_guard_conjunction_exit` |
| 1 | pass/run rejected | cannot prove ensures contract for exit from middle at statement N: result.len == end - start | `pass/slices/runtime_forwarded_range_extent_snapshot_exit` |
| 1 | pass/run rejected | checked direct-reborrow suspension requires exactly one parent entry constraint | `pass/slices/runtime_nested_view_descriptor_reseat_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call inspect from probe: ascii_only(bytes[N..N]) && bytes[N..N].len >= N (needs fact  | `pass/slices/saved_byte_predicate_alias_disjoint_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call inspect_after from probe: ascii_only(bytes[N..N]) && bytes[N..N].len >= N (needs | `pass/slices/saved_byte_predicate_operand_disjoint_exit` |
| 1 | pass/run rejected | terminal call `_` does not select a declared machine entry or callable requirement | `pass/slices/termination_slice_length_compile` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "churn::next", reason: "owned edge trans | `pass/structural/runtime_scalar_record_reclamation_exit` |
| 1 | pass/run rejected | inspection constant omitted its expected debug companion | `pass/terminal_psi/debug_identity_complete_module` |
| 1 | pass/run rejected | shift count in machine `_` state `_` local `_` is not provably below the operand width for `_`: exact shifts prove `_` ( | `pass/terminal_psi/integer_control_contract` |
| 1 | pass/run rejected | machine call cycle: `_` -- the arriving rank reference is missing its exact public subject; runtime calls must be tail;  | `pass/termination/measure_borrowed_projection_range` |
| 1 | pass/run rejected | cannot prove requires contract for call consume from arrive: held.value in Token::Fair in state `_` | `pass/termination/progress_selector_owned_arrival_owned_input_replacement_exit` |
| 1 | pass/run rejected | terminating machine walk reaches recursive component `_`, which has no `_` ranking -- spell one on that component | `pass/termination/runtime_named_state_remainder_endpoint_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call write from write: bytes in [uN; N]::Ascii (invalidated by prior mutation of byte | `pass/text/bounded_ascii_write` |
| 1 | pass/run rejected | cannot prove default-domain field requirement for call step from Main::main::main: parameter output requires [uN; N]::Ut | `pass/text/qualified_byte_owner_state_loan_cycle_exit` |
| 1 | pass/run rejected | native artifact identity physical pipeline failed: common physical staging failed: Selection(Selection(SourceCustodyMism | `pass/text/runtime_mixed_byte_field_equality_exit` |
| 1 | pass/run rejected | state `_` receives `_` without an exact retained backing for its loan: the stored descriptor has no current value source | `pass/text/utf8_boundary_established` |
| 1 | pass/run rejected | machine `_` state `_` does not satisfy trait `_` machine `_`: conformance borrow contract: callable input does not prese | `pass/traits/applied_conformance_captured_fresh_regions_exit` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a state of it yet (statement sequence: completion stored refere | `pass/traits/prepared_callable_nested_borrow_paths_exit` |
| 1 | pass/run rejected | call `_` has an invalid argument correspondence: selected call has no exact applied public signature | `pass/traits/refined_borrowed_implementation_exit` |
| 1 | pass/run rejected | cannot prove requires contract for call code from Primary::code: Width == N // Width == N // Width == N // Width == N (n | `pass/traits/runtime_dyn_family_nested_result` |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Main::main", reason: "structural crash  | `pass/traits/runtime_qualified_byte_crash_equality_exit` |
| 1 | incomplete | `_` has no executable body: this compiler does not lower a statement of it yet (call operation: structural arguments: re | `run/calls/anonymous_shared_borrow_operands` |
| 1 | pass/run rejected | cannot prove requires contract for call join from argument: Amount in Amount::Positive in state `_` | `run/domains/completed_membership_after_earlier_guarantee` |
| 1 | pass/run rejected | construction value is not proven in domain `_` in issue state `_` at statement N; a construction value must establish th | `run/domains/issued_record_integer_bounds` |
| 1 | pass/run rejected | cannot prove requires contract for call use_small from relay: x in uN::Small in state `_` | `run/domains/predicate_interval_premises` |

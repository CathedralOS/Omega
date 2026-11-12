# Soundness mutations shard 5 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
Host: cursor
Shard selection: fixture index `i % 8 == 5` over pass/ and run/ (1130 fixtures).

## Counts
- fixtures in shard: 1130
- mutants checked (deduped by fixture+kind): 1067
- both checkers reject (discarded): 888
- confirmed-invalid holes/disagreements: 177
- discarded-valid after remove_mut re-check: 20
- discarded-no-rule: 0

## Commands
```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg
timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg
python3 .agents/four-jobs-2026-11-12/soundness/run_shard5.py
python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard5.py
```

## Confirmed holes and disagreements

### `drop_requires` — `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`
- Original: `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_arithmetic_bitwise_singleton_signed_and_unsigned_exit_main_omg_0b7021d138__drop_requires`
- Current checker: **accepted**; route: **rejected**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/atomics/placed_atomic_load/main.omg`
- Original: `tests/omega/pass/atomics/placed_atomic_load/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_atomics_placed_atomic_load_main_omg_5ebfd24054__remove_mut`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg`
- Original: `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_borrow_named_operator_disjoint_argument_preserves_bound_main_omg_a92dc43876__remove_mut`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrow/write_only_whole_scalar_replace/main.omg`
- Original: `tests/omega/pass/borrow/write_only_whole_scalar_replace/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrow_write_only_whole_scalar_replace_main_omg_111bcb0f21__remove_mut`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrows/copied_image_descriptor_unrelated_write/main.omg`
- Original: `tests/omega/pass/borrows/copied_image_descriptor_unrelated_write/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_copied_image_descriptor_unrelated_write_main_omg_11ec3a7a8d__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/borrows/runtime_bounded_and_shared_field_loans_exit/main.omg`
- Original: `tests/omega/pass/borrows/runtime_bounded_and_shared_field_loans_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_runtime_bounded_and_shared_field_loans_exit_main_omg_bb09d26fe4__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/borrows/runtime_held_descriptor_replacement_exit/main.omg`
- Original: `tests/omega/pass/borrows/runtime_held_descriptor_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_runtime_held_descriptor_replacement_exit_main_omg_425d7220eb__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrows/runtime_qualified_returned_descriptor_exit/main.omg`
- Original: `tests/omega/pass/borrows/runtime_qualified_returned_descriptor_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_borrows_runtime_qualified_returned_descriptor_exit_main_omg_3895e9803c__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrows/runtime_shared_result_attenuates_exclusive_origin_exit/main.omg`
- Original: `tests/omega/pass/borrows/runtime_shared_result_attenuates_exclusive_origin_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ega_pass_borrows_runtime_shared_result_attenuates_exclusive_origin_exit_main_omg_4da33456c7__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/borrows/suspension_projected_call_view_whole_input_restore_exit/main.omg`
- Original: `tests/omega/pass/borrows/suspension_projected_call_view_whole_input_restore_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_borrows_suspension_projected_call_view_whole_input_restore_exit_main_omg_8da38abbba__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/calls/runtime_guarded_reference_return_crash/main.omg`
- Original: `tests/omega/pass/calls/runtime_guarded_reference_return_crash/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_calls_runtime_guarded_reference_return_crash_main_omg_ffce0be1fb__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/calls/runtime_nested_completed_operands_then_shared_borrow_exit/main.omg`
- Original: `tests/omega/pass/calls/runtime_nested_completed_operands_then_shared_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_calls_runtime_nested_completed_operands_then_shared_borrow_exit_main_omg_3d53c2d766__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/calls/runtime_stored_reference_computation_argument_exit/main.omg`
- Original: `tests/omega/pass/calls/runtime_stored_reference_computation_argument_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_calls_runtime_stored_reference_computation_argument_exit_main_omg_77542dbf7c__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/cases/runtime_case_component_borrow_identity_exit/main.omg`
- Original: `tests/omega/pass/cases/runtime_case_component_borrow_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_cases_runtime_case_component_borrow_identity_exit_main_omg_4bd7fb9040__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/collections/empty_record_array_borrow_exit/main.omg`
- Original: `tests/omega/pass/collections/empty_record_array_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_collections_empty_record_array_borrow_exit_main_omg_d863f30077__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/collections/nested_empty_record_array_borrow_exit/main.omg`
- Original: `tests/omega/pass/collections/nested_empty_record_array_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_collections_nested_empty_record_array_borrow_exit_main_omg_f702ee02af__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg`
- Original: `tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_collections_runtime_finite_element_view_call_write_exit_main_omg_9b8d41b651__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg`
- Original: `tests/omega/pass/collections/runtime_refined_element_view_call_write_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_collections_runtime_refined_element_view_call_write_exit_main_omg_31dae98b08__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/collections/runtime_stored_reference_array_owned_record_contract_exit/main.omg`
- Original: `tests/omega/pass/collections/runtime_stored_reference_array_owned_record_contract_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_collections_runtime_stored_reference_array_owned_record_contract_exit_main_omg_1b0f3d72e0__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/concurrency/incoming_aggregate_loan_after_suspend/main.omg`
- Original: `tests/omega/pass/concurrency/incoming_aggregate_loan_after_suspend/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_concurrency_incoming_aggregate_loan_after_suspend_main_omg_6cb097ef09__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/constants/owned_record_argument_snapshot_exit/main.omg`
- Original: `tests/omega/pass/constants/owned_record_argument_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_constants_owned_record_argument_snapshot_exit_main_omg_7b607d44d4__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/constraints/guarded_parameter_result_contract/main.omg`
- Original: `tests/omega/pass/constraints/guarded_parameter_result_contract/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_constraints_guarded_parameter_result_contract_main_omg_e3deb34bbd__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/contracts/assignment_destination_before_rhs_match_guard_exit/main.omg`
- Original: `tests/omega/pass/contracts/assignment_destination_before_rhs_match_guard_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_contracts_assignment_destination_before_rhs_match_guard_exit_main_omg_099bef0f70__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/dependent/case_where_zero_decided_first_case/main.omg`
- Original: `tests/omega/pass/dependent/case_where_zero_decided_first_case/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_dependent_case_where_zero_decided_first_case_main_omg_a338fb231c__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/dependent/declaration_predicate_requires_proved/main.omg`
- Original: `tests/omega/pass/dependent/declaration_predicate_requires_proved/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_dependent_declaration_predicate_requires_proved_main_omg_3cd7c7182a__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/domains/erasing_cast_keeps_field_facts_exit/main.omg`
- Original: `tests/omega/pass/domains/erasing_cast_keeps_field_facts_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_domains_erasing_cast_keeps_field_facts_exit_main_omg_285b7d81f0__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/domains/tag_alias_retains_receiving_predicate_exit/main.omg`
- Original: `tests/omega/pass/domains/tag_alias_retains_receiving_predicate_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_domains_tag_alias_retains_receiving_predicate_exit_main_omg_a7529034a7__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/drops/borrowed_projection_captures_before_sibling_write_exit/main.omg`
- Original: `tests/omega/pass/drops/borrowed_projection_captures_before_sibling_write_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_drops_borrowed_projection_captures_before_sibling_write_exit_main_omg_1e7312882e__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/drops/runtime_borrowed_selected_cleanup_postcondition_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_borrowed_selected_cleanup_postcondition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_drops_runtime_borrowed_selected_cleanup_postcondition_exit_main_omg_7a8aafa287__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/drops/runtime_empty_array_owner_cleanup_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_empty_array_owner_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_empty_array_owner_cleanup_exit_main_omg_f80b46e93e__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/drops/runtime_local_nominal_cleanup_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_local_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_local_nominal_cleanup_exit_main_omg_c88f3fd8c7__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/drops/runtime_nested_cleanup_postcondition_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_nested_cleanup_postcondition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_nested_cleanup_postcondition_exit_main_omg_8d8235d1f1__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/drops/runtime_nominal_state_loan_cleanup_order_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_nominal_state_loan_cleanup_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_nominal_state_loan_cleanup_order_exit_main_omg_17eabe3a02__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_record_construction_cleanup_order_exit_main_omg_c60036eaf8__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/drops/runtime_shared_nominal_state_loan_exit/main.omg`
- Original: `tests/omega/pass/drops/runtime_shared_nominal_state_loan_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_drops_runtime_shared_nominal_state_loan_exit_main_omg_ed6f0c6caf__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_applied_callback_forwarded_type_argument_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_applied_callback_forwarded_type_argument_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_abstract_signature_call_applied_callback_forwarded_type_argument_exit_main_omg_01212f4832__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_applied_callback_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_applied_callback_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_pass_facts_abstract_signature_call_applied_callback_through_field_exit_main_omg_04650911b1__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_applied_callback_type_bound_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_applied_callback_type_bound_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ega_pass_facts_abstract_signature_call_applied_callback_type_bound_exit_main_omg_efc922a679__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_constant_initializer_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_constant_initializer_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_abstract_signature_call_constant_initializer_through_const_name_exit_main_omg_0c6ffff453__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_constant_initializer_unexecuted_body_dependency_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_constant_initializer_unexecuted_body_dependency_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/act_signature_call_constant_initializer_unexecuted_body_dependency_exit_main_omg_d253fa48ea__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/abstract_signature_call_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/abstract_signature_call_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_abstract_signature_call_through_other_state_exit_main_omg_46e4bad1ec__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/additional_input_domain_mutation_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_pass_facts_additional_input_domain_mutation_through_other_machine_exit_main_omg_9626d91981__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/affine_discard_root_cleanup_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/affine_discard_root_cleanup_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_affine_discard_root_cleanup_through_local_exit_main_omg_650f9b5a31__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/applied_requirement_indices_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/applied_requirement_indices_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_facts_applied_requirement_indices_through_parameter_exit_main_omg_da1d8a869e__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/as_slice_views_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/as_slice_views_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_as_slice_views_through_parameter_exit_main_omg_fcbaa3b60d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/facts/assumed_binding_input_authority_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/assumed_binding_input_authority_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ega_pass_facts_assumed_binding_input_authority_through_other_state_exit_main_omg_14f54a3555__widen_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/facts/assumed_binding_result_bound_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/assumed_binding_result_bound_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/mega_pass_facts_assumed_binding_result_bound_through_other_machine_exit_main_omg_117900933f__widen_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/body_bearing_comparison_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/body_bearing_comparison_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_body_bearing_comparison_through_field_exit_main_omg_85e101cc28__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/borrowed_case_result_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/borrowed_case_result_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_borrowed_case_result_through_const_name_exit_main_omg_b0003fe672__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/borrowed_projection_cleanup_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/borrowed_projection_cleanup_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_facts_borrowed_projection_cleanup_through_parameter_exit_main_omg_04aad2e14d__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/boundary_zero_origin_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/boundary_zero_origin_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_boundary_zero_origin_through_local_exit_main_omg_fece0c3849__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/call_result_bound_discharge_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/call_result_bound_discharge_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_call_result_bound_discharge_through_field_exit_main_omg_843b7a6675__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/captured_storage_bounds_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/captured_storage_bounds_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_captured_storage_bounds_through_call_result_exit_main_omg_1d997f2a22__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/captured_storage_bounds_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/captured_storage_bounds_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_captured_storage_bounds_through_target_body_exit_main_omg_ad9fc9f0d9__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/case_payload_exclusive_loan_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/case_payload_exclusive_loan_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_case_payload_exclusive_loan_through_match_join_exit_main_omg_dd0c98c07e__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/dependency_type_qualified_call_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/dependency_type_qualified_call_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_omega_pass_facts_dependency_type_qualified_call_through_parameter_exit_main_omg_d264d79063__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_facts_descriptor_payload_exclusive_loan_through_match_join_exit_main_omg_44061c55ba__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_facts_descriptor_range_exclusive_loan_through_local_exit_main_omg_bd742776c2__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/domain_selected_cleanup_erased_interface_exit/main.omg`
- Original: `tests/omega/pass/facts/domain_selected_cleanup_erased_interface_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_domain_selected_cleanup_erased_interface_exit_main_omg_596cb01cd0__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/domain_selected_cleanup_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/domain_selected_cleanup_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_domain_selected_cleanup_through_local_exit_main_omg_a0b030bb40__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/double_indexed_rmw_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_double_indexed_rmw_through_field_exit_main_omg_04e24d9734__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_pass_facts_dyn_dispatch_selector_payload_bound_through_const_name_exit_main_omg_f7e9887a41__widen_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/erased_reference_contract_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/erased_reference_contract_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_erased_reference_contract_through_local_exit_main_omg_c6478c7511__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/exclusive_match_arm_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/exclusive_match_arm_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_exclusive_match_arm_through_call_result_exit_main_omg_b98c3080ba__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_exclusive_match_arm_through_target_body_exit_main_omg_7fac4dc386__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/exclusive_observation_exposure_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/exclusive_observation_exposure_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_omega_pass_facts_exclusive_observation_exposure_through_parameter_exit_main_omg_9a5088d94c__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/field_where_index_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_field_where_index_through_other_state_exit_main_omg_d9aa784ee5__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/float_range_delivery_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/float_range_delivery_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_float_range_delivery_through_parameter_exit_main_omg_7272b1caeb__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/foreign_byte_array_element_backing_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ass_facts_foreign_byte_array_element_backing_through_other_machine_exit_main_omg_89cb9ca4e6__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/foreign_nested_record_backing_local/main.omg`
- Original: `tests/omega/pass/facts/foreign_nested_record_backing_local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_foreign_nested_record_backing_local_main_omg_3e28310877__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/foreign_reference_result_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/foreign_reference_result_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_foreign_reference_result_through_call_result_exit_main_omg_cc35259838__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/foreign_reference_result_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/foreign_reference_result_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_foreign_reference_result_through_target_body_exit_main_omg_d5f834f3f1__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/foreign_retained_mutable_borrow_other_state/main.omg`
- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_foreign_retained_mutable_borrow_other_state_main_omg_7176853b17__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/foreign_retained_shared_borrow_field/main.omg`
- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_foreign_retained_shared_borrow_field_main_omg_876eefd6be__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/free_entry_local_state_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/free_entry_local_state_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_free_entry_local_state_through_call_result_exit_main_omg_e3a8da1c6d__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/free_entry_local_state_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/free_entry_local_state_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_free_entry_local_state_through_target_body_exit_main_omg_c28ba5c4fc__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/generic_comparison_body_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/generic_comparison_body_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_generic_comparison_body_through_call_result_exit_main_omg_5c18ee000d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/generic_comparison_body_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/generic_comparison_body_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_generic_comparison_body_through_target_body_exit_main_omg_078cb3ff1f__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/generic_requirement_statement_call_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/generic_requirement_statement_call_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_pass_facts_generic_requirement_statement_call_through_other_state_exit_main_omg_d0440da956__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/ieee_float_32bit_ordered_bound_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_facts_ieee_float_32bit_ordered_bound_through_const_name_exit_main_omg_8694f94a09__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/ieee_float_ordered_comparison_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/ieee_float_ordered_comparison_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_facts_ieee_float_ordered_comparison_through_call_result_exit_main_omg_f5eaf21b9f__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/ieee_float_ordered_comparison_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/ieee_float_ordered_comparison_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_facts_ieee_float_ordered_comparison_through_target_body_exit_main_omg_5d2908df68__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/induction_call_guarantee_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/induction_call_guarantee_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_induction_call_guarantee_through_other_machine_exit_main_omg_dbd6ecdbba__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/linear_owner_borrowed_projection_parameter/main.omg`
- Original: `tests/omega/pass/facts/linear_owner_borrowed_projection_parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_linear_owner_borrowed_projection_parameter_main_omg_1edb26bdc1__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_facts_live_across_two_calls_preservation_bound_through_match_join_exit_main_omg_d7204a10b6__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/match_join_place_read_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/match_join_place_read_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_match_join_place_read_through_other_state_exit_main_omg_4f4129afba__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/match_loan_last_use_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/match_loan_last_use_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_match_loan_last_use_through_other_machine_exit_main_omg_559de1ae93__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/match_primitive_local_origin_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/match_primitive_local_origin_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_facts_match_primitive_local_origin_through_match_join_exit_main_omg_c08f22474b__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/match_reference_referent_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/match_reference_referent_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_match_reference_referent_through_field_exit_main_omg_9820db8eec__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/match_runtime_view_extent_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/match_runtime_view_extent_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_match_runtime_view_extent_through_const_name_exit_main_omg_3c86747bb2__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/may_write_codec_argument_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/may_write_codec_argument_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_may_write_codec_argument_through_other_state_exit_main_omg_042b2d7525__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/nested_borrow_qualification_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/nested_borrow_qualification_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_facts_nested_borrow_qualification_through_other_state_exit_main_omg_7e935516a3__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_local_exit/main.omg`
- Original: `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/pass_facts_nested_machine_joint_rank_bounded_computed_length_local_exit_main_omg_18149cb31b__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/nested_result_extent_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_nested_result_extent_through_other_state_exit_main_omg_59252ddd66__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_nominal_loan_backing_cleanup_through_parameter_exit_main_omg_0c61fa9932__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/nominal_reference_cleanup_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_facts_nominal_reference_cleanup_through_other_machine_exit_main_omg_510bb87af7__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/open_index_distributivity_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/open_index_distributivity_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_open_index_distributivity_through_field_exit_main_omg_2537ad810d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/positional_implementation_binder_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/positional_implementation_binder_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_facts_positional_implementation_binder_through_other_state_exit_main_omg_24de55432a__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/progress_selector_nested_arrival_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/progress_selector_nested_arrival_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_progress_selector_nested_arrival_through_local_exit_main_omg_92638dc58a__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/progress_selector_owned_arrival_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/progress_selector_owned_arrival_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_facts_progress_selector_owned_arrival_through_field_exit_main_omg_3408d351da__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/progress_selector_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/progress_selector_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_progress_selector_through_const_name_exit_main_omg_6d6a35ecf8__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/projection_call_premise_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/projection_call_premise_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_projection_call_premise_through_call_result_exit_main_omg_9892f948b2__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/projection_call_premise_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/projection_call_premise_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_projection_call_premise_through_target_body_exit_main_omg_b463505e1c__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/provider_contract_endpoint_pending_call_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/a_pass_facts_provider_contract_endpoint_pending_call_through_field_exit_main_omg_f7129981ce__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/qualified_field_capacity_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/qualified_field_capacity_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_qualified_field_capacity_through_call_result_exit_main_omg_8ccfc59b8f__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/qualified_field_capacity_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/qualified_field_capacity_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_qualified_field_capacity_through_target_body_exit_main_omg_5f5df79949__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/range_fact_delivery_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/range_fact_delivery_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_range_fact_delivery_through_other_state_exit_main_omg_e762ef210e__drop_requires`
- Current checker: **accepted**; route: **accepted**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/recast_parent_restoration_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/recast_parent_restoration_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_recast_parent_restoration_through_local_exit_main_omg_205f685e41__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/reflection_declaration_inspection_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/reflection_declaration_inspection_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_facts_reflection_declaration_inspection_through_match_join_exit_main_omg_e991c27fc4__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/replaced_field_loan_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/replaced_field_loan_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_replaced_field_loan_through_const_name_exit_main_omg_10e78ae151__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/returned_indexed_linear_residual_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/returned_indexed_linear_residual_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_returned_indexed_linear_residual_through_field_exit_main_omg_b70dfe47a9__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/returned_linear_residual_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/returned_linear_residual_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_returned_linear_residual_through_const_name_exit_main_omg_1781805224__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/returned_write_destination_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/returned_write_destination_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_returned_write_destination_through_call_result_exit_main_omg_a1632bc7be__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/returned_write_destination_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/returned_write_destination_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_returned_write_destination_through_target_body_exit_main_omg_0f6d0ffbd4__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_call_result_exit/main.omg`
- Original: `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_runtime_index_application_mutating_operator_through_call_result_exit_main_omg_12d1a43a0f__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_runtime_index_application_mutating_operator_through_target_body_exit_main_omg_c3f8f44861__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/runtime_index_application_ordinary_call_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/runtime_index_application_ordinary_call_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_facts_runtime_index_application_ordinary_call_through_other_state_exit_main_omg_797e5f51a7__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/saturating_integer_arithmetic_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/saturating_integer_arithmetic_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_saturating_integer_arithmetic_through_field_exit_main_omg_58605756ff__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/saved_case_predicate_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/saved_case_predicate_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_saved_case_predicate_through_const_name_exit_main_omg_5e226dfce4__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/saved_observation_snapshot_after_call_relational_exit/main.omg`
- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_relational_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_omega_pass_facts_saved_observation_snapshot_after_call_relational_exit_main_omg_b3f7a12a61__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/_facts_saved_observation_snapshot_after_call_through_other_machine_exit_main_omg_fb51a8dab5__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/saved_observation_snapshot_through_parameter_exit/main.omg`
- Original: `tests/omega/pass/facts/saved_observation_snapshot_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_saved_observation_snapshot_through_parameter_exit_main_omg_23c684f79f__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/selected_native_code_layout_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/selected_native_code_layout_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_facts_selected_native_code_layout_through_other_machine_exit_main_omg_2ee70f6a8e__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_state_exit/main.omg`
- Original: `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/omega_pass_facts_signed_arithmetic_shift_right_through_other_state_exit_main_omg_a50a5685c3__drop_requires`
- Current checker: **accepted**; route: **rejected**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/signed_exact_addition_bound_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/signed_exact_addition_bound_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_signed_exact_addition_bound_through_local_exit_main_omg_c639dcd803__drop_requires`
- Current checker: **accepted**; route: **accepted**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/signed_exact_multiplication_bound_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/signed_exact_multiplication_bound_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_facts_signed_exact_multiplication_bound_through_field_exit_main_omg_aa6dfef9ed__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/static_evidence_premise_through_const_name_exit/main.omg`
- Original: `tests/omega/pass/facts/static_evidence_premise_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_static_evidence_premise_through_const_name_exit_main_omg_6db616f84b__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/typed_reflection_mutation_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/typed_reflection_mutation_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_facts_typed_reflection_mutation_through_match_join_exit_main_omg_086b6cbf1d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_unsigned_64bit_bitwise_xor_bound_through_local_exit_main_omg_53d5576a2d__drop_requires`
- Current checker: **accepted**; route: **rejected**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/facts/unsigned_8bit_shift_right_bound_through_target_body_exit/main.omg`
- Original: `tests/omega/pass/facts/unsigned_8bit_shift_right_bound_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ega_pass_facts_unsigned_8bit_shift_right_bound_through_target_body_exit_main_omg_d504bdd180__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/write_only_ieee_store_readback_through_other_machine_exit/main.omg`
- Original: `tests/omega/pass/facts/write_only_ieee_store_readback_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ga_pass_facts_write_only_ieee_store_readback_through_other_machine_exit_main_omg_c3db16312a__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/facts/write_only_indexed_reborrow_through_match_join_exit/main.omg`
- Original: `tests/omega/pass/facts/write_only_indexed_reborrow_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ts_omega_pass_facts_write_only_indexed_reborrow_through_match_join_exit_main_omg_d5dd6cd13d__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/facts/write_only_match_join_through_local_exit/main.omg`
- Original: `tests/omega/pass/facts/write_only_match_join_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_write_only_match_join_through_local_exit_main_omg_4de487ee7f__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/facts/write_only_recast_through_field_exit/main.omg`
- Original: `tests/omega/pass/facts/write_only_recast_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_facts_write_only_recast_through_field_exit_main_omg_e84f68690c__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/float/authored_heterogeneous_float_operator/main.omg`
- Original: `tests/omega/pass/float/authored_heterogeneous_float_operator/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_float_authored_heterogeneous_float_operator_main_omg_a2417ca9cb__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/float/computed_comparison_literal_landing_exit/main.omg`
- Original: `tests/omega/pass/float/computed_comparison_literal_landing_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_float_computed_comparison_literal_landing_exit_main_omg_6206ffe0ab__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `invert_comparison_guard` — `tests/omega/pass/float/saturating_named_operations_retain_ordered_inputs/main.omg`
- Original: `tests/omega/pass/float/saturating_named_operations_retain_ordered_inputs/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ests_omega_pass_float_saturating_named_operations_retain_ordered_inputs_main_omg_678e134404__invert_comparison_guard`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Only the selected arm executes, with ordinary once-only operand evaluation.

### `drop_requires` — `tests/omega/pass/generics/const_index_active_case_predicates_exit/main.omg`
- Original: `tests/omega/pass/generics/const_index_active_case_predicates_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_const_index_active_case_predicates_exit_main_omg_08c140b35d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/generics/inferred_equation_computed_satisfaction_exit/main.omg`
- Original: `tests/omega/pass/generics/inferred_equation_computed_satisfaction_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_inferred_equation_computed_satisfaction_exit_main_omg_b4bf9e1499__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/generics/inferred_equation_typed_const_const_name_exit/main.omg`
- Original: `tests/omega/pass/generics/inferred_equation_typed_const_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_inferred_equation_typed_const_const_name_exit_main_omg_4979139fc1__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/generics/runtime_call_index_contract_result/main.omg`
- Original: `tests/omega/pass/generics/runtime_call_index_contract_result/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_runtime_call_index_contract_result_main_omg_c143532780__widen_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/generics/runtime_computed_nested_carrier_exit/main.omg`
- Original: `tests/omega/pass/generics/runtime_computed_nested_carrier_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_runtime_computed_nested_carrier_exit_main_omg_c6ab3483ad__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/generics/runtime_finite_recursive_reference_catalog_exit/main.omg`
- Original: `tests/omega/pass/generics/runtime_finite_recursive_reference_catalog_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/sts_omega_pass_generics_runtime_finite_recursive_reference_catalog_exit_main_omg_099b667ef8__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/generics/runtime_index_carrier_argument_equal_exit/main.omg`
- Original: `tests/omega/pass/generics/runtime_index_carrier_argument_equal_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_runtime_index_carrier_argument_equal_exit_main_omg_d4d9ea8f8a__widen_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/generics/runtime_machine_equation_selected_const_exit/main.omg`
- Original: `tests/omega/pass/generics/runtime_machine_equation_selected_const_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_generics_runtime_machine_equation_selected_const_exit_main_omg_f0cdc6840c__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/host/runtime_descriptor_exclusive_atomic_record_exit/main.omg`
- Original: `tests/omega/pass/host/runtime_descriptor_exclusive_atomic_record_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_host_runtime_descriptor_exclusive_atomic_record_exit_main_omg_adae41e441__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/layouts/access_plan_inaccessible_seed/main.omg`
- Original: `tests/omega/pass/layouts/access_plan_inaccessible_seed/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_layouts_access_plan_inaccessible_seed_main_omg_07f76859ce__remove_mut`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/layouts/fixed_layout_unmentioned_valid_policy_exit/main.omg`
- Original: `tests/omega/pass/layouts/fixed_layout_unmentioned_valid_policy_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_layouts_fixed_layout_unmentioned_valid_policy_exit_main_omg_b052df6849__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/layouts/runtime_target_layout_byte_offset_exit/main.omg`
- Original: `tests/omega/pass/layouts/runtime_target_layout_byte_offset_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_layouts_runtime_target_layout_byte_offset_exit_main_omg_f36468e0d6__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `invert_comparison_guard` — `tests/omega/pass/operators/domain_qualified_boundary_operator_exit/main.omg`
- Original: `tests/omega/pass/operators/domain_qualified_boundary_operator_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_operators_domain_qualified_boundary_operator_exit_main_omg_97c0ad235f__invert_comparison_guard`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Only the selected arm executes, with ordinary once-only operand evaluation.

### `invert_comparison_guard` — `tests/omega/pass/ownership/match_selected_parameter_moves_exit/main.omg`
- Original: `tests/omega/pass/ownership/match_selected_parameter_moves_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_ownership_match_selected_parameter_moves_exit_main_omg_f5b038fe99__invert_comparison_guard`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Only the selected arm executes, with ordinary once-only operand evaluation.

### `amp_to_ampmut` — `tests/omega/pass/ownership/shared_array_view_compile/main.omg`
- Original: `tests/omega/pass/ownership/shared_array_view_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_ownership_shared_array_view_compile_main_omg_5f9af994e5__amp_to_ampmut`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### `remove_mut` — `tests/omega/pass/ports/asm_port_in_immediate/main.omg`
- Original: `tests/omega/pass/ports/asm_port_in_immediate/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_ports_asm_port_in_immediate_main_omg_348cab9a6d__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/proofs/applied_dependent_const_requires_transitive_operand/main.omg`
- Original: `tests/omega/pass/proofs/applied_dependent_const_requires_transitive_operand/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/s_omega_pass_proofs_applied_dependent_const_requires_transitive_operand_main_omg_7419b700b5__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_ensures` — `tests/omega/pass/proofs/embed_returned_length/main.omg`
- Original: `tests/omega/pass/proofs/embed_returned_length/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_proofs_embed_returned_length_main_omg_ee5c33b9a9__drop_ensures`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> A written `ensures` is a minimum guarantee, not an instruction to forget stronger local facts.

### `drop_requires` — `tests/omega/pass/proofs/nested_machine_inferred_requirement/main.omg`
- Original: `tests/omega/pass/proofs/nested_machine_inferred_requirement/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_proofs_nested_machine_inferred_requirement_main_omg_11a35299cc__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/proofs/numeric_state_contract_exit/main.omg`
- Original: `tests/omega/pass/proofs/numeric_state_contract_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_proofs_numeric_state_contract_exit_main_omg_fc6d082866__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `widen_requires` — `tests/omega/pass/providers/assumed_result_bound_local_exit/main.omg`
- Original: `tests/omega/pass/providers/assumed_result_bound_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_providers_assumed_result_bound_local_exit_main_omg_e889f901a9__widen_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/providers/staggered_installation_reach_exit/main.omg`
- Original: `tests/omega/pass/providers/staggered_installation_reach_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_providers_staggered_installation_reach_exit_main_omg_f60a9cafad__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/recast/callee_returned_scalar_interpretation_exit/main.omg`
- Original: `tests/omega/pass/recast/callee_returned_scalar_interpretation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_recast_callee_returned_scalar_interpretation_exit_main_omg_dfe2b8f76f__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/recast/non_owning_erasure_unchanged_input_exit/main.omg`
- Original: `tests/omega/pass/recast/non_owning_erasure_unchanged_input_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_recast_non_owning_erasure_unchanged_input_exit_main_omg_d3d0d7b315__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/recast/returned_view_state_and_record_composition_exit/main.omg`
- Original: `tests/omega/pass/recast/returned_view_state_and_record_composition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_recast_returned_view_state_and_record_composition_exit_main_omg_f5d5e87e24__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/recast/runtime_nested_view_subregion_forwarding_exit/main.omg`
- Original: `tests/omega/pass/recast/runtime_nested_view_subregion_forwarding_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_recast_runtime_nested_view_subregion_forwarding_exit_main_omg_7b19702576__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/recast/write_footprint/local/main.omg`
- Original: `tests/omega/pass/recast/write_footprint/local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_recast_write_footprint_local_main_omg_786ae294c5__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/relevance/erased_reference_parameter_exit/main.omg`
- Original: `tests/omega/pass/relevance/erased_reference_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_relevance_erased_reference_parameter_exit_main_omg_9ba0b9d00e__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/slices/runtime_matched_window_coordinate_capture_exit/main.omg`
- Original: `tests/omega/pass/slices/runtime_matched_window_coordinate_capture_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_slices_runtime_matched_window_coordinate_capture_exit_main_omg_20fc8152d7__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/terminal_psi/member_crash_contract_boundary/main.omg`
- Original: `tests/omega/pass/terminal_psi/member_crash_contract_boundary/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_terminal_psi_member_crash_contract_boundary_main_omg_240fa279d4__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/termination/progress_selector_owned_arrival_reference_boundary_exit/main.omg`
- Original: `tests/omega/pass/termination/progress_selector_owned_arrival_reference_boundary_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/ass_termination_progress_selector_owned_arrival_reference_boundary_exit_main_omg_4cf3389aa7__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/pass/traits/default_row_nested_captured_reference_exit/main.omg`
- Original: `tests/omega/pass/traits/default_row_nested_captured_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_traits_default_row_nested_captured_reference_exit_main_omg_91712ba730__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/pass/traits/public_guarantee_positional_formals_exit/main.omg`
- Original: `tests/omega/pass/traits/public_guarantee_positional_formals_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_traits_public_guarantee_positional_formals_exit_main_omg_7870db14fa__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/pass/types/address_comparison_symbolic_bound_compile/main.omg`
- Original: `tests/omega/pass/types/address_comparison_symbolic_bound_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_types_address_comparison_symbolic_bound_compile_main_omg_37a7860d64__drop_requires`
- Current checker: **accepted**; route: **no-entry**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/pass/wire/reflection_active_mutation_exit/main.omg`
- Original: `tests/omega/pass/wire/reflection_active_mutation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_wire_reflection_active_mutation_exit_main_omg_b13dd84902__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `remove_mut` — `tests/omega/run/domains/checked_provider_issued_root/main.omg`
- Original: `tests/omega/run/domains/checked_provider_issued_root/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_run_domains_checked_provider_issued_root_main_omg_9af9621003__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `remove_mut` — `tests/omega/run/domains/qualified_fixed_byte_array_utf8_parameter_exit/main.omg`
- Original: `tests/omega/run/domains/qualified_fixed_byte_array_utf8_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_run_domains_qualified_fixed_byte_array_utf8_parameter_exit_main_omg_1afce10f4e__remove_mut`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/ownership.md):
> Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.

### `drop_requires` — `tests/omega/run/domains/semantic_predicate_domain_admission/main.omg`
- Original: `tests/omega/run/domains/semantic_predicate_domain_admission/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_run_domains_semantic_predicate_domain_admission_main_omg_c0a9a60374__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/run/facts/double_indexed_rmw_through_local_traps/main.omg`
- Original: `tests/omega/run/facts/double_indexed_rmw_through_local_traps/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_run_facts_double_indexed_rmw_through_local_traps_main_omg_da2bca3a2d__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### `drop_requires` — `tests/omega/run/proofs/embed_collection_length_exit/main.omg`
- Original: `tests/omega/run/proofs/embed_collection_length_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_run_proofs_embed_collection_length_exit_main_omg_ddfc25ec26__drop_requires`
- Current checker: **accepted**; route: **outside**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

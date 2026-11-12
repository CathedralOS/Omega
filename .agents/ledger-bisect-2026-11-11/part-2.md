# Ledger bisect part 2

PIN `51e8c45c793007f211d0d9baee7de16767172444`. PIN-fixed ledger + observation; ledger-only commits skipped in `git bisect run`.

**Re-measure:** 50 fixtures; **fixed on main** 43; **still broken** 7; **clusters** 5.

## Cluster 0 (2 rows)

- **Diagnostic:** cannot prove published termination for machine `FamilyConsole::write_byte`: its checked body reaches an operation withou
- **Witness:** `run/ownership/sequence_owned_local_cleanup`
- **Status:** no_good_on_first_parent
- **Members:** `pass/traits/inline_member_seals_target_family_provider`, `run/ownership/sequence_owned_local_cleanup`

## Cluster 1 (2 rows)

- **Diagnostic:** data `Locator` literal cannot prove the default domain: a `requires` fact is not established by the values observed at t
- **Witness:** `pass/facts/static_callback_reach_through_field`
- **Status:** pending
- **Members:** `pass/domains/private_case_payload_construction`, `pass/facts/static_callback_reach_through_field`

## Cluster 2 (1 rows)

- **Diagnostic:** cannot establish assignment invariants: partial write enclosure has no exact active-case frontier
- **Witness:** `pass/borrows/returned_record_case_field_store_exit`
- **Status:** pending
- **Members:** `pass/borrows/returned_record_case_field_store_exit`

## Cluster 3 (1 rows)

- **Diagnostic:** cannot establish assignment invariants: source parent type is unavailable
- **Witness:** `pass/borrows/runtime_reassigned_view_capture_exit`
- **Status:** pending
- **Members:** `pass/borrows/runtime_reassigned_view_capture_exit`

## Cluster 4 (1 rows)

- **Diagnostic:** selected optimizations `SelectedSameBlockCopyI64RemovalV1`, `SelectedRedundantExtensionRemovalV1`, `SelectedAddressOffse
- **Witness:** `pass/optimizer/pre_allocation_joint_frontier_exit`
- **Status:** pending
- **Members:** `pass/optimizer/pre_allocation_joint_frontier_exit`

## Fixed on main

- `fail/dependent/case_where_unproved_bound_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_call_result_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_const_name_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_field_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_local_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_match_join_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_other_machine_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_other_state_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_parameter_rejected`
- `fail/facts/descriptor_range_exclusive_loan_through_target_body_rejected`
- `fail/facts/match_loan_last_use_through_call_result_rejected`
- `fail/facts/match_loan_last_use_through_const_name_rejected`
- `fail/facts/match_loan_last_use_through_field_rejected`
- `fail/facts/match_loan_last_use_through_local_rejected`
- `fail/facts/match_loan_last_use_through_match_join_rejected`
- `fail/facts/match_loan_last_use_through_other_machine_rejected`
- `fail/facts/match_loan_last_use_through_other_state_rejected`
- `fail/facts/match_loan_last_use_through_parameter_rejected`
- `fail/facts/match_loan_last_use_through_target_body_rejected`
- `fail/facts/static_callback_reach_through_field`
- `fail/traits/dyn_finite_family_narrowed_provider`
- `pass/borrows/runtime_reassigned_view_return_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_call_result_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_const_name_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_field_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_local_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_match_join_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_other_machine_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_other_state_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_parameter_exit`
- `pass/facts/descriptor_range_exclusive_loan_through_target_body_exit`
- `pass/facts/match_loan_last_use_through_call_result_exit`
- `pass/facts/match_loan_last_use_through_const_name_exit`
- `pass/facts/match_loan_last_use_through_field_exit`
- `pass/facts/match_loan_last_use_through_local_exit`
- `pass/facts/match_loan_last_use_through_match_join_exit`
- `pass/facts/match_loan_last_use_through_other_machine_exit`
- `pass/facts/match_loan_last_use_through_other_state_exit`
- `pass/facts/match_loan_last_use_through_parameter_exit`
- `pass/facts/match_loan_last_use_through_target_body_exit`
- `pass/facts/target_family_callback_guarantee/const_name`
- `pass/slices/let_matched_mutable_window`
- `pass/termination/progress_selector_owned_arrival_replacement_helper_exit`

# Ledger bisect part 2 (PIN 51e8c45c7930)

Slice: report cluster indices % 40 == 2 (6 prior tables, 50 fixtures re-measured).

**PIN counts:** fixed on main 43; still broken 7.

## Clusters at PIN (largest first)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run regression | cannot prove published termination for machine `FamilyConsole::write_byte`: its checked body reaches | `pass/traits/inline_member_seals_target_family_provider` | `` |  |  | pending |
- `pass/traits/inline_member_seals_target_family_provider`
- `run/ownership/sequence_owned_local_cleanup`

| 2 | pass/run regression | data `Locator` literal cannot prove the default domain: a `requires` fact is not established by the  | `pass/domains/private_case_payload_construction` | `` |  |  | pending |
- `pass/domains/private_case_payload_construction`
- `pass/facts/static_callback_reach_through_field`

| 1 | pass/run regression | cannot establish assignment invariants: partial write enclosure has no exact active-case frontier | `pass/borrows/returned_record_case_field_store_exit` | `` |  |  | pending |
- `pass/borrows/returned_record_case_field_store_exit`

| 1 | pass/run regression | cannot establish assignment invariants: source parent type is unavailable | `pass/borrows/runtime_reassigned_view_capture_exit` | `` |  |  | pending |
- `pass/borrows/runtime_reassigned_view_capture_exit`

| 1 | pass/run regression | selected optimizations `SelectedSameBlockCopyI64RemovalV1`, `SelectedRedundantExtensionRemovalV1`, ` | `pass/optimizer/pre_allocation_joint_frontier_exit` | `f3612364ffbb` | 2228 | tests: record four case-payload rows that already check (#2228) | bisected |
- `pass/optimizer/pre_allocation_joint_frontier_exit`

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

## Commands

- Member recovery: `python3 .agents/ledger-bisect-2026-11-11/recover_slice_members.py 2 40`
- Re-measure: direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER` (50 fixtures, jobs=1)
- Bisect: `.agents/ledger-bisect-2026-11-11/bisect_one.sh <witness>` (good=759c80404428, bad=PIN, `--first-parent`)
- Host: linux x86_64; OMEGA_CORPUS_JOBS=1

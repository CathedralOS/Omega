# Ledger bisect part 0 (k=0, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## PIN cluster summary

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run rejected | cannot prove ensures contract for exit from HeapProvider::allocate at statement 0: result in Allocation::Held; the retur | `pass/memory/runtime_linear_native_allocation_call_result_exit` | `` |  |  | propagated |
Members: `pass/memory/runtime_linear_native_allocation_call_result_exit` `pass/memory/runtime_linear_native_allocation_exit`

| 2 | pass/run rejected | cannot prove ensures contract for exit from selected at statement 2: result >= Minimum | `pass/facts/open_const_implementation_statement_const_result_exit` | `` |  |  | propagated |
Members: `pass/facts/open_const_implementation_statement_const_result_exit` `pass/facts/open_const_implementation_statement_saved_output_exit`

| 1 | pass/run rejected | cannot establish assignment invariants: source parent type is unavailable | `pass/borrows/runtime_reassigned_view_capture_exit` | `` |  |  | propagated |
Members: `pass/borrows/runtime_reassigned_view_capture_exit`

| 1 | pass/run rejected | cannot prove ensures contract for exit from Selected::read at statement 0: result == Value | `pass/facts/open_const_implementation_private_record_package_exit` | `` |  |  | propagated |
Members: `pass/facts/open_const_implementation_private_record_package_exit`

| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value from Main::main: first >= 0 && first <= 100 (needs fact `valu | `pass/borrows/runtime_shared_reference_array_cohort_restoration_exit` | `` |  |  | propagated |
Members: `pass/borrows/runtime_shared_reference_array_cohort_restoration_exit`

## Prior-slice rows fixed on main

- `fail/borrows/reassigned_view_conflicts_with_captured_child`
- `fail/facts/open_array_const_implementation_through_call_result_rejected`
- `fail/facts/open_array_const_implementation_through_const_name_rejected`
- `fail/facts/open_array_const_implementation_through_field_rejected`
- `fail/facts/open_array_const_implementation_through_local_rejected`
- `fail/facts/open_array_const_implementation_through_match_join_rejected`
- `fail/facts/open_array_const_implementation_through_other_machine_rejected`
- `fail/facts/open_array_const_implementation_through_other_state_rejected`
- `fail/facts/open_array_const_implementation_through_parameter_rejected`
- `fail/facts/open_array_const_implementation_through_target_body_rejected`
- `fail/facts/open_const_implementation_changed_statement_output`
- `fail/facts/open_const_implementation_private_record_package_rejected`
- `fail/facts/open_const_implementation_private_result_guarantee`
- `fail/facts/open_const_implementation_return_write_output`
- `fail/facts/open_const_implementation_statement_const_result_rejected`
- `fail/facts/open_const_implementation_statement_saved_output_rejected`
- `fail/facts/open_const_implementation_through_call_result_rejected`
- `fail/facts/open_const_implementation_through_const_name_rejected`
- `fail/facts/open_const_implementation_through_field_rejected`
- `fail/facts/open_const_implementation_through_local_rejected`
- `fail/facts/open_const_implementation_through_match_join_rejected`
- `fail/facts/open_const_implementation_through_other_machine_rejected`
- `fail/facts/open_const_implementation_through_other_state_rejected`
- `fail/facts/open_const_implementation_through_parameter_rejected`
- `fail/facts/open_const_implementation_through_target_body_rejected`
- `fail/facts/open_record_const_implementation_specialization_agreement_rejected`
- `fail/facts/open_record_const_implementation_through_call_result_rejected`
- `fail/facts/open_record_const_implementation_through_const_name_rejected`
- `fail/facts/open_record_const_implementation_through_field_rejected`
- `fail/facts/open_record_const_implementation_through_local_rejected`
- `fail/facts/open_record_const_implementation_through_match_join_rejected`
- `fail/facts/open_record_const_implementation_through_other_machine_rejected`
- `fail/facts/open_record_const_implementation_through_other_state_rejected`
- `fail/facts/open_record_const_implementation_through_parameter_rejected`
- `fail/facts/open_record_const_implementation_through_target_body_rejected`
- `pass/domains/runtime_selected_nested_field_domain_exit`
- `pass/facts/open_array_const_implementation_through_call_result_exit`
- `pass/facts/open_array_const_implementation_through_const_name_exit`
- `pass/facts/open_array_const_implementation_through_field_exit`
- `pass/facts/open_array_const_implementation_through_local_exit`
- `pass/facts/open_array_const_implementation_through_match_join_exit`
- `pass/facts/open_array_const_implementation_through_other_machine_exit`
- `pass/facts/open_array_const_implementation_through_other_state_exit`
- `pass/facts/open_array_const_implementation_through_parameter_exit`
- `pass/facts/open_array_const_implementation_through_target_body_exit`
- `pass/facts/open_const_implementation_through_call_result_exit`
- `pass/facts/open_const_implementation_through_const_name_exit`
- `pass/facts/open_const_implementation_through_field_exit`
- `pass/facts/open_const_implementation_through_local_exit`
- `pass/facts/open_const_implementation_through_match_join_exit`
- `pass/facts/open_const_implementation_through_other_machine_exit`
- `pass/facts/open_const_implementation_through_other_state_exit`
- `pass/facts/open_const_implementation_through_parameter_exit`
- `pass/facts/open_const_implementation_through_target_body_exit`
- `pass/facts/open_record_const_implementation_through_call_result_exit`
- `pass/facts/open_record_const_implementation_through_const_name_exit`
- `pass/facts/open_record_const_implementation_through_field_exit`
- `pass/facts/open_record_const_implementation_through_local_exit`
- `pass/facts/open_record_const_implementation_through_match_join_exit`
- `pass/facts/open_record_const_implementation_through_other_machine_exit`
- `pass/facts/open_record_const_implementation_through_other_state_exit`
- `pass/facts/open_record_const_implementation_through_parameter_exit`
- `pass/facts/open_record_const_implementation_through_target_body_exit`
- `pass/facts/target_family_callback_guarantee/match_join`
- `pass/slices/narrowed_view_window`
- `run/domains/qualified_record_view_reference_erased_storage_exit`

## Prior slice member recovery

- Prior cluster idx 0 report rows=70; recovered **64** paths from `regressed_fixtures.txt` open_const_implementation families (corpus batches incomplete on report branch).

## Provenance

- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`
- Commands: `.agents/ledger-bisect-2026-11-11/tools/run_direct_filter.sh`, `git bisect run --first-parent` via `run_bisect_cluster.sh`
- Bisect worktree: `/tmp/bisect-wt`

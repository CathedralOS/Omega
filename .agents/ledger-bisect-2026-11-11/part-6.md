# Ledger bisect part 6

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Prior-report slice indices (index % 40 == 6): 6, 46, 86, 126, 166, 206
- Rows re-measured: 28 (fixed on main: 13, still broken: 15)
- PIN clusters (by first diagnostic): 4 (bisected: 0)

## PIN clusters

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 11 | pass/run regression | no provider is selected for boundary requirement `Window::middle`, which the program entry reaches o | `pass/providers/specialized_mixed_structural_result_operator_hosted_native` | `` |  |  | pending |
Members: `pass/borrows/runtime_boundary_range_view_exit`, `pass/borrows/runtime_computed_shared_view_reborrow_exit`, `pass/providers/boundary_declaration_structural_result_return_exit`, `pass/providers/lifetime_boundary_requirement_dispatch_exit`, `pass/providers/specialized_boundary_operator_physical_custody`, `pass/providers/specialized_mixed_structural_fixed_operator_hosted_native`, `pass/providers/specialized_mixed_structural_result_operator_hosted_native`, `pass/providers/specialized_structural_fixed_operator_hosted_native`, `pass/providers/specialized_structural_fixed_operator_terminal_custody`, `pass/providers/table_leaf_dispatch_parameter_exit`, `pass/providers/target_contract_adjacency_exit`

| 2 | pass/run regression | terminal-artifact production failed: Lowering(CompilerEvaluationAdmission("machine `@constant-UNUSED | `pass/layouts/declaration_only_initializer_formation` | `` |  |  | pending |
Members: `pass/layouts/declaration_only_initializer_formation`, `pass/terminal_psi/inspection_scalar_library`

| 1 | pass/run regression | authored Operator declaration selection occurrence 9690 (`*` in tests/omega/pass/expressions/match_o | `pass/expressions/match_operand_anonymous_operation_exit` | `` |  |  | pending |
Members: `pass/expressions/match_operand_anonymous_operation_exit`

| 1 | pass/run regression | checked machine `related_reflexive` cannot establish proposition ensure `boolean:value == value` in  | `pass/proofs/proposition_relation_hierarchy_compile` | `` |  |  | pending |
Members: `pass/proofs/proposition_relation_hierarchy_compile`

## Fixed on main (slice rows)

- `fail/arithmetic/exact_overflow_value_call_hint`
- `fail/build/native_table_non_address_field`
- `fail/calls/callback_shared_program_excluded_trap`
- `fail/calls/foreign_applied_callback_requires_selected_entry_plan`
- `fail/generics/native_locator_inconsistent_byte_extents`
- `fail/generics/native_locator_undefined_user_coordinate`
- `fail/layouts/declaration_only_initializer_formation`
- `fail/terminal_psi/inspection_scalar_library`
- `pass/borrow/runtime_write_only_boundary_satisfier_exit`
- `pass/build/native_component_reuse_foreign`
- `pass/calls/callback_future_registration_cycle`
- `pass/calls/private_callback_call_closure`
- `pass/calls/resultless_native_callback_compile`

## Prior-report assignment (member recovery)

- old index 6: 22 rows, witness `pass/borrow/runtime_write_only_boundary_satisfier_exit`, members measured 19
- old index 46: 4 rows, witness `fail/layouts/declaration_only_initializer_formation`, members measured 4
- old index 86: 2 rows, witness `fail/generics/native_locator_inconsistent_byte_extents`, members measured 2
- old index 126: 1 rows, witness `pass/expressions/match_operand_anonymous_operation_exit`, members measured 1
- old index 166: 1 rows, witness `pass/proofs/proposition_relation_hierarchy_compile`, members measured 1
- old index 206: 1 rows, witness `fail/arithmetic/exact_overflow_value_call_hint`, members measured 1

## Commands

- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`
- `bash .agents/ledger-bisect-2026-11-11/tools/measure_pin.sh slice6_fixtures.txt observed_pin.txt`
- `git bisect start <PIN> <good> --first-parent` + `git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh`

Agent: Cursor | Model: unknown | Effort: unknown | Tokens: unknown | Steps: unknown | Time: unknown | Steers: 0

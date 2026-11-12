# Ledger bisect part 31 (k=31, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **5** (indices 31, 71, 111, 151, 191)
- Fixture rows re-measured: **11**
- Fixed on PIN: **4**
- Still broken on PIN: **7**
- PIN diagnostic clusters (after regroup): **4**

## PIN clusters

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | pass/run rejected | cannot establish assignment invariants: performed write has unknown destination origins in machine `main` state `entry`  | `pass/borrows/runtime_selected_reference_mutation_complement_exit` | `b7e951cdab51dda4e199f3fd33c53a0408fe72db` | 1242 | psi #1242 assignment_windows/post_store_predicates tighten post-store destination supplier capture so selected-reference writes report unknown destination origins | bisected |
Members:
- `fail/borrows/conditional_reference_replacement_false_first_stale_index` — fixed on main: cannot establish assignment invariants: performed write has unknown destination origins in machine `
- `fail/borrows/conditional_reference_replacement_true_first_stale_index` — fixed on main: cannot establish assignment invariants: performed write has unknown destination origins in machine `
- `fail/slices/reborrowed_replaced_descriptor` — fixed on main: cannot establish assignment invariants: performed write has unknown destination origins in machine `
- `pass/borrows/runtime_selected_reference_mutation_complement_exit` — still broken: cannot establish assignment invariants: performed write has unknown destination origins in machine `
- `pass/borrows/runtime_uniform_projected_reference_selection_exit` — still broken: cannot establish assignment invariants: performed write has unknown destination origins in machine `
- `pass/write_only/returned_destination_transport_exit` — still broken: cannot establish assignment invariants: performed write has unknown destination origins in machine `

| 2 | incomplete | `main` calls `exercise`, which calls `window`; `window` has no executable body: this compiler does not lower a transitio | `pass/facts/computed_record_window_through_other_machine_exit` | `` |  |  | pending |
Members:
- `pass/facts/computed_record_window_through_other_machine_exit` — still broken: `main` calls `exercise`, which calls `window`; `window` has no executable body: this compiler does n
- `pass/facts/computed_record_window_through_other_state_exit` — still broken: `main` calls `exercise`, which calls `window`; `window` has no executable body: this compiler does n

| 1 | pass/run rejected | cannot prove requires contract for call compute from write: visits == 0 (needs fact `counter == 0` here; establish it be | `pass/facts/match_computed_root_through_other_state_exit` | `` |  |  | pending |
Members:
- `pass/facts/match_computed_root_through_other_state_exit` — still broken: cannot prove requires contract for call compute from write: visits == 0 (needs fact `counter == 0` h

| 1 | pass/run rejected | machine `received` state `entry` statement 6: operand has no complete completed call-slot relation | `pass/generics/generic_result_parameter_receipt` | `` |  |  | pending |
Members:
- `pass/generics/generic_result_parameter_receipt` — still broken: machine `received` state `entry` statement 6: operand has no complete completed call-slot relation

## Fixed on main (PIN)

- `fail/borrows/conditional_reference_replacement_false_first_stale_index`
- `fail/borrows/conditional_reference_replacement_true_first_stale_index`
- `fail/slices/reborrowed_replaced_descriptor`
- `pass/facts/target_family_contract_operand/call_result`

## Provenance

- Host: linux x86_64, 4 cpus
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`
- PIN ledger copy: `/tmp/ledger-bisect-pin/corpus_outcomes.txt`
- Measure: `mbx test --no-run -p compiler --test corpus_runner` + direct runner, filter 11 fixtures

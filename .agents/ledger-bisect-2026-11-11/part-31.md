# Ledger bisect part 31 (k=31, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **5** (report indices 31, 71, 111, 151, 191)
- Fixture rows re-measured: **11**
- Fixed on PIN: **4**
- Still broken on PIN: **7**
- PIN diagnostic clusters (after regroup): **4**

## PIN clusters (regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | pass/run rejected | cannot establish assignment invariants: performed write has unknown destination origins | `pass/borrows/runtime_selected_reference_mutation_complement_exit` | `b7e951cdab51dda4e199f3fd33c53a0408fe72db` | 1242 | psi #1242 assignment_windows/post_store_predicates tighten post-store destination supplier capture | bisected |
| 2 | incomplete | `main` calls `exercise`, which calls `window`; `window` has no executable body | `pass/facts/computed_record_window_through_other_machine_exit` | `2174c0d10bb146aea6a86f55262bc536feb86ea3` | 1313 | omega #1313 provider settlement prefers product providers over terminal demand | bisected |
| 1 | pass/run rejected | cannot prove requires contract for call compute from write | `pass/facts/match_computed_root_through_other_state_exit` | `d30fccc7aae65a2358024cf39788d1007e2661b4` | 2276 | psi #2276 private-machine requires proving stops carrying computed-root counter facts into compute | bisected |
| 1 | pass/run rejected | machine `received` state `entry` statement 6: operand has no complete completed call-slot relation | `pass/generics/generic_result_parameter_receipt` | `5ab4e12334fba74babfc3ae8d9992572fba307e6` | 348 | psi #348 nested invocation contracts drop completed call-slot relation for generic results | bisected |

Members:
- `pass/borrows/runtime_selected_reference_mutation_complement_exit` — still broken
- `pass/borrows/runtime_uniform_projected_reference_selection_exit` — still broken
- `pass/write_only/returned_destination_transport_exit` — still broken
- `fail/borrows/conditional_reference_replacement_false_first_stale_index` — fixed on main
- `fail/borrows/conditional_reference_replacement_true_first_stale_index` — fixed on main
- `fail/slices/reborrowed_replaced_descriptor` — fixed on main
- `pass/facts/computed_record_window_through_other_machine_exit` — still broken
- `pass/facts/computed_record_window_through_other_state_exit` — still broken
- `pass/facts/target_family_contract_operand/call_result` — fixed on main
- `pass/facts/match_computed_root_through_other_state_exit` — still broken
- `pass/generics/generic_result_parameter_receipt` — still broken

## Fixed on main (PIN)

- `fail/borrows/conditional_reference_replacement_false_first_stale_index`
- `fail/borrows/conditional_reference_replacement_true_first_stale_index`
- `fail/slices/reborrowed_replaced_descriptor`
- `pass/facts/target_family_contract_operand/call_result`

## Legacy slice (report 2026-11-11)

| report idx | report rows | witness (report) | PIN outcome |
| ---: | ---: | --- | --- |
| 31 | 6 | `fail/borrows/conditional_reference_replacement_false_first_stale_index` | 3 pass still broken (assignment invariants); 3 fail fixed |
| 71 | 2 | `pass/facts/computed_record_window_through_other_machine_exit` | 2 still broken (window body missing) |
| 111 | 1 | `pass/facts/target_family_contract_operand/call_result` | fixed on main |
| 151 | 1 | `pass/facts/match_computed_root_through_other_state_exit` | still broken |
| 191 | 1 | `pass/generics/generic_result_parameter_receipt` | still broken |

## Provenance

- Host: linux x86_64, 4 cpus
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`
- PIN ledger copy: `/tmp/ledger-bisect-pin/corpus_outcomes.txt`; per-witness specs under `/tmp/ledger-bisect-pin/`
- Bisect helper: `/tmp/ledger-bisect-pin/tools/bisect_witness.sh` (skips ledger-only commits via exit 125)
- Measure: `mbx test --no-run -p compiler --test corpus_runner` + direct runner on 11 fixtures (`part-31-measure.txt`)
- Re-measure command: `OMEGA_CORPUS_FIXTURE_FILTER=<11 fixtures> OMEGA_CORPUS_RECORDS=... corpus_runner`
- Good commits (first-parent walk vs fixed PIN specs): c0 `c64728d90e83`, c1 `773b8422694b`, c2 `442a95195e64`, c3 `42ef40205f19`

# Ledger bisect part 29 (k=29, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Slice rows (prior report clusters index % 40 == 29): 11
- Fixed on PIN: 8
- Still broken (re-clustered): 4

## Clusters

| Rows | Kind | Diagnostic | Witness | Break | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass/run rejected | `two_children` has no executable body: this compiler does no | `pass/ownership/runtime_match_conditional_observation_cleanup_exit` | `22f215eb3e7d` | 2078 | psi: preserve common fields when matching case values (#2078) | commit 22f215eb3e7d9814cb3e29f42856e | bisected |
| 1 | incomplete | `main` has no executable body: call operation: structural re | `run/borrows/returned_descriptor_entry_snapshot` | `pending` | pending | pending | pending |
| 1 | incomplete | `main` has no executable body: reference result has no compl | `run/slices/returned_record_fixed_array_window` | `pending` | pending | pending | pending |
| 1 | pass/run rejected | cannot prove requires contract for call capacity from geomet | `pass/proofs/reviewed_mathematical_data_contract_exit` | `pending` | pending | pending | pending |

## Members

### c0
- `pass/ownership/runtime_match_conditional_observation_cleanup_exit` still broken
- good first-parent: `2476d1be2203`
### c1
- `run/borrows/returned_descriptor_entry_snapshot` still broken
### c2
- `run/slices/returned_record_fixed_array_window` still broken
### c3
- `pass/proofs/reviewed_mathematical_data_contract_exit` still broken

## Fixed on main

- `pass/layouts/placed_reference_results/two_live`
- `pass/recast/returned_qualification_preserves_representation_exit`
- `pass/recast/returned_qualified_reference_exit`
- `pass/borrows/runtime_stored_carrier_exchange_exit`
- `fail/calls/static_boundary_statement_unrelated_evidence`
- `fail/traits/explicit_implementation_unrelated_trait`
- `fail/layouts/placed_stable_array_readonly_write`
- `fail/traits/prepared_callable_wrong_member_source`

## Commands

- `git fetch --unshallow origin`
- `python3 .agents/ledger-bisect-tools/pin_bisect_state.py <witnesses...>`
- `bash .agents/ledger-bisect-tools/bisect_cluster.sh <id> <witness> <out.json>`
- Host: linux x86_64; `OMEGA_CORPUS_JOBS=1`; direct `corpus_runner`

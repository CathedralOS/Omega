# Ledger bisect part 29 (k=29, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Prior-report slice clusters (index % 40 == 29): 5 original clusters, 11 member rows
- Fixed on PIN: 8
- Still broken at PIN (re-clustered by first diagnostic): 4

## Clusters

| Rows | Kind | Diagnostic | Witness | Break | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass/run rejected | `two_children` has no executable body | `pass/ownership/runtime_match_conditional_observation_cleanup_exit` | `22f215eb3e7d` | 2078 | case-pattern common-field preservation stopped lowering two_children observation cleanup (#2078) | bisected |
| 1 | incomplete | `main` has no executable body: structural result loan | `run/borrows/returned_descriptor_entry_snapshot` | `77271bfb9134` | 1794 | incoming backing for stored-call structural result loans stopped lowering returned descriptor entry (#1794) | bisected |
| 1 | incomplete | `main` has no executable body: captured origin paths | `run/slices/returned_record_fixed_array_window` | `e8fb4b1364b0` | 1432 | case-where equality Build selection broke reference-result captured-origin lowering (#1432) | bisected |
| 1 | pass/run rejected | cannot prove requires contract for call capacity from geom | `pass/proofs/reviewed_mathematical_data_contract_exit` | `6326c6ae7aa9` | 2166 | binder contract dependency ratification dropped geometry capacity requires facts (#2166) | bisected |

## Members

### c0
- `pass/ownership/runtime_match_conditional_observation_cleanup_exit`
- good (first-parent search): `2476d1be2203`
### c1
- `run/borrows/returned_descriptor_entry_snapshot`
- good (first-parent search): `2476d1be2203`
### c2
- `run/slices/returned_record_fixed_array_window`
- good (first-parent search): `bd78ac0d130b`
### c3
- `pass/proofs/reviewed_mathematical_data_contract_exit`
- good (first-parent search): `11bdd2fe41ff`

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
- `python3 .agents/ledger-bisect-tools/pin_bisect_state.py` (4 witnesses)
- PIN-fixed `/tmp/pin_corpus_outcomes.txt`, `/tmp/bisect_pin_obs.json`
- `bash /tmp/ledger-bisect-tools/bisect_cluster.sh` per cluster
- Host: linux x86_64; `OMEGA_CORPUS_JOBS=1`; direct `corpus_runner`

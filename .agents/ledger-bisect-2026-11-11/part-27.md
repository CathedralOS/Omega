# Ledger bisect part 27 (k=27, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

Prior report cluster indices in this slice: **27, 67, 107, 147, 187** (13 member rows; full member lists recovered from the report — no `(+N more)` gaps in this slice).

## PIN summary

| | Count |
| --- | ---: |
| Prior-slice rows re-measured | 13 |
| Fixed on PIN | 5 |
| Still broken on PIN | 8 |
| PIN diagnostic clusters | 7 |

## PIN clusters (regrouped by first diagnostic)

| id | rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| --- | ---: | --- | --- | --- | --- | ---: | --- | --- |
| c01 | 2 | pass/run rejected | unknown canonical target profile `reserved_target` | `pass/targets/private_target_supplies_remain_disclosed` | `3009cab5a41cba2b0491f3ef35a7b073c18986da` | 1395 | Psi selected-requirement static evidence/argument handling (#1395) is the first compiler commit after good `356c7fae5106` where the witness fails the fixed-PIN ledger | bisected |
| c02 | 1 | pass/run rejected | unknown canonical target profile `archive_target` | `pass/targets/retained_target_family_bodies` | | | | pending |
| c03 | 1 | pass/run rejected | state `selected` receives `main` while local borrow `pair` is still active | `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` | | | | pending |
| c04 | 1 | pass/run rejected | state `finish` receives `pool` while local borrow `field` is still active | `run/borrows/terminal_case_completed_observation` | | | | pending |
| c05 | 1 | incomplete | `main` has no executable body (completion stored reference dependencies) | `pass/traits/prepared_callable_nested_borrow_paths_exit` | | | | pending |
| c06 | 1 | pass/run rejected | cannot prove requires contract for call agree from read_kept | `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit` | | | | pending |
| c07 | 1 | pass/run rejected | cannot prove selected case for payload access `chosen.value` | `pass/facts/case_payload_exclusive_loan_through_other_machine_exit` | | | | pending |

### Members

**c01:** `pass/targets/private_target_supplies_remain_disclosed`, `pass/targets/retained_target_provider_family_exit`

**c02:** `pass/targets/retained_target_family_bodies`

**c03:** `pass/borrows/runtime_stored_reference_case_residual_retirement_exit`

**c04:** `run/borrows/terminal_case_completed_observation`

**c05:** `pass/traits/prepared_callable_nested_borrow_paths_exit`

**c06:** `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit`

**c07:** `pass/facts/case_payload_exclusive_loan_through_other_machine_exit`

## Fixed on PIN (prior-slice rows)

- `fail/contracts/target_family_recursive_guarantee`
- `fail/contracts/target_family_unshared_result`
- `fail/targets/sibling_body_type_error_rejected`
- `fail/targets/unrealized_conformance_rejected`
- `fail/borrows/aliased_forwarded_placed_views`

## Methodology

Fixed-PIN bisect (lead correction #2): `tests/omega/corpus_outcomes.txt` and each witness’s PIN observation live under `.agents/ledger-bisect-2026-11-11/pin-witness/*.json`. `bisect_witness.sh` skips ledger-only commits (`tests/omega/corpus_outcomes.txt` / `*.md` only), exits 125 on build failure, 0 when the witness meets fixed-PIN ledger expectation, 1 when it is worse or matches the PIN-broken signature. Good commit: first-parent walk back from PIN (exponential + binary depth search). Prior ledger-at-revision bisects on this slice are **void** (e.g. `97f86261016…` / `f3612364` ledger-only).

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler corpus_runner --no-run < /dev/null
# measure: OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER=... OMEGA_CORPUS_RECORDS=... corpus_runner
# bisect: bash .agents/ledger-bisect-2026-11-11/tools/bisect_cluster.sh c0N <witness>
```

Host: cloud VM. `OMEGA_CORPUS_JOBS=1`.

# Ledger bisect part 23 (k=23, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Rows in slice: **14** — fixed on PIN (expected outcome): **3**, still broken: **11**.

## Cluster table

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | pass/run rejected | machine `Main::main` state `main` expression type application uses undeclared lifetime argument `'st | `pass/borrows/carried_loan_replacement` | `ccd41080b4c8` | 1945 | #1945 retains generic constructor/type-inference payloads through owned construction desti | bisected |

## Members

### c63_lifetime_carrier (3 rows)
- `fail/borrows/replaced_carrier_keeps_new_backing_loan` — fixed — obs `rejected` — machine `reject_live_write` state `entry` expression type application uses undeclared lifetime argum
- `pass/borrows/carried_loan_replacement` — broken — obs `rejected` — machine `Main::main` state `main` expression type application uses undeclared lifetime argument `'st
- `pass/ownership/runtime_carried_loan_field_replacement_exit` — broken — obs `rejected` — machine `Main::main` state `main` expression type application uses undeclared lifetime argument `'st

## Fixed-on-main members (cluster-level)

- **c63_lifetime_carrier**: `fail/borrows/replaced_carrier_keeps_new_backing_loan`

## Commands

- Host: linux x86_64
- Jobs: `OMEGA_CORPUS_JOBS=1`; macOS `check-target` batches use `OMEGA_CORPUS_NATIVE=1` + `OMEGA_CORPUS_NATIVE_TARGET=macos_arm64`
- PIN ledger copy: `/tmp/ledger_bisect_part23_corpus_outcomes.txt`
- Direct runner: `.agents/ledger-bisect-2026-11-11/measure_fixtures.sh`
- Bisect: `.agents/ledger-bisect-2026-11-11/run_cluster_bisect.sh` with fixed PIN witness specs


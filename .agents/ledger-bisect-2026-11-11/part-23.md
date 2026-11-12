# Ledger bisect part 23 (k=23, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Rows in slice: **14** — fixed on PIN (expected outcome): **3**, still broken: **11**.

## Cluster table

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 7 | pass/run regression | cannot prove requires contract for call receive_inferred from main: Count == 7 | `pass/borrows/runtime_dependent_callback_window_byte_exit` | `` |  |  | propagated |
| 3 | pass/run regression | undeclared lifetime argument `'storage' in `Carrier` | `pass/borrows/carried_loan_replacement` | `ccd41080b4c8` | 1945 | psi #1945 retains generic constructor/type-inference through owned construction destinatio | bisected |
| 1 | fail still accepts (fixed) | division by zero in machine `quotient` | `fail/arithmetic/indexed_divisor_copied_selector_storage_write` | `` |  |  | fixed_on_pin |
| 1 | pass/run regression | view formation has incomplete permission suppliers at its captured source | `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit` | `` |  |  | propagated |
| 1 | pass/run regression (fixed) | (built at PIN) | `pass/facts/target_family_callback_guarantee/local` | `` |  |  | fixed_on_pin |
| 1 | pass/run regression | trait `Reading` conformance borrow contract: callable input does not preserve captured region identi | `pass/traits/applied_conformance_captured_fresh_regions_exit` | `` |  |  | propagated |

## Members

### c23_dependent_callback (7 rows)
- `pass/borrows/runtime_dependent_callback_window_byte_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/borrows/runtime_dependent_forwarded_window_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/borrows/runtime_dependent_window_byte_payload_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/borrows/runtime_dependent_window_image_sibling_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/borrows/runtime_dependent_window_region_disjoint_write_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/generics/runtime_dependent_callback_renamed_formals_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==
- `pass/generics/runtime_dependent_window_parameter_index_exit` — broken — obs `rejected` — cannot prove requires contract for call receive_inferred from main: Count == 7 (needs fact `Count ==

### c63_lifetime_carrier (3 rows)
- `fail/borrows/replaced_carrier_keeps_new_backing_loan` — fixed — obs `rejected` — machine `reject_live_write` state `entry` expression type application uses undeclared lifetime argum
- `pass/borrows/carried_loan_replacement` — broken — obs `rejected` — machine `Main::main` state `main` expression type application uses undeclared lifetime argument `'st
- `pass/ownership/runtime_carried_loan_field_replacement_exit` — broken — obs `rejected` — machine `Main::main` state `main` expression type application uses undeclared lifetime argument `'st

### c103_division_by_zero (1 rows)
- `fail/arithmetic/indexed_divisor_copied_selector_storage_write` — fixed — obs `rejected` — division by zero in machine `quotient` state `entry` return value: the divisor is provably zero, whi

### c143_view_permission (1 rows)
- `pass/borrow/runtime_write_only_exclusive_reference_nested_call_operand_exit` — broken — obs `rejected` — view formation has incomplete permission suppliers at its captured source

### c183_operator_selection (1 rows)
- `pass/facts/target_family_callback_guarantee/local` — fixed — obs `built` — 

### c223_trait_reading (1 rows)
- `pass/traits/applied_conformance_captured_fresh_regions_exit` — broken — obs `rejected` — machine `Reader::read` state `read` does not satisfy trait `Reading` machine `read`: conformance bor

## Fixed-on-main members (cluster-level)

- **c63_lifetime_carrier**: `fail/borrows/replaced_carrier_keeps_new_backing_loan`
- **c103_division_by_zero**: `fail/arithmetic/indexed_divisor_copied_selector_storage_write`
- **c183_operator_selection**: `pass/facts/target_family_callback_guarantee/local`

## Commands

- Host: linux x86_64
- Jobs: `OMEGA_CORPUS_JOBS=1`; macOS `check-target` batches use `OMEGA_CORPUS_NATIVE=1` + `OMEGA_CORPUS_NATIVE_TARGET=macos_arm64`
- PIN ledger copy: `/tmp/ledger_bisect_part23_corpus_outcomes.txt`
- Direct runner: `.agents/ledger-bisect-2026-11-11/measure_fixtures.sh`
- Bisect: `.agents/ledger-bisect-2026-11-11/run_cluster_bisect.sh` with fixed PIN witness specs


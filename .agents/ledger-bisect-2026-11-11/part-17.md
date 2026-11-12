# Ledger bisect part 17 (k=17, N=40)

Measured `main` at `51e8c45c793007f211d0d9baee7de16767172444` (PIN). Prior-report slice: 17 member rows across 6 clusters (`index % 40 == 17`). Re-measured on PIN via direct `corpus_runner` (`OMEGA_CORPUS_RECORDS`); reclustered still-broken rows by first diagnostic at PIN. Bisect uses fixed copies of PIN `corpus_outcomes.txt` and witness observation in `/tmp`, skips ledger-only first-parent commits (exit 125), good commit found by walking first-parent until exit 0.

## Slice summary

| | Count |
| --- | ---: |
| Prior slice rows | 17 |
| Fixed on PIN | 5 |
| Still broken | 12 |

## Clusters (still broken on PIN, reclustered)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 9 | pass/run rejected | machine-parameter requirement `Observe` requires contract constant expression call argument `kept` does not form parameter `kept` of `premise`: actual T, required T | `pass/facts/abstract_signature_call_generic_actual_through_call_result_exit` | `29f25c7de497816b5e2d0833e033c8161c3f0de6` | 1291 | #1291 representation-agreement checking rejects the abstract-signature generic call actual before it forms the `premise` parameter | bisected |
| 3 | pass/run rejected | `AddressBridgeProvider::classified_make` has no executable body (authored result domains) | `pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit` | `60cb5e1f79dd05465f77775990e46bcc2e789470` | 1721 | #1721 safe-array-write / function-selection refactor leaves authored result-domain declarations without a lowered body for `classified_make` | bisected |

### Members (cluster c0, 9 rows)

- `pass/facts/abstract_signature_call_generic_actual_through_call_result_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_const_name_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_field_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_local_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_match_join_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_other_machine_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_other_state_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_parameter_exit`
- `pass/facts/abstract_signature_call_generic_actual_through_target_body_exit`

### Members (cluster c1, 3 rows)

- `pass/facts/opaque_independent_qualified_result_import_alias_exit`
- `pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit`
- `pass/facts/opaque_independent_qualified_result_through_local_exit`

### Fixed on PIN (5 rows)

- `fail/facts/selected_value_application_through_field_rejected`
- `fail/traits/evidence_binder_given_data_type`
- `pass/facts/resident_backing_plain_ram_exit`
- `pass/facts/selected_value_application_through_field_exit`
- `pass/recast/returned_same_type_view_capture_exit`

## Bisect notes

- **Good commits:** `73c70115c79` (#960, abstract signature witness) and `60ee163d4f0f` (opaque witness); both verified exit 0 in triplicate before bisect. PIN witnesses exit 1 in triplicate (deterministic).
- **Methodology:** first bad is before PIN (`29f25c7…` ≈ 743 merges before PIN; `60cb5e1f…` ≈ 590 before PIN), satisfying sanity vs `741270df60f7`.
- **Opaque merge-base warning:** git bisect reported uncertain merge base between good and PIN; first bad `60cb5e1f79dd` still returned by `git bisect run` (exit 0).

## Commands

- Re-measure: `OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER=<csv> OMEGA_CORPUS_RECORDS=… target/debug/build/compiler/*/out/corpus_runner-*`
- Bisect: `.agents/ledger-bisect-2026-11-11/tools/prepare_witness_pin.sh <witness> .agents/ledger-bisect-2026-11-11/part17_pin_records.txt` then `git bisect start <PIN> <GOOD> --first-parent` + `git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh`
- Host: linux x86_64 cloud agent; jobs: 1

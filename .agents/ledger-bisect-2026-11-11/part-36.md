# Ledger bisect part 36 (k=36, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`.

Prior-report cluster indices in this slice: 36, 76, 116, 156, 196 (10 member rows total; no `(+N more)` truncation).

## Summary

| | Count |
| --- | ---: |
| Prior-report rows in slice | 10 |
| Fixed on PIN | 4 |
| Still broken on PIN | 6 |
| PIN clusters after re-group | 2 |
| Bisected | 2 |
| Propagated | 0 |

Bisect uses fixed `/tmp/ledger-bisect-pin-corpus_outcomes.txt` and `/tmp/ledger-bisect-pin-meta.json` (PIN observation), skips first-parent commits that touch only `tests/omega/corpus_outcomes.txt` or `*.md`.

## Clusters still broken on PIN

| Rows | Kind | First diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 5 | pass/run rejected | cannot form specification index … independent collection bounds are not established by preceding formed facts | `pass/modules/selected_constant_array_alias_computed_extent_exit` | `5746c155a30e40702a0e53c7d3674c659cdeb429` | 824 | Psi collection-bound / specification-index formation no longer establishes independent bounds used by module array-alias requires contracts | bisected |
| 1 | pass/run rejected | native artifact identity physical pipeline failed … DescriptorCallOrigins … descriptor result origin could not be substituted at the call | `pass/borrows/runtime_shared_view_record_presentation_exit` | `5c6c16d4d1867bcac99343285c5074169d7cbfbc` | 1518 | Write-only reference store / storage-address custody changes leave descriptor call-result origins unsubstitutable at native legalization | bisected |

### Cluster `spec_index` members

- `pass/modules/selected_constant_array_alias_computed_extent_exit` still broken
- `pass/modules/selected_constant_array_alias_match_qualification` still broken
- `pass/modules/selected_constant_array_alias_named_extent_exit` still broken
- `pass/modules/selected_constant_array_alias_qualification` still broken
- `pass/slices/state_projected_selector_bounds` still broken

### Cluster `native_descriptor_origins` members

- `pass/borrows/runtime_shared_view_record_presentation_exit` still broken

## Fixed on PIN (4 rows; prior-report clusters 76, 116, 156)

- `pass/facts/target_family_contract_operand/other_machine` fixed on main (ledger expects checked; PIN: checked)
- `pass/facts/target_family_contract_operand/other_state` fixed on main
- `pass/facts/target_family_contract_operand/parameter` fixed on main
- `pass/slices/saved_byte_predicate_operand_disjoint_exit` fixed on main

## Bisect sanity

- Spec-index witness at PIN: exit 1 (bad) ×3 deterministic; at good `af27094c67b`: exit 0 ×3.
- Native witness at PIN: exit 1 ×3; at good `fedeee648fc`: exit 0 ×3.
- First-bad commits are compiler changes (#824, #1518), not ledger-only rows (e.g. not `f3612364` / #2228).

## Commands

```bash
cd /agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
# re-measure slice (direct corpus_runner, jobs=1)
FILTER='pass/modules/selected_constant_array_alias_computed_extent_exit,pass/modules/selected_constant_array_alias_match_qualification,pass/modules/selected_constant_array_alias_named_extent_exit,pass/modules/selected_constant_array_alias_qualification,pass/slices/state_projected_selector_bounds,pass/facts/target_family_contract_operand/other_machine,pass/facts/target_family_contract_operand/other_state,pass/facts/target_family_contract_operand/parameter,pass/slices/saved_byte_predicate_operand_disjoint_exit,pass/borrows/runtime_shared_view_record_presentation_exit'
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_JOBS=1
export OMEGA_CORPUS_FIXTURE_FILTER=$FILTER OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/part-36-records.txt
bash .agents/ledger-bisect-2026-11-11/tools/bisect_run_fixture.sh
# spec-index bisect
export BISECT_WITNESS=pass/modules/selected_constant_array_alias_computed_extent_exit
bash .agents/ledger-bisect-2026-11-11/tools/pin_witness_setup.sh "$BISECT_WITNESS" "$PIN"
git bisect start --first-parent "$PIN" af27094c67b23c939e3569495a37deaf8a2132bb
git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_step.sh
# native bisect
export BISECT_WITNESS=pass/borrows/runtime_shared_view_record_presentation_exit
bash .agents/ledger-bisect-2026-11-11/tools/pin_witness_setup.sh "$BISECT_WITNESS" "$PIN"
git bisect start --first-parent "$PIN" fedeee648fc
git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_step.sh
```

Host: linux cloud agent VM. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`.

Branch head after this report: `work/ledger-bisect-part-36` at PIN plus `.agents/ledger-bisect-2026-11-11/`.

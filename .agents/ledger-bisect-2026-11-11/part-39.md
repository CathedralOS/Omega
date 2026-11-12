# Ledger bisect part 39 (k=39, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`.

Prior-report slice: cluster indices `39, 79, 119, 159, 199` (5 clusters, 9 member rows; all member paths listed in the 2026-11-11 report — none used `(+N more)`). Re-clustered at PIN by first diagnostic into **3** still-broken clusters, **5** rows; **4** fixed.

## Summary at PIN

| | Count |
| --- | ---: |
| Still broken | 3 clusters, 5 rows |
| Fixed on main | 4 rows |

## Clusters (still broken)

| id | rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | bisect |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| blockexec-suspend-ack | 2 | pass | `execute` has operational envelope `suspend block` but acknowledges neither suspension nor blocking | `pass/blockexec/blocking_executor_custody_claims_compile` | `2ab64136c02d7879635a92e3e078d7374bec7590` | #1811 | #1811 refactors terminal call argument preparation (`ComputationCallArguments` + canonical access roster) so blocking `execute` sites no longer carry matching suspend/block acknowledgements | bisected |
| opaque-named-conformance | 2 | pass | opaque representation `…::AddressRepresentation` does not resolve to one visible named conformance | `pass/facts/opaque_independent_qualified_input_import_alias_exit` | `60cb5e1f79dd05465f77775990e46bcc2e789470` | #1721 | #1721 changes selected-call/dynamic forwarding so independent opaque import aliases no longer resolve to a single visible named conformance | bisected |
| ownership-unwrap-lowering | 1 | pass | `main` calls `unwrap`; `unwrap` has no executable body (operand custody on moved match payload) | `pass/ownership/runtime_match_expression_moves_selected_payload_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | #2078 | #2078 case/common-field match preservation leaves the moved-match payload on a path that declines lowering `unwrap` at operand custody | bisected |

### Members (still broken)

- `pass/blockexec/blocking_executor_custody_claims_compile` rejected
- `pass/blockexec/runtime_selected_worker_body_exit` rejected
- `pass/facts/opaque_independent_qualified_input_import_alias_exit` rejected
- `pass/facts/opaque_independent_qualified_input_renumbered_catalog_exit` rejected
- `pass/ownership/runtime_match_expression_moves_selected_payload_exit` incomplete

### Fixed on main (slice members)

- `pass/dependent/runtime_symbolic_slice_predicate_store_exit`
- `pass/facts/target_family_callback_guarantee/local`
- `pass/layouts/placed_reference_results/nested`
- `pass/generics/runtime_captured_map_borrowed_type` (matches ledger `rejected`)

## Methodology

- Fixed `/tmp/ledger_bisect_pin_corpus_outcomes.txt` + per-witness baseline JSON; `bisect_pin_test.py` uses PIN expected status (not checkout ledger).
- `bisect_witness.sh` skips ledger-only first-parent commits (`corpus_outcomes.txt` / `*.md` only); exit `125` on build failure.
- Good commits walked from PIN: blockexec `7a7ea313e7c…` (0), opaque `c64728d90e83…` (0), ownership `8d8506a6bca…` (0). PIN witnesses exit `1`. Breaking commits all **strict ancestors of** `741270df60f7`.
- Blockexec anchors deterministic (`7a7ea`→0, `0aeed`/`PIN`→1 on repeated runs).

## Commands

- Re-measure: direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`, filter of 9 fixtures.
- Bisect: `git bisect start --first-parent <bad-PIN> <good>`; `git bisect run bash /tmp/ledger-bisect-tools/bisect_witness.sh`.
- Host: cloud VM; `CARGO_BUILD_JOBS=4`.

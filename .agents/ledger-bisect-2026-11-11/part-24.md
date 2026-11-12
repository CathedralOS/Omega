# Ledger bisect part 24 (k=24, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`. Slice: prior-report cluster indices 24, 64, 104, 144, 184, 224 (6 legacy groups; 14 member rows).

## Summary (PIN re-measure)

| Class | Count |
| --- | ---: |
| fixed on main | 12 |
| still broken | 2 |

## Clusters still broken (regrouped by first diagnostic at PIN)

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass/run rejected | `repair` has no executable body (structural operand custody assignment) | `pass/facts/recoverable_replacement_window_through_match_join_exit` | `9f1fa7f344fc41cd4cae8821560bb628d981bb82` | 2189 | #2189 seeds typed-tree lowering with resolved symbols at start instead of at `finish`, and the witness’s `repair` structural-assignment body stops lowering afterward | bisected |
| 1 | pass/run rejected | conformance borrow contract: captured region identity | `pass/traits/applied_conformance_captured_fresh_regions_exit` | (pending) | | | in progress |

## Cluster 0 — recoverable replacement window (bisected)

**PIN observation:** `incomplete` — `` `main` calls `choose`, which calls `prepare`, which calls `repair`; `repair` has no executable body: this compiler does not lower a statement of it yet (statement sequence: assignment: structural operand custody); in state `repair::entry`, statement 2 ``

**Good commit (walk):** `3ec75ded8c556fb49a66f5df8e21d7c2706a3052` (`checked`). Younger good on first-parent: `20fd485ca46` also `checked`.

**Breaking commit:** `9f1fa7f344fc41cd4cae8821560bb628d981bb82` — psi: recognize imported types when checking build programs (#2189)

**Bisect:** PIN-fixed `corpus_outcomes.txt` + manifest under `/tmp/ledger-bisect-fixture/`; forward first-parent binary search between `3ec75ded8c5` and `9f1fa7f344f` (9 commits). PIN and `9f1fa7f344f` reproduced bad outcome 3× and 1×; `3ec75ded8c5` good 3×. Deterministic.

**Members (legacy slice row status at PIN):**

- `pass/facts/recoverable_replacement_window_through_match_join_exit` — broken
- All other rows from prior clusters 24/64/144/224 fixed (see below)

## Fixed on main (12 rows)

- `fail/arithmetic/bounded_store_consumed_before_repair`
- `fail/dependent/data_requires_cyclic_handoff_store_rejected`
- `fail/machines/private_frontier_guarded_counter_cycle_invalid`
- `fail/machines/private_where_store_through_cycle_control`
- `fail/time/instant_mutation_leaves_invalid_return`
- `pass/recast/runtime_mutable_equivalent_range_recast_exit`
- `pass/recast/runtime_mutable_equivalent_record_recast_exit`
- `fail/float/public_class_contract_ieee_equality_retains_zero_sign`
- `fail/float/public_class_result_contract_retains_actual`
- `fail/float/public_class_state_contract_retains_entry_subject`
- `fail/facts/slice_view_element_read_bound_through_other_state_rejected`
- `fail/termination/retired_block_form`

## Commands

- Host: linux/x86_64, `OMEGA_CORPUS_JOBS=1`, `CARGO_TARGET_DIR=/tmp/omega-ledger-bisect-target`
- Re-measure: direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER` (14 fixtures), records → `/tmp/part24_pin_records.txt`
- Bisect helper: `/tmp/ledger-bisect-tools/bisect_witness.sh` + `/tmp/ledger-bisect-tools/prepare_fixture.py`

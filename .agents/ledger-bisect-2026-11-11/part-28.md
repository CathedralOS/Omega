# Ledger bisect part 28 (k=28, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

Prior-report slice: cluster indices 28, 68, 108, 148, 188 (12 member rows; no truncated `(+N more)` in those clusters).

## Summary

| | Count |
| --- | ---: |
| Slice member rows | 12 |
| Fixed on PIN (vs PIN ledger) | 11 |
| Still broken on PIN | 1 |
| Clusters after re-grouping at PIN | 1 |

Bisect methodology: fixed copies at `/tmp/ledger-bisect-pin/` (`corpus_outcomes.txt` + witness PIN record); ledger-only first-parent commits skipped (exit 125); good bound from `e43babaccd7` (walk verified); witness stability 3× exit 1 at PIN and 3× exit 0 at good (deterministic).

## Clusters (PIN first diagnostic)

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass | Exact integer cast in machine \`exit\` state \`entry\` from \`u64\` to \`i32\` is not provably representable | `pass/expressions/match_owned_computed_arm_arguments` | `29f25c7de497816b5e2d0833e033c8161c3f0de6` | 1291 | psi #1291 representation-agreement / structural-judgment tightening rejects the match-owned computed arm’s u64→i32 exact cast | bisected |

Members:
- `pass/expressions/match_owned_computed_arm_arguments`

## Fixed on PIN (11)

- `fail/facts/live_across_call_spill_through_other_state_rejected`
- `fail/facts/narrow_to_wide_sign_extension_through_other_state_rejected`
- `fail/facts/narrow_to_wide_zero_extension_through_other_state_rejected`
- `fail/facts/scalar_sign_zero_extension_through_other_state_rejected`
- `fail/facts/unsigned_exact_subtraction_interval_through_other_state_rejected`
- `fail/proofs/structural_application_premise_false_rejected`
- `fail/traits/evidence_binder_given_data_type`
- `pass/arithmetic/modular_power_state_bounds_exit`
- `pass/float/float_range_field_storage_exit`
- `pass/generics/runtime_const_extent_origin_exit`
- `pass/host/runtime_semantic_gui_reach_exit`

## Prior-report clusters covered (all members now fixed except cluster idx 28’s pass regression)

| prior idx | rows@741270df | at PIN |
| ---: | ---: | --- |
| 28 | 6 | 5 fail siblings fixed; witness still broken (re-grouped) |
| 68 | 3 | all fixed |
| 108 | 1 | fixed |
| 148 | 1 | fixed |
| 188 | 1 | fixed |

## Commands

- Re-measure: `OMEGA_CORPUS_JOBS=1` direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER` (12 fixtures) → `.agents/ledger-bisect-2026-11-11/pin_measure.txt`
- Bisect: `git bisect start --first-parent 51e8c45c793007f211d0d9baee7de16767172444 e43babaccd7` + `/tmp/ledger-bisect/bisect_witness.sh` (log: `bisect.log`)
- Host: linux x86_64 cloud VM; jobs: 1

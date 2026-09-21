# CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION — z70 scoped re-verification

**Revision:** `59e0b5ec22` (origin/main tip, linux x86_64 host).
**Command:** `cargo nextest run -p checked-trees-to-lowered-psi -E 'test(~ranked_countdown) or test(~ranked_u64_countdown) or test(~effectful_discarded_call_writes_before_return) or test(~scalar_array_source) or test(~closed_record_projections_replay)'` — 77 matched, **72 passed, 5 failed**; every failure lands on an already-owned family at an identical signature. No unattributed drift since the `23338b3d093c` reading.

## Family-by-family

| Family (ledger) | Members sampled | Result at 59e0b5ec22 |
| --- | --- | --- |
| Ranked safe-point segment bounds (closed — terminal-fixed-fuel bounded-walk series `0d0f85459ad`…`94e764a6da6`) | `ranked_countdown_lowers_to_verified_resumable_interpreter_execution`, `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64` | **PASS / PASS** — repair holds |
| Closed-projection replay admission (closed at `7af30a1f839a`) | `closed_record_projections_replay_exact_sources_carriers_and_all_siblings` | **PASS** — holds |
| Scalar-return custody (C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES, fenced) | `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel` | FAIL — identical `Lowering(Unsupported("composed Unit scalar call requires structural call custody"))` at `tests/owned_record_return_source.rs:306` |
| `scalar_array_source::cyclic` (scalar-graph/LICM lane, surfaced at `e7c0099cb2b7`) | all 4 cyclic members | FAIL ×4 — identical `index out of bounds: len is 1 but index is 3` at `src/scalar_graph/scalar_graph_lowering/call_lowering.rs:419` |
| `scalar_array_source` non-cyclic remainder | 73 other members in filter | PASS — the cyclic family stays the only red in that suite |

## Verdict

The row's "no independent slice" adjudication stands. Both recorded residuals
are still owned by live lanes (scalar-return custody; scalar-graph/LICM); both
recorded closures still hold. The ledger doc itself
(`wiki/drafts/known_baseline_failures.md`) remains fenced to its attribution
owners — nothing here changes what it already records.

TASKS.md carries a two-line stamp pointing here; this file is the ledger.

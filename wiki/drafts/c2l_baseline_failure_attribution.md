# checked-trees-to-lowered-psi baseline-failure attribution

Fresh sweep for the `## checked-trees-to-lowered-psi` section of
[known_baseline_failures.md](known_baseline_failures.md), held here while that
file is leased. Recorded at revision `de81c972d4` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable).

```bash
cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast
```

2160 run: 2100 passed, 59 failed, 1 still executing when the run was cut
(`nominal_affine_source::integer_comparison::
mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
— unbounded at >1560s inside `lower_machine`; the nominal integer-comparison
convergence search is not converging at this revision. That is a runaway, not
a slow pass — attribute as its own row below).

## Family A — un-migrated bare-boundary-trait fixtures (30)

Signature: source checking rejects with `field 'console'/'output' on data
'Main' names bare boundary trait '<Trait>' in value position; the intrinsic
'Service<R>' carrier is the only service value spelling` (thrown from
`src/tests.rs:82`'s shared `check` helper). This is the deliberate
`32f5182254` cut (`psi: reject bare boundary-trait value spellings at source
checking`) — these in-src fixtures still spell `console: Console` /
`output: Output` instead of `Service<…>` and were skipped by the corpus
migration because they live inside the crate's own test modules.

Members (30 rows, all panicking through `src/tests.rs:82`'s shared `check`
helper): `tests::composed_operand_catalogs::*` (16: eleven `closed_sum_*`,
`dynamic_continuation_operands`, `dynamic_result_continuation`, both
`dynamic_unit` rows), `tests::dynamic_composed_unit::*` (7),
`tests::structural_control_cases::lowers_closed_guard_and_provider_attachment`
+ `provider_attachment_and_ordinary_state_locals` (2),
`tests::composed_unit_nested_control` (1), `tests::indexed_primitive_storage`
(1), `tests::attached_unit_cases::attached_unit_borrowed_self` (1), plus two
more in-src rows carrying the same diagnostic.

Owner: the Service<R>-carrier migration lane (TASKS.md:910 bullet — "Migrate
library, samples, canaries and Squalr from bare boundary-trait fields…"; its
recorded residual is exactly raw/in-crate fixtures where `Service` cannot
resolve without injecting `service.omg` or re-spelling onto
`&'s mut <boundary>` receivers). **Fixture debt, not a compiler bug.**

## Family B — attached-Unit transitive plan closure (16)

Signature: `Lowering(InvalidUnitMachinePlan { reason: "attached Unit closure
is missing a checked transitive machine plan", omission: "'<machine>' has no
admitted body (local construction stopped at signature / state graph: …)" })`.

Members: `suite provider_attachment_source::*` (6),
`suite unit_state_graph::provider_attachments::*` (9 — the "untampered
provider graph must lower before negative controls" precondition),
`suite guarded_scalar_returns_source::
stored_returned_cases_support_borrowed_refined_getters` (1).

Owner: the missing transitive-Unit-plan class — **GENERAL-CYCLIC-EXECUTION**
and the state-graph route (the same attribution the main doc already gives
this signature).

## Family C — operation proof-machine calls (3)

Signature: `Unsupported("crash predicate value position is outside the
selected scalar namespace")`.

Members: `suite exact_affine_sibling_source::
landed_affine_sibling_custody_crosses_source_codec_and_independent_verification`,
`suite exact_shift_left_certificate_source::
bounded_exact_left_shift_uses_only_its_canonical_certificate`,
`suite mixed_shift_source::
erased_arithmetic_prefix_still_requires_its_own_certificate`.

Owner: the proof-machine scalar-namespace lane — crash predicates issued
through proof machines are positioned outside the selected scalar namespace.

## Family D — scalar return custody / unit result transfer (4)

Members, `suite owned_record_return_source::*`:
`discarded_scalar_invocation_precedes_whole_owned_return` ("ordered body
retains a structural result independently of preceding scalar calls"),
`effectful_discarded_call_writes_before_return_across_fuel`
(`Unsupported("composed Unit scalar call requires structural call
custody")`),
`source_replay_rejects_return_parameter_and_carrier_substitution`
(`Option::unwrap()` on `None` in the replay path — an evidence-lookup
robustness gap, not a soft rejection),
`source_replay_requires_the_exact_affine_return_transfer` ("Record: changed
return transfer 0").

Owner: unit-result custody lane; the unwrap is the row to fix first — replay
should reject with a diagnostic, not panic.

## Family E — safe-point schedule / interpreter rows (2)

`tests::structural_control_cases::ranked_countdown_lowers_to_verified_
resumable_interpreter_execution` — `assertion left == right` on the
`(BlockId, EdgeId, fuel)` schedule rows (expected `(BlockId(1),EdgeId(1),1),
(BlockId(2),EdgeId(2),25769803776), …`; per-edge fuel values drifted to the
u64-bound 25769803776 pattern).
`tests::structural_control_cases::ranked_u64_countdown_fails_closed_when_
fixed_fuel_exceeds_u64` — `BoundOverflow` from the per-edge safe-point
u64 schedule.

Owner: the fixed-fuel/safe-point scheduling lane.

## Family F — unbounded convergence (1, running)

`nominal_affine_source::integer_comparison::
mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`:
>1560s inside `lower_machine` at the cut. Owner: the nominal integer-
comparison convergence lane — the shared-cleanup-return search does not
terminate at this revision.

## Singular rows

- `expression_preparation::bindings::tests::
  closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
  ("invalid or foreign field cannot disable replay") — record-projection
  replay accepts a foreign/invalid field that should disable it.
- `suite unit_plan_omissions::*` (3 — the "a_provider_carrying_argument…",
  "a_routed_task_result…", "a_routed_task_start_call…" rows) panic while
  printing a `data Task<T> [linear]` source block — expectation mismatch in
  the plan-omission witness output, needs one look at the assertion to place.

## Net delta vs the prior record

The prior row (9d0d864656/51f21bb168) said "2032 run, 20 failed… group as
boundary byte buffers, scalar-return pure source custody, the
ordered-boolean guarantees, crash-member byte entries, provider attachment
and results, and one attached-unit borrowed-self case." At `de81c972d4` the
set is 59+1: the ordered-boolean `OperationProofUnavailable` row is fixed;
the *new dominant* family is A (30 un-migrated bare-trait fixtures —
regression introduced by 32f5182254's deliberate rejection landing while its
in-crate fixtures kept the old spelling); family B (16) is the standing
GENERAL-CYCLIC-EXECUTION unit-plan residual; C/D/E/F and the singular rows
cover the remaining ~13.

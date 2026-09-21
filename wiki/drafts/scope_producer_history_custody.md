# Scope: PRODUCER-HISTORY-CUSTODY

Scope-verification record for the dispatched name `PRODUCER-HISTORY-CUSTODY`
(board stub at TASKS.md:13159 — "mined candidate; verify scope then
implement"; the NEW-SV- prefixed planner item carries this document as its
deliverable). Audited at `3a82039327` on linux x86-64 (cargo; `mbx` absent
on this host).

## Resolution

The name maps to the producer-history custody invariant stated in
`omega-rust/pipeline.md` ("Keep producer history in explicit replay
evidence, not the path ordinary consumers walk to obtain current data") and
AGENTS.md ("Optimization history is explicit evidence; it must not select a
different downstream representation"). The surface is the set of lanes that
carry a producing stage's history forward and the consumers forbidden from
recovering current data out of them.

## Landed state

- `abstract-operations-to-abstract-operations/src/publication/model.rs`
  publishes `ValidatedOptimizedAbstractPlan { plan: Arc<AbstractOperationPlan>,
  evidence: AbstractOptimizationEvidence }` — current data on `plan`, the
  producer history sealed on `evidence`.
- `optimization-unit/src/optimization_unit/evidence.rs` holds
  `AbstractOptimizationEvidence` (`selections`, `psi_selections`,
  `budget_per_pass`, `commits`, `validated_candidates`, `usage`, `decisions`,
  `external_decisions`, `pass_manifests`, `transformation_ledger`,
  `identity_bundle`) — documented "the retained history needed to replay a
  published abstract program; current program data is not recovered by
  walking this history".
- `optimization-core/src/selection.rs` (~:572) records Psi selections as
  execution history in the sealed Terminal artifact.
- Downstream representations declare the same independence:
  `machine-code/src/machine_code/layout/program.rs` ("current inputs to
  fragment emission, independent of producer-stage history"),
  `selected-form-encoding-to-resolved-layout` ("independently admit retained
  current data, without a producer-stage history"),
  `packages/review/evidence/src/record/signatures/external_policy.rs`
  ("exact producer identity retained independently of its evaluation
  history"), and `selected-instructions/src/selected_instructions/
  functions.rs` (replay history per function without copying unrelated
  bodies).
- Consumers of `evidence()` are confined to the optimizer's own
  validation/projection modules and the differential custody tests
  (`tests/native-differential/tests/abstract_publication/decision_custody.rs`).

## Gates

`tests/architecture/layering.rs` pins the custody contract: the published
model must carry `plan` + `evidence` and must not retain `OptimizationRun`
or `Session`; run replay visibly owns the ordered custody steps
(`rule_set::rebuild`, `commits::replay`, `candidate_decisions::validate`,
`records::validate`, `validate_external_decision_recording`); the retired
flat Applied-only decision replay must not return; publication lives inside
abstract optimization (no separate `omega-optimization-run-to-*` stage); the
native-realization coordinator schedules `optimize_abstract_operations` but
may not name its private internals. The "report-only beside exact replay"
family (content fingerprints, UEFI layout fingerprint, external-root
summaries, const layout fingerprints) pins that summaries never substitute
for exact evidence.

## Open legs

None identified in this surface. The custody lanes are landed, the published
product splits current data from history, and the architecture gates hold at
the audit revision (the layer test roster cited above remains in force).
Residual risk is scope-shaped, not code-shaped: history lanes beyond the
optimizer (terminal-verifier replay, codec custody) answer to their own
boards.

## Slice verdict

No independent slice exists under this name. The invariant is landed and
gated; the stub should fold into the owning rows (or be retired) rather than
dispatch implementation work.

## Fences observed at audit time

Live claims adjacent to — but not covering — this document:

- `optimization-core/src/selection.rs`, `optimization-unit/src/
  optimization_unit`, and `abstract-operations-to-abstract-operations`
  rules/pass_manager/representation_specialization:
  REPRESENTATION-SPECIALIZATION (~17:59Z) — fences the evidence-type
  definitions if a code leg ever becomes available.
- `tests/native-differential/tests/abstract_publication/
  decision_custody.rs`: NATIVE-DIFF-CUSTODY-EXPECTATION-RETARGET (~17:17Z).
- `native-realization` slices (`native_product`, `behavior_exclusions`,
  `retained_native_product.rs`): BUILD-EXCLUSION-REALIZATION (~15:52Z).

`publication/model.rs`, `layering.rs`, and the downstream history-
independence declarations are unfenced; re-check `tools/claims.py status`
before any code leg.

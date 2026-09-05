# Abstract Operations Optimization

[Pipeline](../pipeline.md) | Previous: Terminal Psi To Abstract Operations | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This X-to-X phase optimizes verified abstract operations and publishes their
current program with independently checked evidence. Publication is part of
the optimizer, not a separate run-to-program pipeline stage.

## Stage Contract

Input: verified abstract operations, exact selections and their Psi projection,
and the per-pass work budget.

Output: one `ValidatedOptimizedAbstractPlan` holding the current
`Arc<AbstractOperationPlan>`, explicit replay inputs, immutable optimization
evidence and validation receipts. Ordinary consumers read `plan()` or retain
`shared_program()`; neither requires walking producer history.

Primary responsibility: replay and validate the accepted optimizer candidates,
reconstruct their final abstract-operation projection, and keep that projection
inseparable from its validation receipt.

## Implementation Map

- `abstract-operations-to-abstract-operations/src/phase.rs` owns verified-unit
  construction, selected pass execution and publication. The native coordinator
  calls this phase once.
- Its `src/publication/mod.rs` owns the
  ordered selection-projection, run-replay, source-projection, independent
  validation, and pre-physical-manifest join.
- `src/publication/replay/mod.rs` owns catalog-derived schedule reconstruction followed by
  commit replay, Applied-decision custody, ledger/usage replay, and external
  policy-mirror validation. `rule_set.rs`, `commits.rs`,
  `candidate_decisions/`, and `records.rs` own those mechanics.
- Applied custody binds each selected pass and complete rule contract, the
  independently replayed declaration, manifest analyses and facts, and the
  baseline predicted cost. Coordinated manifest and external-log rewrites do
  not create publication evidence.
- `src/publication/source/mod.rs` owns active/pruned function-roster custody before
  `source/function.rs` projects parameters, block offsets, and operations.
- `src/publication/model.rs` and `error.rs` own the validated output and typed
  failures. Publication consumes the completed run and discards its executing
  session and analysis cache. Immutable records live in
  `optimization-unit/src/evidence.rs`.
- `optimization-validation` owns independent candidate and projection
  validation.
- `tests/native-differential/tests/abstract_publication/manifests/mod.rs` is
  the pre-physical V6 custody-test entrance.
  Its named leaves separate the reusable optimized/donor fixture, positive and
  multipass behavior, all 35 mutable logical-field mutations, dynamic wire-
  offset reconstruction, and 16 exact wire rejection axes. Singleton Rust
  enums are corrupted only through their closed wire tags.
- `abstract-operations-to-target-operations/src/optimized.rs` owns the
  later custody-preserving join to target operations; it is intentionally not
  part of this stage.

## Ownership Rules

- Must independently replay the optimizer's final unit before publication.
- Current program data may outlive the publication wrapper; detaching data
  does not grant the validation authority held by that wrapper.
- Must not select a native target, perform target legalization, install a
  provider, allocate registers, or claim native-publication authority.
- Must preserve the exact Terminal-Psi root, selections, decision log,
  transformation ledger, pass manifests, and independent validation identity.
- Target-aware custody belongs to the target-lowering stage that constructs and
  independently validates the target projection. The coordinator only sequences
  that typed stage result.

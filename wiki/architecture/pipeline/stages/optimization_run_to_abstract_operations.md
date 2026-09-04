# Optimization Run To Abstract Operations

[Pipeline](../pipeline.md) | Previous: Terminal Psi To Abstract Operations | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This stage projects one completed, independently validated optimization run
back into executable abstract-operation shape. It is a representation
transformation, not an optimization policy owner or target-realization stage.

## Stage Contract

Input: one completed `OptimizationRun` retaining its verified Terminal-Psi
input, selections, candidates, decisions, manifests, and transformation ledger.

Output: one `ValidatedOptimizedAbstractPlan` retaining both the projected
`AbstractOperationPlan` and the evidence that authorized that projection.

Primary responsibility: replay and validate the accepted optimizer candidates,
reconstruct their final abstract-operation projection, and keep that projection
inseparable from its validation receipt.

## Implementation Map

- `omega-optimization-run-to-abstract-operations/src/lib.rs` visibly owns the
  ordered selection-projection, run-replay, source-projection, independent
  validation, and pre-physical-manifest join.
- `src/replay/mod.rs` owns catalog-derived schedule reconstruction followed by
  commit replay, Applied-decision custody, ledger/usage replay, and external
  policy-mirror validation. `rule_set.rs`, `commits.rs`,
  `applied_decisions.rs`, and `records.rs` own those mechanics.
- Applied custody binds each selected pass and complete rule contract, the
  independently replayed declaration, manifest analyses and facts, and the
  baseline predicted cost. Coordinated manifest and external-log rewrites do
  not create publication evidence.
- `src/source/mod.rs` owns active/pruned function-roster custody before
  `source/function.rs` projects parameters, block offsets, and operations.
- `src/model.rs` and `src/error.rs` own the retained carrier and typed failure
  axes. The former mixed `projection.rs` catchall is retired.
- `omega-optimization-validation` owns independent candidate and projection
  validation.
- `src/tests/manifests/mod.rs` is the pre-physical V6 custody-test entrance.
  Its named leaves separate the reusable optimized/donor fixture, positive and
  multipass behavior, all 35 mutable logical-field mutations, dynamic wire-
  offset reconstruction, and 16 exact wire rejection axes. Singleton Rust
  enums are corrupted only through their closed wire tags.
- `omega-abstract-operations-to-target-operations/src/optimized.rs` owns the
  later custody-preserving join to target operations; it is intentionally not
  part of this stage.

## Ownership Rules

- Must not trust or detach the optimizer's final mutable unit.
- Must not select a native target, perform target legalization, install a
  provider, allocate registers, or claim native-publication authority.
- Must preserve the exact Terminal-Psi root, selections, decision log,
  transformation ledger, pass manifests, and independent validation identity.
- Target-aware custody belongs to the target-lowering stage that constructs and
  independently validates the target projection. The coordinator only sequences
  that typed stage result.

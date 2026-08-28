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

- `omega-optimization-run-to-abstract-operations/src/lib.rs` owns the exact
  run-to-abstract transformation and its custody carrier.
- `omega-optimization-validation` owns independent candidate and projection
  validation.
- `omega-optimization-pipeline/src/stages/selection/optimized_target_operations.rs` owns the
  later orchestration join to target operations; it is intentionally not part
  of this stage.

## Ownership Rules

- Must not trust or detach the optimizer's final mutable unit.
- Must not select a native target, perform target legalization, install a
  provider, allocate registers, or claim native-publication authority.
- Must preserve the exact Terminal-Psi root, selections, decision log,
  transformation ledger, pass manifests, and independent validation identity.
- Target-aware custody belongs to orchestration because it joins distinct
  pipeline stages and deployment evidence.

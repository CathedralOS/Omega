# Allocation Legality To Fixed-View Copies

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in stage implements one exact named selected-CFG transformation:
`LeafLocalBeforeFixedUseV1`. It materializes an incompatible ABI entry view and
leaf return-use view as an explicit target-owned `CopyI64` instruction. It is
not copy propagation, a broad optimization level, or general interval
splitting.

## Stage Contract

Input: `StagedOptimizedAllocationLegality`, the exact target register
environment including its selected `copy_i64` key, the named policy, and an
explicit nonzero `OptimizationWorkBudget`.

Output: `StagedOptimizedFixedViewCopies`, retaining the complete source
legality chain plus an opaque independently validated transformation artifact.
The artifact binds source selected/range/legality identities, target register
environment, policy, budget and exact usage, canonical copy rows, the complete
transformed selected plan, and its content identity.

Authority: insert one leaf-local scalar-u64 copy immediately before each exact
incompatible fixed return Use, create a fresh dense VReg and instruction ID,
and rewrite only that return operand. It grants no other CFG rewrite,
allocation, spill, frame, emission, or publication authority.

## Exact V1 Policy

The producer first counts the complete required work and rejects if any budget
axis is insufficient; it never returns a partial plan. Each admitted transition
must originate at a scalar-u64 entry VReg, terminate at a non-entry leaf
`ReturnI64` Use, and name the exact source and destination views from validated
legality. Duplicate sites or any other shape reject the whole transformation.

Original instruction IDs and all original instructions, blocks, successors,
and terminators remain stable. Fresh instruction IDs append to the dense ID set
and fresh VReg IDs append to the dense register set. Each copy uses the exact
ISA-owned `copy_i64` constraint row. Its provenance names the original semantic
source value but has no operations, edges, obligations, or logical fuel. Return
provenance and fuel remain byte-for-byte unchanged; a native register move is
not a second Psi logical event.

The current accounting charges one rule evaluation per function, and one
candidate, validation step, and commit per transition, plus one whole-plan
iteration. Zero-transition input produces a canonical zero-copy artifact whose
transformed selected identity equals its source identity.

## Independent Validation And Gaps

Replay independently enumerates source legality requirements, validates the
copy constraint shape, reconstructs every fresh ID, VReg, copy instruction,
operand rewrite, provenance row, work-usage field, and the complete expected
selected plan, then recomputes a domain-separated transformation identity.

Only the admitted entry-to-leaf fixed Use is supported. Hoisting, arbitrary
fixed-use sites, calls, pressure splitting, stable-address values,
rematerialization, coalescing, spills, and frames remain unsupported.

## Implementation Map

- `pipeline/optimization/omega-regalloc/src/fixed_view_copy_*` owns the artifact,
  producer, independent replay, work-budget gate, and identity.
- `pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/fixed_view_copies.rs` owns
  nested source-to-transformation custody.
- ISA catalogs own the exact `CopyI64` rows; the target register environment
  binds the selected row key.

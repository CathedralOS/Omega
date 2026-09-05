# Live Ranges To Allocation Legality

This is an internal contract of [Selected Instructions To Register Homes](selected_instructions_to_register_homes.md), not a standalone pipeline crate.

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in analysis joins exact CFG-aware live ranges with one identity-bound
target register environment. It determines where physical views are legal and
where incompatible fixed views require a later explicit transition. It does
not assign homes.

## Stage Contract

Input: `StagedOptimizedLiveRanges`, including the retained selected CFG,
liveness, range receipt, physical model, instruction catalog, active
reservation profile, and joined environment identity.

Output: `StagedOptimizedAllocationLegality`, nesting the complete input plus an
opaque validated legality plan. Its receipt binds every upstream identity,
the allocation-legality identity, and exact function, VReg, point, candidate,
and entry-transition counts.

Authority: exact candidate physical views for each occupied VReg point and
exact entry-fixed-view to operand-fixed-view incompatibilities. It grants no
range split, inserted copy, selected-CFG mutation, physical home, spill, frame,
emission, or publication authority.

## Candidate Rule

For each point of each block-local VReg fragment, the analysis starts from the
VReg's physical register class. General candidates must be allocatable views.
A candidate rejects when any storage or canonical-write unit intersects:

- the exact active reservation profile;
- architectural semantic liveness at that block and point; or
- an architectural use, definition, or clobber action at that block and point.

An explicit fixed view replaces the general candidate set at its exact phase.
It must have the required class and remain disjoint from reservations and
architectural state, but it need not be generally allocatable: target-owned
fixed instruction forms may legitimately name such a view. Candidate rows are
strictly view-ID sorted, nonempty, and point exact.

## Fixed-View Transitions

An entry value may be constrained to one ABI view and later used by an
instruction requiring another. The legality plan records each such mismatch as
an `entry_transitions` row retaining both views and the exact destination
operand site.

The forwarded-value fixture exposes two path-specific requirements:
`RSI -> RAX` on x86-64 and `X1 -> X0` on AArch64, one for each return leaf. The
constant-leaf fixture has no incompatible entry constraint and therefore no
transition rows.

These rows do not choose a transformation. Hoisting one copy before the branch
and inserting separate copies in the leaves are distinct, named split/copy
plans with different costs. A later independently validated transformation
must choose and materialize one before allocation can publish changing homes.

## Validation And Gaps

Production and replay use separate derivations. Replay checks the exact range
and environment roots, reconstructs every occupied unit and candidate set,
reconstructs transition rows, rejects reordering or empty/superset candidates,
and recomputes a domain-separated identity.

Virtual interference remains a separate range fact and constrains the
allocator's simultaneous choices. The bounded downstream register-home stage
consumes this artifact only when every VReg has a shared legal view across all
of its points and there are no transition requirements. The separately named
`LeafLocalBeforeFixedUseV1` transformation can materialize the exact admitted
entry-to-leaf-return case; it must then discard these analysis results and
recompute liveness, ranges, and legality. General fixed-to-fixed path analysis,
calls, ties, early clobbers, providers, spills, and frame assignment remain
unsupported. No result from this stage can enter machine emission.

## Implementation Map

- `selected-instructions-to-register-homes/src/analyses/allocation_legality/`
  owns computation, identity and independent replay. The adjacent `legality/`
  module selects policy and retains cross-step custody.
- `register-model` owns aliases, write footprints and validated active
  reservations; `register-homes` owns durable allocation identities.

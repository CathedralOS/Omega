# Allocation Legality To Register Homes

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in stage assigns deterministic physical register views for the exact
bounded subset whose allocation is already fully described by one
transition-free legality plan. It is a base case for the general allocator, not
a physical-emission boundary.

## Stage Contract

Input: `StagedOptimizedAllocationLegality`, nesting the selected CFG, liveness,
live ranges, target register environment, reservations, phase-specific legal
views, virtual interference, and fixed-view transition requirements.

Output: `StagedOptimizedRegisterHomes`, nesting the complete input plus opaque
validated VReg-to-view assignments. Its receipt binds every upstream identity,
the register-home content identity, and exact function and assignment counts.

Authority: one stable physical view for each admitted VReg. It grants no
selected-CFG mutation, inserted copy, range split, spill, rematerialization,
stack slot, frame, instruction emission, or publication authority.

## Deterministic Assignment

VRegs are considered by first live point and then stable VReg ID. A VReg is
admitted only when it has no unresolved entry-to-operand fixed-view transition
and the intersection of its exact per-point candidate rows is nonempty.

Candidates are considered by stable physical-view ID. A candidate rejects when
its storage or canonical-write footprint overlaps the assigned footprint of an
already placed interfering VReg. The lowest remaining candidate becomes the
home. Values with no exact interference may reuse one view, including the two
mutually exclusive result leaves in the current conditional fixture. If no
candidate remains, the stage rejects; it does not invent a spill or copy.

## Validation And Custody

Production and replay use separate derivations. Replay reconstructs the target
register-environment identity from the physical model, constraint catalog,
reservation profile, selected constraint keys, and target; checks exact range
and legality roots; repeats canonical ordering, candidate intersection, and
footprint/interference selection; rejects reordered or altered assignments;
and recomputes a domain-separated register-home identity.

The orchestration carrier retains the complete legality stage. Therefore a
home receipt cannot be detached from Terminal Psi, the exact named optimizer
selection, optimization-unit and projection identities, fuel schedule,
selected CFG, liveness, ranges, legality, or target register environment.

## Known Gaps

The forwarded-value fixture still rejects when passed directly because its ABI
entry view and fixed return view differ on both admitted targets. The exact
named `LeafLocalBeforeFixedUseV1` transformation can now materialize that case
in selected IR, create fresh VRegs, retain complete provenance and fuel custody,
and rerun liveness, ranges, and legality. A separate post-copy custody carrier
then invokes this same strict home algorithm; the direct carrier is not
weakened and cannot interpret a transition as permission to switch homes.

General active-interval allocation, splitting, calls and clobber crossings,
ties, early clobbers, spills, rematerialization, stack slots, frames, and
machine emission remain unsupported.

## Implementation Map

- `optimization/omega-regalloc/src/home_assignment_*` owns production,
  independent replay, the opaque validated artifact, and content identity.
- `orchestration/omega-optimization-pipeline/src/register_homes.rs` owns nested
  cross-stage custody.
- `omega-register-model` remains the sole authority for views, aliases, write
  footprints, constraints, and active reservations.

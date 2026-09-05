# Allocation Legality To Register Homes

This is an internal contract of [Selected Instructions To Register Homes](selected_instructions_to_register_homes.md), not a standalone pipeline crate.

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in stage assigns deterministic physical register views for the exact
bounded subset whose allocation is already fully described by one
transition-free, spill-free legality plan. It is a constrained interference
allocator, not a physical-emission boundary.

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

Distinct use-to-definition ties first quotient VRegs into allocation vertices.
Every vertex retains its ordered members, earliest and latest live points, and
the intersection of every member's ordinary and early-clobber candidate rows.
An unresolved entry transition or empty intersection rejects before placement.

The constraint graph is derived from two explicit sources:

- any interference between members of two vertices forbids overlapping
  storage or canonical-write footprints; and
- an early-clobber definition forbids its write footprint from overlapping
  the storage footprint of each untied use.

At every placement step, the allocator recomputes the candidates compatible
with already assigned neighbors. It chooses the vertex with the fewest viable
views, then the greatest remaining constrained degree, earliest live point,
and lowest leader VReg. The lowest compatible physical-view ID becomes the
home for every member. This constrained ordering admits cases that a
start-ordered greedy walk rejected, such as assigning `{r0}` before an
interfering `{r0, r1}` vertex. Unconstrained vertices may still reuse one view.
If no candidate remains, the stage reports exact pressure; it does not invent
a spill or copy.

## Validation And Custody

Production and replay use separate derivations. Replay reconstructs the target
register-environment identity from the physical model, constraint catalog,
reservation profile, selected constraint keys, and target; checks exact range
and legality roots; independently rebuilds tied vertices, candidate domains,
interference and directional early-clobber constraints, and the canonical
placement order; rejects reordered or altered assignments; and recomputes a
domain-separated register-home identity.

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

Spill insertion, live-range splitting, general call/clobber crossings,
coalescing beyond mandatory ties, stack slots, frames, and machine emission
remain outside this stage. Exact ties and early-clobber constraints are part of
the admitted graph rather than deferred gaps.

## Implementation Map

- `selected-instructions-to-register-homes/src/assignment/` owns both
  assignment algorithms and their direct/post-reanalysis entrances.
  `home_assignment/` keeps independent producer and replay implementations;
  `baseline/` and `transformed/` retain their respective input evidence.
- `register-homes` owns durable home plans and their codec.
  `register-model` owns views, aliases, constraints and reservations.

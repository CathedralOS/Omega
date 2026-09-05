# Fixed-View Copies To Reanalyzed Legality

This is an internal contract of [Selected Instructions To Register Homes](selected_instructions_to_register_homes.md), not a standalone pipeline crate.

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

Any selected-CFG rewrite invalidates its predecessor analyses. This stage runs
fresh liveness, live-range, interference, architectural-state, and
physical-view-legality derivations over the independently validated transformed
selected plan. It never adapts or patches the source facts.

## Stage Contract

Input: `StagedOptimizedFixedViewCopies`, including the complete before/after
custody chain and unchanged target register environment.

Output: `StagedOptimizedSelectedReanalysis`, retaining the transformation plus
fresh opaque validated liveness, ranges, and legality. Its receipt binds all
source and transformation identities, the transformed selected identity, every
new analysis identity, counts, and exactly zero remaining entry transitions.

Authority: analysis only. This stage grants no additional rewrite, physical
home, emission, or publication authority.

## Validation Boundary

`selected-instructions-to-register-homes` exposes a sealed selected-analysis interface implemented only
for the original opaque selector result and the opaque independently validated
fixed-view-copy result. External code cannot implement it for a raw plan. This
avoids forging `ValidatedSelectedInstructions` while allowing the same
independent liveness and range validators to consume the transformed CFG.

The stage revalidates transformation custody, then independently replays each
new analysis. The new live-range identity must name the transformed selected
identity and new liveness identity; the new legality identity must name the new
range identity and the exact unchanged register environment. Any remaining
fixed-view transition rejects the stage.

A separate post-copy home carrier consumes only this reanalyzed zero-transition
legality result and invokes the same strict bounded home assigner used by the
direct transition-free path. It still grants no machine-emission authority.

## Implementation Map

- `selected-instructions-to-register-homes/src/analyses/reanalysis/`
  owns transformed analysis custody over the sealed selected-analysis input.
- The same transform's `assignment/transformed/` owns post-copy home assignment
  and its retained evidence. Neither step is a separate allocator crate.

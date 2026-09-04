# Liveness To Live Ranges

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in stage converts validated selected-CFG liveness into canonical
allocator-input range facts. It preserves CFG topology rather than treating
block layout as execution order, and it authorizes no allocation.

## Stage Contract

Input: one complete `StagedOptimizedLiveness` carrier. Production and custody
validation revalidate both the selected plan and its liveness receipt; a raw or
detached range plan cannot enter orchestration.

Output: `StagedOptimizedLiveRanges`, nesting the entire input carrier plus an
opaque validated range plan. Its receipt binds the Terminal Psi program,
target, entry machine, optimizer/projection/unit/fuel identities, selected and
liveness identities, the exact target-register-environment identity, range
identity, and structural counts.

Authority: block-local live fragments, exact live-edge connectors, operand and
fixed-view occurrences, architectural state/actions, and canonical VReg
interference. It grants no splitting, physical home, spill, stack slot, frame,
emission, or publication authority.

## CFG-Aware Point Model

Instruction position `p` has `before = 2p` and `after = 2p + 1`. Uses occur at
the before point and definitions at the after point. Each block owns one exact
half-open point domain. A VReg or architectural unit is represented by maximal
half-open fragments inside individual blocks; fragments are never joined
merely because their numeric layout positions are adjacent.

Every live successor row becomes an exact connector retaining source block,
terminator, nonzero/zero polarity, Psi edge, and target block. Thus values live
across control flow remain connected without creating a convex global interval,
and values in mutually exclusive leaf blocks remain disjoint.

## Distinct Constraint Domains

VReg rows retain class, ordered use/def occurrences, entry and operand fixed
views, fragments, and edge connectors. Canonical unordered VReg pairs record
interference when two ranges occupy a common point in the same block.

Architectural register units never become graph VRegs. Their semantic
live fragments and connectors are separate from instruction action rows.
Implicit uses occur before an instruction; implicit defs and clobbers occur
after it. A dead RIP/PC write is therefore still a machine-state action without
being falsely extended as semantic liveness.

## Validation Boundary

The producer and validator have separate derivation implementations. The
validator replays liveness custody, point/domain construction, maximal
fragments, exact edge connectors, occurrences, fixed sites, architectural
actions, sorted canonical pairs, and a domain-separated content identity over
every retained field. Nonmaximal, overlapping, reordered, detached, or
content-corrupted rows reject.

## Current Admitted Shapes And Gaps

The constant-leaf conditional demonstrates that the two leaf-local result
VRegs do not interfere. The forwarded-parameter conditional demonstrates a
shared VReg live on both branch edges and the first real condition/result
interference pair. Both shapes run on x86-64 and AArch64 with target-owned unit
effects and fixed views.

Use-def operands, ties, and early clobbers remain fail-closed. Loops, calls,
crashes, cleanup, suspension, block-parameter/parallel-copy edges, loop weights,
splitting, and allocation remain unsupported until their selected-IR frontiers
and independent checks exist. The admitted physical model, constraint catalog,
and conservative active reservation profile now have explicit replay
identities, but provider/runtime reservation closure, fixed-home transitions,
and explicit transition materialization remain prerequisites for authoritative
assignment. The next allocation-legality stage now supplies the phase-exact
candidate facts rather than asking an allocator to rederive them.

## Implementation Map

- `pipeline/omega-regalloc` owns the range model, computation, identity,
  independent replay, and opaque validated result.
- `pipeline/omega-liveness-to-live-ranges` owns the executable stage and nested
  cross-stage custody.
- `omega-selected-instructions` and the liveness artifact remain the
  authoritative inputs; transitional assigned scratch homes are not consulted.

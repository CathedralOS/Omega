# Target Operations To Selected Instructions

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This stage selects a typed virtual-register instruction CFG. It runs after
independently validated optimized abstract lowering and before liveness or
physical register assignment. The bounded direct-assignment continuation is a
temporary compatibility path, not a second representation family.

## Stage Contract

Input: one complete `ValidatedOptimizedTargetOperations` custody carrier plus
the exact independently validated target register environment.

Output: `StagedOptimizedSelectedInstructions`, which owns that input custody,
the validated selected plan, and identities binding the Terminal Psi program,
optimization unit, fuel schedule, optimized projection, target, exact joined
register environment, and selection.

Primary responsibility: turn admitted target operations into typed virtual
register instructions with exact register constraints, machine-state effects,
Psi provenance, and path-specific logical-fuel placement. It does not compute
liveness itself, choose physical homes, emit machine instructions, or authorize
publication. The separate opt-in liveness stage may consume only this validated
carrier.

## Current Admitted Shape

The selector accepts seven exact three-block scalar families under one runtime
Boolean parameter: immediate pairs, entry-parameter pairs, exact-add pairs,
exact-subtract pairs, widened exact-add pairs, widened exact-subtract pairs, and
an active-resident exact-add chain paired with one false-arm immediate. One
ordered catalog is the complete source-shape inventory. It selects exactly zero
or one family; omission rejects as unsupported and overlap fails closed.

Every scalar family constructs its virtual-register roster and blocks as one
body.
The common entry compares the condition and branches to the two exact source
successors. Leaves then materialize, forward, or compute their unsigned 64-bit
result and return it with exact operation, obligation, value, edge, definition,
and fuel provenance. Fixed ABI views are constraints, not assigned homes.
x86-64 obtains RFLAGS/RIP effects and AArch64 obtains NZCV/PC effects from their
independently validated ISA-owned catalog rows. Other shapes reject rather than
falling back or entering transitional assignment.

One separate atomic plan family admits the exact two-function owned-linear
projected structural call/return closure already authenticated by target
legalization. It creates no scalar virtual register or selected machine
instruction. Instead it retains eight direct 8-byte integer fragment
placements, the exact ordinary-call and return register-constraint rows,
fixed views/classes/access, complete implicit uses/defs/clobbers, and the three
required transfers. X86-64 transfers with the complete `copy_i64` row when ABI
views differ; AArch64 records an explicit same-view/no-copy transfer. Liveness
and pre-allocation machine-effect analysis reject every nonempty instance, so
no later physical authority follows from this selection boundary.

## Semantic Custody

- The compare is compiler-introduced work and consumes the exact condition
  value without inventing a Psi operation or logical-fuel charge.
- The branch retains both source edges, polarity, source targets, ordered
  bindings, and separate successor fuel. Only the taken successor settles its
  edge.
- Each materialization retains the exact leaf-local constant operation, value,
  definition site, and operation fuel.
- Each return retains the exact returned value, return edge, fixed result
  constraint, and edge fuel.
- A canonical selected-plan content identity covers every retained field. The
  independent validator reconstructs the projection from the optimized unit,
  abstract plan, target plan, and target register catalog rather than trusting
  the producer.

## Implementation Map

- `representations/omega-selected-instructions` owns data shapes only.
- `pipeline/omega-target-operations-to-selected-instructions` owns
  production, independent validation, source-custody joins, and content
  identity construction.
- `selection/construction/mod.rs` owns the complete scalar, plain-Unit, and
  structural-Unit roster join.
- `selection/construction/scalar/mod.rs` reconstructs common condition context,
  selects one row from the adjacent `catalog.rs`, and assembles the complete
  selected function from that row's register-plus-block body.
- `selection/construction/structural_unit/mod.rs` joins independently
  reconstructed ABI layout, optional structural call, and Unit return; layout
  and call mechanics descend into named leaves.
- `selection/construction/projected_structural_call_return/mod.rs` coordinates
  the bounded atomic closure through named projection, constraint, and transfer
  leaves; its independent validator descends through separate source and target
  replay.
- `pipeline/optimization/omega-optimization-pipeline/src/stages/selection/selection.rs` owns the opaque
  cross-stage carrier, injects exact ISA/ABI constraints, and binds the physical
  model, constraint catalog, active reservation profile, and selected keys into
  one environment identity.
- `omega-isa-x86_64` and `omega-isa-aarch64` own the mapping
  from target machine registers to validated physical register views.

## Known Gaps

The selected CFG must expand to the complete legalized instruction vocabulary,
including scalar calls, memory, cleanup, suspension, additional proof-bearing
operations, loops, and general value flow. Bounded block/instruction liveness
and CFG-aware range fragments now exist for the admitted exact three-block
families, but general
liveness, range splitting, allocation, spills, frame assignment, and
independent physical-realization validation remain later stages. The existing
scratch-cycling assigned-operation route is transitional and is not evidence
for any of those properties.

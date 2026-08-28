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

The initial selector intentionally accepts two exact three-block forms under
one runtime Boolean parameter. In the first, each successor block contains one
leaf-local unsigned 64-bit constant followed by a cleanup-free return. In the
second, one additional unsigned 64-bit entry parameter is returned directly
from either cleanup-free leaf. Other shapes reject rather than falling back or
entering transitional assignment.

The selected plan contains three virtual registers and six instructions:
compare, conditional branch, two materializations, and two returns. The
condition's ABI view and each return view are fixed constraints, not assigned
homes. x86-64 obtains RFLAGS/RIP effects and AArch64 obtains NZCV/PC effects
from their independently validated ISA-owned catalog rows.

The forwarded-parameter form has two ABI-constrained entry VRegs and four
instructions: compare, branch, and two returns. The shared result VReg remains
live on both exact successor edges. Its entry view and later fixed return views
are separate constraint sites; no copy, split, or physical home is invented by
selection.

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
- `pipeline/optimization/omega-optimization-pipeline/src/stages/selection/selection.rs` owns the opaque
  cross-stage carrier, injects exact ISA/ABI constraints, and binds the physical
  model, constraint catalog, active reservation profile, and selected keys into
  one environment identity.
- `omega-isa-x86_64` and `omega-isa-aarch64` own the mapping
  from target machine registers to validated physical register views.

## Known Gaps

The selected CFG must expand to the complete legalized instruction vocabulary,
including calls, memory, cleanup, suspension, proof-bearing operations, loops,
and general value flow. Bounded block/instruction liveness and CFG-aware range
fragments now exist for these two exact three-block shapes, but general
liveness, range splitting, allocation, spills, frame assignment, and
independent physical-realization validation remain later stages. The existing
scratch-cycling assigned-operation route is transitional and is not evidence
for any of those properties.

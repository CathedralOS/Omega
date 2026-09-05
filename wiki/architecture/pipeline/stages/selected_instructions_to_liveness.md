# Selected Instructions To Liveness

This is an internal contract of [Selected Instructions To Register Homes](selected_instructions_to_register_homes.md), not a standalone pipeline crate.

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This opt-in stage computes exact liveness facts over an independently validated
selected-instruction CFG. It is an analysis and custody boundary, not register
allocation. The ordinary empty-selection compiler path does not enter it.

## Stage Contract

Input: one complete `StagedOptimizedSelectedInstructions` carrier. The stage
does not accept a detached raw selected plan or infer facts from the
scratch-cycling assigned-operation route.

Output: `StagedOptimizedLiveness`, which owns the complete input carrier plus an
opaque validated liveness plan and receipt. Its custody binds Terminal Psi,
target, entry machine, optimizer run, optimized projection, optimization unit,
fuel schedule, selected-plan identity, liveness identity, and structural
counts.

Authority: block/instruction liveness and operand-constraint positions only.
The carrier grants no live-interval, interference, physical-home, spill, frame,
emission, or publication authority.

## Separate Dataflow Domains

Virtual registers and architectural register units remain distinct.

- Virtual-register transfer is `before = uses union (after - defs)`.
- Architectural-unit transfer is `before = implicit_uses union (after -
  implicit_defs - clobbers)`.

This distinction makes the compare-to-branch RFLAGS or NZCV dependency visible
without inventing a virtual register for flags. RIP/PC, RSP/SP, and X30 remain
architectural state. Fixed ABI views remain virtual-register position
constraints, not assigned homes or implicit unit liveness.

Every function records entry definitions, dense instruction positions, and
operand positions. Every block records exact canonical live-in/live-out sets
and ordered instruction facts. Conditional successor rows preserve terminator,
nonzero/zero polarity, Psi edge, selected target, and edge-specific live sets,
even when the two sets are equal.

## Validation Boundary

The producer computes a deterministic reverse fixed point. The independent
validator separately reconstructs selected instruction order, CFG successors,
operand roles, fixed constraints, implicit state effects, block and instruction
transfers, successor facts, and sorted duplicate-free sets. It then derives a
domain-separated content identity over every retained field.

Orchestration revalidates the complete selected custody and the raw liveness
plan before issuing the nested receipt. Provenance, proof obligations, logical
fuel, and cleanup ownership are not treated as liveness uses; they remain bound
through the selected-plan identity and parent carrier.

## Current Admitted Shape And Gaps

The initial production slice covers both selected three-block runtime Boolean
conditional forms on x86-64 and AArch64. The constant form contains compare,
branch, two materializations, and two cleanup-free returns. The forwarded form
contains compare, branch, and two returns of one shared unsigned-i64 entry VReg;
it proves exact live-out and successor facts on both arms. V1 rejects use-def
operands, tied operands, and early clobbers because their interference timing
is not yet implemented.

Live intervals, loop weights, call crossings, crashes, cleanup and suspension
frontiers, disconnected functions, allocation, splitting, spills, frame
assignment, and physical-realization validation remain future stages. Broader
liveness coverage depends on representing those frontiers explicitly in the
selected CFG; it must not be inferred from provenance or transitional physical
homes.

## Implementation Map

- `omega-selected-instructions-to-register-homes/src/analyses/liveness/` owns
  computation, content identity and independent validation. Its `staging/`
  module binds those facts to the admitted pipeline input.
- `omega-selected-instructions` owns the selected CFG; register-model and ISA
  catalogs supply physical units, views and instruction constraints.

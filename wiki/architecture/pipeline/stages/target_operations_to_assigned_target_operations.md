# Target Operations To Assigned Target Operations

[Pipeline](../pipeline.md) | Previous: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md) | Next: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md)

This stage assigns physical homes such as registers, stack slots, spills, and calling-convention locations.

## Stage Contract

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and calling-convention homes.

## Implementation Map

- `builder.rs` owns copying target operation arenas into assigned target operation arenas and selecting assigned homes for value operands.
- `registers.rs` owns architecture-specific scratch register selection until real allocation replaces the current fixed policy.

## Semantic Ownership

- Places: become concrete stack/runtime homes or target-addressable memory shapes.
- Values: receive assigned homes such as immediates, stack slots, runtime storage, runtime pointees, indexed runtime-frame locations, or scratch registers; target value summaries are preserved as assigned value metadata.
- Facts: diagnostic metadata only; this stage does not discharge proof obligations.
- Loans: prior-stage invariant only; borrow state is not rechecked here.
- Moves: preserve target ownership summaries while physical homes are assigned; explicit assigned transfer operation lowering is still pending.
- Drops: preserve target ownership summaries while physical homes are assigned; explicit assigned cleanup operation lowering is still pending.
- Calls: receive physical ABI placement when represented by target operation metadata.
- Transitions: receive concrete branch/linkage operands where possible, without changing control-flow shape.
- Effects: remain operation metadata attached to already-authorized operations.
- Boundary edges: preserve target boundary-edge summaries while host-call
  operands receive physical ABI placement.

## Ownership Rules

- Must not own object encoding, final bytes, semantic validation, proof discharge, or borrow checking.
- Must keep register/stack assignment policy here instead of leaking it backward into target operation construction.
- Must preserve target operation ordering unless a later allocator explicitly owns reordering.

## Known Gaps

Current scratch register assignment is fixed and minimal. Real register allocation, spill insertion, and full stack-frame assignment should grow here or in narrow modules immediately under this stage.
Ownership summaries are preserved through assignment but not yet lowered into
assigned copy/cleanup operations.
Boundary-edge summaries are preserved through assignment.
Value summaries are preserved through assignment, but their storage/drop
consequences are still metadata rather than explicit assigned cleanup or move
operations.

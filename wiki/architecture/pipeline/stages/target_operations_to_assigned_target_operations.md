# Target Operations To Assigned Target Operations

[Pipeline](../pipeline.md) | Previous: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md) | Next: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md)

This stage assigns physical homes such as registers, stack slots, spills, and calling-convention locations.

## Stage Contract

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and calling-convention homes.

## Semantic Ownership

- Places: become concrete homes or memory locations.
- Values: become assigned registers, stack slots, immediates, or symbols.
- Facts: diagnostic metadata only.
- Loans: prior-stage invariant only.
- Moves: become assigned copies or spills.
- Drops: become assigned cleanup operations.
- Calls: receive physical ABI placement.
- Transitions: receive concrete branch/linkage operands where possible.
- Effects: remain operation metadata.
- Boundary edges: receive physical ABI placement.

## Ownership Rules

Must not own: object encoding or final bytes.

## Known Gaps

Register allocation and stack assignment should stay here, not leak back into target-aware operation construction.

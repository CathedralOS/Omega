# Abstract Operations To Target Operations

[Pipeline](../pipeline.md) | Previous: [Control Flow To Abstract Operations](control_flow_to_abstract_operations.md) | Next: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md)

This stage legalizes abstract operations against target, layout, ABI, ISA, and calling-convention constraints.

## Stage Contract

Input: abstract operations.

Output: target-aware operations.

Primary responsibility: legalize operations using target, layout, ABI, ISA, and calling-convention knowledge.

## Semantic Ownership

- Places: lower to target-aware memory/register shapes.
- Values: become target-legal operands.
- Facts: should not be re-proved here.
- Loans: should not be rechecked here.
- Moves: become legal target copies, loads, stores, or elisions.
- Drops: become target-callable cleanup sequences.
- Calls: become target-aware call sequences.
- Transitions: become target-aware branch/jump/return operations.
- Effects: map to target/runtime operations.
- Boundary edges: map to ABI-aware host/runtime/compiler operation shapes.

## Ownership Rules

Must not own: language acceptance of unsafe behavior.

## Known Gaps

This stage needs clean separation between legalization and physical assignment.

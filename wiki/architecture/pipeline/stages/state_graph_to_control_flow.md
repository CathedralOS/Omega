# State Graph To Control Flow

[Pipeline](../pipeline.md) | Previous: [Checked Trees To State Graph](checked_trees_to_state_graph.md) | Next: [Control Flow To Abstract Operations](control_flow_to_abstract_operations.md)

This stage lowers the state-machine graph into explicit blocks, branches, calls, exits, and data-flow structure.

## Stage Contract

Input: `StateGraph`.

Output: `ControlFlow`.

Primary responsibility: lower state-machine structure into explicit blocks, branches, calls, exits, and data flow.

## Semantic Ownership

- Places: become control-flow-accessible storage/value references.
- Values: become explicit data-flow operands or temporaries.
- Facts: should be preserved as annotations or diagnostics support where needed.
- Loans: should have already been validated; any remaining data is for correctness-preserving lowering.
- Moves: should become control-flow events before backend lowering.
- Drops: should become scheduled control-flow cleanup.
- Calls: explicit control-flow operations.
- Transitions: lowered into branches, calls, exits, and block edges.
- Effects: attached to operations/blocks for later reporting and validation.
- Boundary edges: attached to operations that lower to imported/compiler/runtime code.

## Ownership Rules

Must not own: semantic proof discharge or target register assignment.

## Known Gaps

Control-flow should not erase move/drop/boundary events before the backend can lower them.

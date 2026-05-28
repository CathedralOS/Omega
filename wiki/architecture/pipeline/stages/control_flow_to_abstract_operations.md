# Control Flow To Abstract Operations

[Pipeline](../pipeline.md) | Previous: [State Graph To Control Flow](state_graph_to_control_flow.md) | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This stage starts backend lowering by converting checked control flow into target-independent operations.

## Stage Contract

Input: `ControlFlow`.

Output: target-independent abstract operations.

Primary responsibility: lower checked control flow into explicit operations with virtual registers and target-independent storage/value actions.

## Semantic Ownership

- Places: lower toward abstract storage references.
- Values: become abstract operands, temporaries, constants, or virtual registers.
- Facts: mostly diagnostic/proven metadata.
- Loans: should be already validated; may remain as assertions.
- Moves: become explicit abstract copies/transfers or no-ops.
- Drops: become abstract cleanup/deallocation calls or no-ops.
- Calls: become abstract call operations.
- Transitions: become branches, jumps, returns, exits, and block edges.
- Effects: attach to operations.
- Boundary edges: become abstract runtime/host/compiler calls.

## Ownership Rules

Must not own: target register assignment or machine instruction selection.

## Known Gaps

Currently some runtime lowering decisions are still too tangled with later backend stages.

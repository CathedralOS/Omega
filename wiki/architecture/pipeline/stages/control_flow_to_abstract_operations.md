# Control Flow To Abstract Operations

[Pipeline](../pipeline.md) | Previous: [State Graph To Control Flow](state_graph_to_control_flow.md) | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This stage starts backend lowering by converting checked control flow into target-independent operations.

## Stage Contract

Input: `ControlFlow`.

Output: target-independent abstract operations.

Primary responsibility: lower checked control flow into explicit operations with virtual registers and target-independent storage/value actions.

## Implementation Map

- `lib.rs` owns the public stage entrypoint only.
- `lowering.rs` owns `AbstractOperationLoweringInput` and adapts the current
  control-flow/runtime planning bundle into instruction-selection input.
- The actual operation construction currently happens in
  `omega-instruction-selection`; this is a transitional boundary, not the
  desired long-term split.

## Semantic Ownership

- Places: should lower toward abstract storage references, but much of that
  policy still lives beyond this adapter.
- Values: should become abstract operands, temporaries, constants, or virtual
  registers.
- Facts: preserved as diagnostic/proven metadata; not re-proved here.
- Loans: already validated; may remain as assertions or metadata.
- Moves: should become explicit abstract copies/transfers or no-ops from
  control-flow ownership events.
- Drops: should become abstract cleanup/deallocation calls or no-ops from
  control-flow ownership events.
- Calls: should become abstract call operations.
- Transitions: should become branches, jumps, returns, exits, and block edges.
- Effects: should attach to abstract operations for later reporting/lowering.
- Boundary edges: should become abstract runtime/host/compiler calls.

## Ownership Rules

- Must preserve checked/control-flow evidence while adapting into backend
  planning inputs.
- Must not own semantic proof discharge, borrow validation, target register
  assignment, machine instruction selection, object encoding, or final image
  policy.
- Must not hide long-term abstract-operation construction inside opaque adapter
  plumbing.

## Known Gaps

This stage is not yet a true representation-to-representation lowering pass.
Runtime and instruction-selection policy still owns too much of the abstract
operation construction that should eventually live here.
It also does not yet consume control-flow move/drop events to build explicit
transfer and cleanup operations.

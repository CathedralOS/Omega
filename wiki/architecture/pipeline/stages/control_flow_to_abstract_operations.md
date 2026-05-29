# Control Flow To Abstract Operations

[Pipeline](../pipeline.md) | Previous: [State Graph To Control Flow](state_graph_to_control_flow.md) | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This stage starts backend lowering by converting checked control flow into target-independent operations.

## Stage Contract

Input: `ControlFlow`.

Output: target-independent abstract operations.

Primary responsibility: lower checked control flow into explicit operations with virtual registers and target-independent storage/value actions.

## Implementation Map

- `lib.rs` owns the public stage entrypoint only.
- `lowering.rs` owns abstract operation plan assembly and attaches preserved
  semantic summaries after the transitional instruction-selection adapter runs.
- `lowering/input.rs` owns `AbstractOperationLoweringInput` and adapts the
  current control-flow/runtime planning bundle into instruction-selection input.
- `lowering/semantics.rs` owns construction of `AbstractSemanticSummary` from
  control-flow semantic roots and lowered host-call evidence. The top-level
  lowering code should assign this root as a unit instead of mutating individual
  semantic sub-arenas, and should use `AbstractSemanticSummary` constructors
  rather than spelling out its internal fields.
- `omega-control-flow/src/semantics.rs` is the source semantic root for this
  stage: `ControlFlowSemanticRoots` keeps proof, invariant, contract, value,
  boundary, borrow, and ownership arenas visibly separate from executable
  control-flow shape.
- `lowering/ownership.rs` owns the control-flow ownership-event copy into the
  abstract-operation ownership summary. It should remain a preservation/lowering
  seam, not a place to invent new move/drop semantics.
- `lowering/boundary.rs` owns the host-operation to abstract boundary-edge
  summary copy. It records the backend-visible trust edge, not source-level
  authorization.
- `omega-abstract-operations/src/plan.rs` owns the representation root:
  executable operation shape lives under `AbstractOperationCode`, while
  preserved semantic evidence lives under `AbstractSemanticSummary`.
- `omega-abstract-operations/src/semantics.rs` owns grouped semantic-root
  construction for abstract values, boundary edges, and ownership summaries.
  `instruction/function.rs`
  owns abstract function plans,
  `instruction/operation.rs` owns abstract operation records and source
  coordinates, `instruction/operation_kind.rs` owns abstract operation kinds,
  `instruction/value_operand.rs` owns abstract value operands, and
  `instruction/storage.rs` owns runtime storage regions.
- The actual operation construction currently happens in
  `omega-instruction-selection`; this is a transitional boundary, not the
  desired long-term split.

## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Lower toward abstract storage references, but much of that policy still lives beyond this adapter. |
| Values | Preserved as abstract value summaries; later passes should turn them into operands, temporaries, constants, virtual registers, or storage policy. |
| Facts | Preserved as diagnostic/proven metadata; not re-proved here. |
| Loans | Already validated; may remain as assertions or metadata. |
| Moves | Preserved as abstract ownership events; they should later become explicit abstract copies/transfers or no-ops. |
| Drops | Preserved as abstract ownership events; they should later become abstract cleanup/deallocation calls or no-ops. |
| Calls | Should become abstract call operations. |
| Transitions | Should become branches, jumps, returns, exits, and block edges. |
| Effects | Should attach to abstract operations for later reporting/lowering. |
| Boundary edges | Control-flow source boundary edges and lowered host operations become distinct abstract boundary summaries beside abstract runtime/host/compiler calls. |

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
It preserves control-flow move/drop events as abstract ownership summaries, but
does not yet consume those summaries to build explicit transfer and cleanup
operations.
It preserves control-flow value summaries as abstract value summaries, but does
not yet consume them to decide type-aware ownership kind, storage shape, or
runtime operand lowering.
Boundary-edge summaries now preserve both source-level boundary trait edges and
lowered host-operation edges. The remaining gap is linking those two layers
and validating the result against target policy.

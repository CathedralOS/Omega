# State Graph To Control Flow

[Pipeline](../pipeline.md) | Previous: [Checked Trees To State Graph](checked_trees_to_state_graph.md) | Next: [Control Flow To Abstract Operations](control_flow_to_abstract_operations.md)

This stage lowers the state-machine graph into explicit blocks, branches, calls, exits, and data-flow structure.

## Stage Contract

Input: `StateGraph`.

Output: `ControlFlow`.

Primary responsibility: lower state-machine structure into explicit blocks, branches, calls, exits, and data flow.

## Semantic Ownership

This stage owns control-flow shape. It should turn graph topology into explicit
blocks and operations without changing the semantic truth established by checked
trees and preserved by the state graph.

| Noun | Ownership |
| --- | --- |
| Places | Become control-flow-accessible storage/value references. |
| Values | Become explicit data-flow operands, temporaries, or carried payloads. |
| Facts | Preserved as annotations/diagnostic support; not re-proved. |
| Loans | Preserved only as correctness metadata/assertions; not revalidated. |
| Moves | Should become explicit control-flow events before backend lowering. |
| Drops | Should become scheduled control-flow cleanup before backend lowering. |
| Calls | Become explicit control-flow operations. |
| Transitions | Lower into branches, calls, exits, continuations, and block edges. |
| Effects | Attach to operations/blocks for later reporting and validation. |
| Boundary edges | Attach to operations that lower to imported/compiler/runtime code. |

## Ownership Rules

Must own:

- Explicit block/operation/branch/exit structure for state-machine execution.
- Preservation of graph-carried contracts, borrows, facts, effects, and boundary
  edges as control-flow metadata or events.
- Scheduling of already-checked cleanup and ownership events once those events
  exist in the graph input.

Must not own:

- Semantic proof discharge, borrow overlap validation, target register
  assignment, ABI lowering, instruction selection, or object/image emission.

## Implementation Map

This stage should read as graph-to-control-flow remapping, with each semantic
noun preserved in a focused file:

- `builder.rs` assembles the final `ControlFlowPlan` from graph arenas and owns
  only top-level orchestration.
- `machines.rs` remaps machine, contained-machine, and owned-data metadata.
- `states.rs` remaps state nodes and state parameters while preserving state
  contract, borrow, operation, transition, and effect summaries.
- `operations.rs` remaps graph operations into control-flow operations.
- `transitions.rs` remaps graph transition edges and transition targets.
- `facts.rs` preserves proof obligations and invariant facts.
- `contracts.rs` and `borrows.rs` preserve checked evidence summaries without
  revalidating them. `borrows/conversions.rs` owns individual borrow root,
  access, call, loan, activation, and weakening conversion from graph form into
  control-flow form.
- `handles.rs` owns handle-span remapping helpers only.

## Known Gaps

- Control-flow should not erase move/drop/boundary events before the backend can
  lower them.
- Once moves/drops/boundary edges become first-class checked/graph events, this
  stage needs dedicated operation variants instead of generic metadata leakage.

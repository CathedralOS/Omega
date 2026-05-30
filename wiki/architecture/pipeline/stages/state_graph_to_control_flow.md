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
The input semantic source is `StateGraphSemanticRoots`.
The representation root is `ControlFlowPlan`: executable shape lives in the
`ControlFlowCode` root for expressions, machines, states, operations, and
transitions, while preserved semantic evidence lives under
`ControlFlowSemanticRoots`.

| Noun | Ownership |
| --- | --- |
| Places | Become control-flow-accessible storage/value references. |
| Values | Graph value summaries are preserved as control-flow value summaries. |
| Facts | Preserved as annotations/diagnostic support; not re-proved. |
| Loans | Preserved only as correctness metadata/assertions; not revalidated. |
| Moves | Preserved from graph ownership summaries into control-flow ownership events. |
| Drops | Preserved from graph ownership summaries into control-flow ownership events. |
| Calls | Become explicit control-flow operations. |
| Transitions | Lower into branches, calls, exits, continuations, and block edges. |
| Effects | Attach to operations/blocks for later reporting and validation. |
| Boundary edges | Graph boundary summaries are preserved as control-flow boundary summaries. |

## Ownership Rules

Must own:

- Explicit block/operation/branch/exit structure for state-machine execution.
- Preservation of graph-carried contracts, borrows, facts, effects, and boundary
  edges as control-flow metadata or events.
- Preservation of graph-carried value summaries without inventing storage or
  ownership policy.
- Scheduling of already-checked cleanup and ownership events once those events
  exist in the graph input.

Must not own:

- Semantic proof discharge, borrow overlap validation, target register
  assignment, ABI lowering, instruction selection, or object/image emission.

## Implementation Map

This stage should read as graph-to-control-flow remapping, with each semantic
noun preserved in a focused file:

- `builder.rs` assembles the final `ControlFlowPlan` from graph arenas and owns
  only top-level orchestration. `builder/borrowed.rs` owns borrowed graph
  remapping, while `builder/owned.rs` owns owned graph remapping. Both paths
  join executable roots and preserved semantic roots through
  `ControlFlowPlan::with_roots`.
- `omega-control-flow/src/plan.rs` owns the representation roots:
  executable control-flow shape lives under `ControlFlowCode`, while preserved
  semantic evidence lives under `ControlFlowSemanticRoots`. The plan
  constructor should keep those roots explicit instead of relying on ad hoc
  field assembly, and `ControlFlowCode::with_roots` should be the join point
  for executable expression/machine/state/operation/transition roots.
- `omega-control-flow/src/semantics.rs` owns the `ControlFlowSemanticRoots`
  bundle for preserved proof, invariant, contract, value, boundary, borrow, and
  ownership arenas. Its constructor names those noun roots explicitly, and this
  stage should use that constructor when preserving graph semantic evidence.
  Individual fact, contract, value, boundary, borrow, and ownership roots should
  likewise use their noun-specific root constructors instead of raw field
  assembly.
- `machines.rs` remaps machine, contained-machine, and owned-data metadata.
- `states.rs` remaps state nodes and state parameters while preserving state
  contract, value, boundary, borrow, ownership, operation, transition, and effect summaries.
- `operations.rs` remaps graph operations into control-flow operations.
- `transitions.rs` remaps graph transition edges and transition targets.
- `facts.rs` preserves proof obligations and invariant facts.
  `facts/conversions.rs` owns individual proof obligation, proof owner, proof
  kind, and invariant conversion from graph form into control-flow form.
- `contracts.rs` and `borrows.rs` preserve checked evidence summaries without
  revalidating them. `contracts/conversions.rs` owns individual contract fact,
  call, and exit conversion from graph form into control-flow form.
  `borrows/conversions.rs` owns individual borrow root, access, call, loan,
  activation, and weakening conversion from graph form into control-flow form.
- `boundaries.rs` owns boundary-edge conversion from graph form into
  control-flow form.
- `ownership.rs` owns move/drop event conversion from graph form into
  control-flow form.
- `values.rs` owns value-summary conversion from graph form into control-flow
  form.
- `arena_remap.rs` owns the shared borrowed-arena remapping loop. Noun modules
  still own conversion policy; the helper only preserves arena shape while
  applying those conversions.
- `handles.rs` owns only generic handle/span remapping mechanics and re-exports
  noun-specific remappers from `handles/{code,borrows,boundaries,contracts,ownership,values}.rs`.
  This keeps handle conversion searchable by the same semantic categories as
  the remapped arenas.

## Known Gaps

- Control-flow now preserves move/drop ownership events, but backend lowering
  still needs to decide how moves become transfers and drops become cleanup.
- Control-flow now preserves value summaries, but later lowering still needs
  type-aware ownership, storage, and operand consequences.
- Control-flow boundary summaries preserve source-level boundary trait edges,
  but abstract/backend host-operation summaries still need explicit linkage
  back to them.

# Checked Trees To State Graph

[Pipeline](../pipeline.md) | Previous: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md) | Next: [State Graph To Control Flow](state_graph_to_control_flow.md)

This stage turns checked machine/state structure into an explicit graph for scheduling, proof reasoning, and later control-flow lowering.

## Stage Contract

Input: `CheckedTrees`.

Output: `StateGraph`.

Primary responsibility: make machine/state transitions explicit for scheduling, proof, and later control-flow lowering.

## Semantic Ownership

This stage owns scheduling shape, not semantic invention. It takes checked
facts/events and makes state-machine topology explicit enough that later stages
can lower without rediscovering machine structure.

| Noun | Ownership |
| --- | --- |
| Places | Carried only when graph nodes/edges need state, parameter, or data identity. |
| Values | Transition arguments and state payloads become graph-carried data. |
| Facts | Checked facts attach to states, edges, calls, exits, and proof summaries. |
| Loans | Preserved as graph-visible borrow summaries/events; not revalidated here. |
| Moves | Should be graph-visible if a transition consumes ownership. |
| Drops | Should be graph-visible if a transition exits a lifetime region. |
| Calls | State/helper calls become graph actions or edge computations. |
| Transitions | First-class graph edges with targets, guards, continuations, and payloads. |
| Effects | Accumulated per machine/state/edge where later reporting or lowering needs them. |
| Boundary edges | Must remain visible when graph actions cross host/compiler/runtime boundaries. |

## Ownership Rules

Must own:

- Machine, state, transition, operation, contract, borrow, proof, and effect
  summaries in graph form.
- Edge-local payloads needed to lower transitions without consulting source
  syntax again.
- Preservation of checked evidence without weakening or inventing proofs.

Must not own:

- Parser recovery, name/type lookup, proof invention, borrow validation, or
  target instruction lowering.
- Physical storage, ABI placement, object/image symbols, or platform-specific
  boundary mechanics.

## Implementation Map

The implementation should keep graph scheduling separate from evidence
preservation:

- `builder.rs` orchestrates per-machine graph construction and worker-local graph
  merging.
- `segments.rs` splits checked state statements into graph segments.
  `segments/branching.rs` owns branch-call topology detection and recursive
  branch-flow discovery. `segments/operations.rs` owns graph operation kind
  selection and expression-ref copying.
- `states.rs` assembles graph state nodes from segments, including state-local
  contract, borrow, effect, operation, and transition summaries.
- `transitions.rs` plans graph transition edges, targets, continuations, guards,
  and transition expression refs.
- `contracts.rs`, `borrows.rs`, and `facts.rs` preserve checked evidence in
  graph-shaped summaries; they should not revalidate proof or borrow legality.
- `remap.rs` remaps worker-local operations, transitions, and expressions into
  the final graph arena.
- `machine_metadata.rs` projects machine owned data, contained machines, and
  direct/reached effect bits into graph metadata.
- `capacity.rs` estimates graph arena sizes; it should stay about allocation
  planning, not semantic ownership.

## Known Gaps

- Transition ownership transfer should be as explicit as call ownership transfer.
- Move/drop events need durable checked-tree inputs before this stage can carry
  them honestly.
- Boundary edges should have an explicit graph representation instead of only
  being implied by contracts/effects/operations.

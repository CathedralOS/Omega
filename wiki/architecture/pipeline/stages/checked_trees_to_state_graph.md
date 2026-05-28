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

## Known Gaps

- Transition ownership transfer should be as explicit as call ownership transfer.
- Move/drop events need durable checked-tree inputs before this stage can carry
  them honestly.
- Boundary edges should have an explicit graph representation instead of only
  being implied by contracts/effects/operations.

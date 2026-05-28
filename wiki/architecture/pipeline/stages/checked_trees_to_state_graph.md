# Checked Trees To State Graph

[Pipeline](../pipeline.md) | Previous: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md) | Next: [State Graph To Control Flow](state_graph_to_control_flow.md)

This stage turns checked machine/state structure into an explicit graph for scheduling, proof reasoning, and later control-flow lowering.

## Stage Contract

Input: `CheckedTrees`.

Output: `StateGraph`.

Primary responsibility: make machine/state transitions explicit for scheduling, proof, and later control-flow lowering.

## Semantic Ownership

- Places: should be carried only when graph edges need state/data identity.
- Values: transition arguments and state payloads become graph data.
- Facts: relevant checked facts should be attachable to states/edges.
- Loans: should preserve enough information to avoid illegal graph rewrites.
- Moves: should be explicit if a transition consumes a value.
- Drops: should be explicit if a transition exits a lifetime region.
- Calls: state/helper calls become graph actions or edge computations.
- Transitions: first-class graph edges.
- Effects: should be accumulated per node/edge where relevant.
- Boundary edges: should stay visible when graph actions cross host/compiler boundaries.

## Ownership Rules

Must not own: proof invention, parser recovery, target instruction lowering.

## Known Gaps

Transition ownership transfer should be as explicit as call ownership transfer.

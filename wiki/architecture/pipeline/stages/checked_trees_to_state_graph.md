# Checked Trees To State Graph

[Pipeline](../pipeline.md) | Previous: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md) | Next: [State Graph To Control Flow](state_graph_to_control_flow.md)

This stage turns checked machine/state structure into an explicit graph for scheduling, proof reasoning, and later control-flow lowering.

This is a bootstrap stage, not the future public Psi boundary. Its machine,
state, edge, and semantic-root structure is input to terminal Psi, but its copied
`ExpressionTable`, source handles, and source-derived segmentation are not part
of the terminal schema. See [Terminal Psi Architecture](../terminal_psi.md).

## Stage Contract

Input: `CheckedTrees`.

Output: `StateGraph`.

Primary responsibility: make machine/state transitions explicit for scheduling, proof, and later control-flow lowering.

## Semantic Ownership

This stage owns scheduling shape, not semantic invention. It takes checked
facts/events and makes state-machine topology explicit enough that later stages
can lower without rediscovering machine structure.
The input semantic source is `CheckFacts`, including grouped checked-flow
roots under `FlowFacts`.
The representation root is `StateGraph`: scheduling topology lives in the
`StateGraphCode` root for expressions, machine/state topology, operations, and
transitions, while preserved semantic evidence lives under
`StateGraphSemanticRoots`.

| Noun | Ownership |
| --- | --- |
| Places | Carried only when graph nodes/edges need state, parameter, or data identity. |
| Values | Checked value facts become state-local graph value summaries. |
| Facts | Checked facts attach to states, edges, calls, exits, and proof summaries. |
| Loans | Preserved as graph-visible borrow summaries/events; not revalidated here. |
| Moves | Preserved from checked-flow ownership events into graph ownership summaries. |
| Drops | Preserved from checked-flow ownership events into graph ownership summaries. |
| Calls | State/helper calls become graph actions or edge computations. |
| Transitions | First-class graph edges with targets, guards, continuations, and payloads. |
| Reach | Accumulated per machine/state/edge where later reporting or lowering needs it. |
| Boundary edges | Preserved from checked-flow boundary events into state-local graph boundary summaries. |

## Ownership Rules

Must own:

- Machine, state, transition, operation, contract, value, borrow, proof, and
  effect summaries in graph form.
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

- `builder.rs` orchestrates per-machine graph construction and worker
  scheduling.
- `omega-state-graph/src/graph.rs` owns the representation roots: executable
  scheduling shape lives under `StateGraphCode`, while preserved semantic
  evidence lives under `StateGraphSemanticRoots`. Graph construction should
  join executable roots through `StateGraphCode::with_roots` and then join
  code plus semantic roots through `StateGraph::with_roots` so code topology
  and semantic evidence stay visibly separate.
- `omega-state-graph/src/semantics.rs` owns the `StateGraphSemanticRoots`
  bundle for preserved proof, invariant, contract, value, boundary, borrow, and
  ownership arenas. Its constructor names those noun roots explicitly so later
  stages do not need to infer the semantic spine from field order. Individual
  fact, contract, value, boundary, borrow, and ownership roots should likewise
  use their noun-specific root constructors instead of raw field assembly.
- `merge.rs` owns worker-local graph merging and remapping of state-local
  contract, value, boundary, borrow, ownership, operation, transition, and
  metadata spans into the final graph. Worker-local source arenas should stay
  bundled so the merge API exposes one semantic source bundle instead of a long
  argument list.
- `segments.rs` splits checked state statements into graph segments.
  `segments/branching.rs` owns branch-call topology detection and recursive
  branch-flow discovery. `segments/operations.rs` owns graph operation kind
  selection and expression-ref copying. `segments/parameters.rs` owns
  state-parameter payload materialization for graph segments.
  `segments/transitions.rs` owns segment-transition payloads, branch-call
  transition spans, unconditional-transition detection, and expression-copy
  helpers used by graph transition planning.
- `states.rs` assembles graph state nodes from segments, including state-local
  contract, value, boundary, borrow, ownership, effect, operation, and transition summaries.
- `transitions.rs` assembles graph transition edges, guards, and transition
  expression refs. `transitions/targets.rs` owns transition/call target
  resolution and continuation segment lookup. A one-member named transition is
  local only when its exact symbol is the source machine's entry or one unique
  source-owned state; a foreign free-machine name is call-shaped and rejects as
  an unsupported jump instead of entering local-state repair.
- `contracts.rs`, `borrows.rs`, and `facts.rs` preserve checked evidence in
  graph-shaped summaries; they should not revalidate proof or borrow legality.
  `contracts/summary.rs` owns state-local contract summary construction, while
  `contracts/remap.rs` owns contract-summary arena remapping when worker-local
  graph fragments are merged.
  `borrows/remap.rs` owns borrow-summary arena remapping when worker-local graph
  fragments are merged. `borrows/calls.rs` owns call argument access
  preservation, while `borrows/lifetimes.rs` owns active-loan,
  activation, and weakening preservation from checked flow.
- `boundaries.rs` preserves checked-flow boundary edges into graph-shaped
  state-local boundary summaries and remaps worker-local boundary arenas during
  graph merging.
- `ownership.rs` preserves checked-flow move/drop events into graph-shaped
  ownership summaries and remaps worker-local ownership arenas during graph
  merging. `ownership/summary.rs` owns state-local move/drop preservation,
  while `ownership/remap.rs` owns worker-local ownership arena remapping.
- `values.rs` preserves checked value facts into state-local graph value
  summaries, enriches the matching statement/expression facts with checked
  arithmetic-policy adapter evidence for spelled and named operators (including
  nested operators), and remaps worker-local value arenas during graph merging.
- `remap.rs` owns narrow operation/transition/expression remap helpers used by
  graph merging.
- `machine_metadata.rs` projects machine owned data, the checked
  `CarryFacts` contained-field topology, and direct/reached operational/service
  summaries into graph metadata. It does not rediscover containment from type
  names.
- `capacity.rs` estimates graph arena sizes; it should stay about allocation
  planning, not semantic ownership. `capacity/expressions.rs` owns copied
  expression-table sizing for those allocation estimates.

## Known Gaps

- This stage still copies executable `TypedTrees` expressions rather than
  lowering them. Terminal Psi replaces those handles with canonical values,
  predicates, structural places, operations, and edge-local actions.

- Value summaries preserve checked expression origins and normalized
  arithmetic-policy adapter evidence, but still need ownership kind, drop
  policy, and storage consequences.
- Transition ownership transfer should be as explicit as call ownership transfer.
- Move/drop event producers are still conservative upstream, so this stage
  preserves the available ownership evidence but cannot yet expect complete
  type-aware transfer/drop coverage.
- Graph boundary summaries preserve checked source-level boundary edges, but
  backend host-operation summaries still need explicit linkage back to them.

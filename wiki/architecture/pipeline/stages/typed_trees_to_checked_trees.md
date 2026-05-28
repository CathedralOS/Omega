# Typed Trees To Checked Trees

Input: `TypedTrees`.

Output: `CheckedTrees`.

Primary responsibility: validate semantic obligations and build checked facts.

Semantic nouns:

- Places: first strongly useful place layer via `omega_facts::Place` and
  checked-flow `CanonicalPlace`.
- Values: still weaker than desired; expressions and symbols stand in for value
  instances.
- Facts: first-class fact contexts, origins, payloads, proof obligations, and
  contract facts.
- Loans: first-class borrow facts, accesses, loans, activations, weakenings, and
  overlap checks.
- Moves: should become first-class here; currently too implicit.
- Drops: should become first-class here; currently too implicit.
- Calls: first-class call facts for contracts, borrows, flow, and effects.
- Transitions: checked for proof/arguments, but ownership transfer needs more
  explicit data.
- Effects: direct/transitive effect plans are available.
- Boundary edges: represented through boundary contracts/operators/policies, but
  should become explicit checked-flow events.

Must not own: machine instruction shape, ABI placement, final storage layout.

Known gaps: add durable value, move, drop, and boundary-edge events to checked
flow.

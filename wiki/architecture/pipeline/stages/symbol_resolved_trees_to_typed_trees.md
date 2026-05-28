# Symbol Resolved Trees To Typed Trees

Input: `SymbolResolvedTrees`.

Output: `TypedTrees`.

Primary responsibility: attach type and signature meaning.

Semantic nouns:

- Places: type-aware member/index candidates.
- Values: typed expression results.
- Facts: typed facts and constraints.
- Loans: not known yet, except through mutable/reference type surfaces.
- Moves: not yet durable events.
- Drops: type information can imply future drop requirements, but scheduling is
  deferred.
- Calls: typed call signatures and argument/return expectations.
- Transitions: typed transition arguments and return/exit expectations.
- Effects: typed effect declarations and call surfaces.
- Boundary edges: typed boundary contracts and operator signatures.

Must not own: final proof discharge, liveness, move/drop scheduling, ABI layout.

Known gaps: value identity should start becoming more explicit here so checked
trees are not forced to reverse-engineer it from expressions.

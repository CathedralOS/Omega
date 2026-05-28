# Symbol Resolved Trees To Typed Trees

[Pipeline](../pipeline.md) | Previous: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md) | Next: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md)

This stage attaches type and signature meaning to resolved program structure.

## Stage Contract

Input: `SymbolResolvedTrees`.

Output: `TypedTrees`.

Primary responsibility: attach type and signature meaning.

## Semantic Ownership

- Places: type-aware member/index candidates.
- Values: typed expression results.
- Facts: typed facts and constraints.
- Loans: not known yet, except through mutable/reference type surfaces.
- Moves: not yet durable events.
- Drops: type information can imply future drop requirements, but scheduling is deferred.
- Calls: typed call signatures and argument/return expectations.
- Transitions: typed transition arguments and return/exit expectations.
- Effects: typed effect declarations and call surfaces.
- Boundary edges: typed boundary contracts and operator signatures.

## Ownership Rules

Must not own: final proof discharge, liveness, move/drop scheduling, ABI layout.

## Known Gaps

Value identity should start becoming more explicit here so checked trees are not forced to reverse-engineer it from expressions.

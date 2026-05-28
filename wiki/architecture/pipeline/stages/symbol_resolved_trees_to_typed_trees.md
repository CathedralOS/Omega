# Symbol Resolved Trees To Typed Trees

[Pipeline](../pipeline.md) | Previous: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md) | Next: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md)

This stage attaches type and signature meaning to resolved program structure.

## Stage Contract

Input: `SymbolResolvedTrees`.

Output: `TypedTrees`.

Primary responsibility: attach type and signature meaning.

## Semantic Ownership

This stage owns type and signature meaning. It may prove that a construct has a
type-compatible shape, but it should not decide liveness, ownership transfer, or
whether facts discharge runtime proof obligations.

| Noun | Ownership |
| --- | --- |
| Places | Type-aware member/index candidates, not yet checked borrow/move places. |
| Values | Typed expression results and return expectations. |
| Facts | Typed fact and constraint surfaces. |
| Loans | Mutable/reference type surfaces only; no active loan model. |
| Moves | Not durable events yet. |
| Drops | Type information can imply future drop requirements; scheduling is deferred. |
| Calls | Typed call signatures, argument expectations, and return expectations. |
| Transitions | Typed transition arguments and return/exit expectations. |
| Effects | Typed effect declarations and call surfaces. |
| Boundary edges | Typed boundary contracts and operator signatures. |

## Ownership Rules

Must own:

- Type identity, type compatibility, and signature compatibility.
- Typed call, transition, operator, domain, effect, and boundary surfaces.
- Enough value/type information for checked trees to build durable facts without
  reverse-engineering source syntax.

Must not own:

- Final proof discharge, liveness, borrow overlap, move/drop scheduling,
  graph/control-flow shape, ABI layout, or target storage.

## Known Gaps

- Value identity should start becoming more explicit here so checked trees are
  not forced to reverse-engineer it from expressions.
- Boundary contracts should keep typed provider/capability surfaces distinct
  from later checked-flow boundary events.

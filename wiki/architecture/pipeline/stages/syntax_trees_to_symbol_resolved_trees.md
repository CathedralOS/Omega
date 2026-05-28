# Syntax Trees To Symbol Resolved Trees

[Pipeline](../pipeline.md) | Previous: [Tokens To Syntax Trees](tokens_to_syntax_trees.md) | Next: [Symbol Resolved Trees To Typed Trees](symbol_resolved_trees_to_typed_trees.md)

This stage attaches symbol identity to definitions and references while preserving the source-shaped program structure.

## Stage Contract

Input: `SyntaxTrees`.

Output: `SymbolResolvedTrees`.

Primary responsibility: attach symbol identity to definitions and references.

## Semantic Ownership

- Places: names and members begin to resolve to symbols, but place validity is not proven.
- Values: expression producers gain resolved names.
- Facts: proof facts can refer to resolved domains, symbols, and members.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: call targets become symbol-facing.
- Transitions: target states become symbol-facing.
- Effects: effect names become symbol-facing.
- Boundary edges: boundary declarations point at resolved constructs, but provider validity is not fully modeled here.

## Ownership Rules

Must not own: type checking, flow invalidation, borrow overlap, backend shape.

## Known Gaps

Keep root/operator/domain symbol handling first-class and avoid string identity leaking into later phases.

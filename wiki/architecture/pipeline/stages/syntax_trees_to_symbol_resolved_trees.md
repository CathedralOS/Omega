# Syntax Trees To Symbol Resolved Trees

Input: `SyntaxTrees`.

Output: `SymbolResolvedTrees`.

Primary responsibility: attach symbol identity to definitions and references.

Semantic nouns:

- Places: names and members begin to resolve to symbols, but place validity is
  not proven.
- Values: expression producers gain resolved names.
- Facts: proof facts can refer to resolved domains, symbols, and members.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: call targets become symbol-facing.
- Transitions: target states become symbol-facing.
- Effects: effect names become symbol-facing.
- Boundary edges: boundary declarations point at resolved constructs, but
  provider validity is not fully modeled here.

Must not own: type checking, flow invalidation, borrow overlap, backend shape.

Known gaps: keep root/operator/domain symbol handling first-class and avoid
string identity leaking into later phases.

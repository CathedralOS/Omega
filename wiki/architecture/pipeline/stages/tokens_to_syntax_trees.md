# Tokens To Syntax Trees

[Pipeline](../pipeline.md) | Previous: [Source Files To Tokens](source_files_to_tokens.md) | Next: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md)

This stage parses tokens into source-shaped syntax without deciding which names, types, or effects they mean.

## Stage Contract

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

## Semantic Ownership

- Places: syntactic expressions that may later become places.
- Values: literal/expression syntax only.
- Facts: parsed proof facts and contract clauses.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: syntactic call expressions/statements.
- Transitions: syntactic transition statements and targets.
- Effects: effect clauses as names.
- Boundary edges: parsed `boundary` traits, operators, authority contracts, library entries, and target policies.

## Ownership Rules

Must not own: symbol identity, type identity, borrow validity, proof discharge.

## Known Gaps

Parser diagnostics and chapter examples should stay synchronized as syntax shifts.

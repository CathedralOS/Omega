# Tokens To Syntax Trees

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

Semantic nouns:

- Places: syntactic expressions that may later become places.
- Values: literal/expression syntax only.
- Facts: parsed proof facts and contract clauses.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: syntactic call expressions/statements.
- Transitions: syntactic transition statements and targets.
- Effects: effect clauses as names.
- Boundary edges: parsed `boundary` traits, operators, capability contracts,
  library entries, and target policies.

Must not own: symbol identity, type identity, borrow validity, proof discharge.

Known gaps: parser diagnostics and chapter examples should stay synchronized as
syntax shifts.

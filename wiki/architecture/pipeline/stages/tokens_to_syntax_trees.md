# Tokens To Syntax Trees

[Pipeline](../pipeline.md) | Previous: [Source Files To Tokens](source_files_to_tokens.md) | Next: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md)

This stage parses tokens into source-shaped syntax without deciding which names, types, or effects they mean.

## Stage Contract

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

## Semantic Ownership

This stage owns source shape only. It may recognize grammar constructs, but it
must not decide which symbol, type, effect, boundary provider, or proof fact any
name denotes.

| Noun | Ownership |
| --- | --- |
| Places | Syntactic expressions that may later become places. |
| Values | Literal/expression syntax only. |
| Facts | Parsed proof facts and contract clauses only. |
| Loans | Not owned. |
| Moves | Not owned. |
| Drops | Not owned. |
| Calls | Syntactic call expressions/statements. |
| Transitions | Syntactic transition statements and targets. |
| Effects | Effect clauses as unresolved names. |
| Boundary edges | Parsed `boundary` traits, operators, authority contracts, library entries, and target policies. |

## Ownership Rules

Must own:

- Grammar, source structure, spans, and parser diagnostics.
- Preservation of syntax needed by later symbol/type/proof stages.

Must not own:

- Symbol identity, type identity, borrow validity, proof discharge, effect
  validity, boundary provider validity, or backend shape.

## Known Gaps

- Parser diagnostics and chapter examples should stay synchronized as syntax
  shifts.
- Syntax pages should avoid describing later semantic ownership as parser
  behavior.

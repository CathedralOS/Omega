# Tokens To Syntax Trees

[Pipeline](../pipeline.md) | Previous: [Source Files To Tokens](source_files_to_tokens.md) | Next: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md)

This stage parses tokens into source-shaped syntax without deciding which names, types, or effects they mean.

## Stage Contract

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

## Implementation Map

- `parser.rs` owns public entrypoints and whole-file parse completion checks.
- `parser/input.rs` owns token cursor movement, span mapping, and parser lookahead helpers.
- `parser/file.rs` and `parser/item.rs` own top-level item sequencing.
- Item modules such as `data.rs`, `domain.rs`, `machine.rs`, `trait_definition.rs`, `operator.rs`, `library.rs`, `platform.rs`, `target.rs`, `export_item.rs`, and `use_item.rs` own the grammar for their corresponding source forms.
- `parser/expression.rs` owns expression precedence and membership parsing.
- `parser/expression/primary.rs` owns literals, grouped expressions, array literals, path names, and struct literals.
- `parser/expression/postfix.rs` owns calls, argument lists, indexing/ranges, member access, and casts.
- `parser/machine.rs` owns machine headers, body/member sequencing, implicit entry construction, and attached-data path splitting.
- `parser/machine/clauses.rs` owns machine `satisfies`, `terminates`, `decreases`, `effects`, `requires`, and `ensures` clauses.
- `parser/statement.rs`, `transition.rs`, `state.rs`, `type_reference.rs`, and `proof_fact.rs` own source-shaped subgrammars reused across items.
- `parser/capability.rs` owns unresolved capability/authority contract syntax.
- `parser/diagnostics.rs` owns parse-time grammar diagnostics.
- `parser/tests.rs` owns broad parser coverage; tests should not live in the entrypoint file.

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

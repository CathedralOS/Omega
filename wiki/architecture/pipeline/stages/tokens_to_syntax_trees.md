# Tokens To Syntax Trees

[Pipeline](../pipeline.md) | Previous: [Source Files To Tokens](source_files_to_tokens.md) | Next: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md)

This stage parses tokens into source-shaped syntax without deciding which names, types, or reach clauses they mean.

## Stage Contract

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

Representation shape: `SyntaxTrees` keeps root item handles under a
`SyntaxTreeRoots` root and arena-backed item, expression, statement, and
type-reference storage under `SyntaxTreeTables`. Parser output should preserve
source shape without turning nested syntax into scattered heap objects.

## Implementation Map

The Psi product role owns this stage; its eventual hosted source belongs under
`source/compiler/omega/psi/`. The current Rust realization is:

- `source/compiler/rust/psi/representations/psi-syntax-trees` contains `SyntaxTrees`, its
  arena-backed tables, identity/snapshot materialization, and all source-shaped
  nodes.
- `source/compiler/rust/psi/pipeline/psi-tokens-to-syntax-trees` contains the parser modules
  listed below. Every workspace harness uses this Psi stage directly.
- `source/compiler/rust/psi/foundation/psi-arena` contains the generic typed dense, paged,
  generational, hierarchy, and ordered-root arena storage required by source
  representations.
- `source/compiler/rust/psi/foundation/psi-diagnostics` contains the target-neutral
  `PhaseSnapshot` contract used to materialize readable source-shaped trees.
- `source/compiler/rust/psi/foundation/psi-language-core` contains the grammar-facing
  multiplicity, data-supply, carry, domain-body, call-acknowledgement,
  atomic-ordering, cast-form, operator-spelling, and source-assembly contract
  vocabulary.
- `source/compiler/rust/psi/foundation/psi-numerics` contains exact numeric meanings,
  arithmetic-domain vocabulary, and integer/float literal payloads. Parser-side
  literal validation therefore remains target-neutral when the stage migrates.
- `source/compiler/rust/psi/foundation/psi-symbols` contains shared symbol identities and
  hierarchy storage. This parser stage does not assign symbols, but later
  Psi-owned resolution can consume its source-shaped output without an Omega
  foundation dependency.
- `parser.rs` owns public entrypoints and whole-file parse completion checks.
- `parser/input.rs` owns token cursor movement, span mapping, and parser
  lookahead helpers.
- `parser/input/delimited.rs` owns balanced delimiter skipping and top-level
  delimiter search.
- `parser/input/literals.rs` owns parser-side numeric literal validation and
  conversion from token text into syntax values.
- `parser/file.rs` and `parser/item.rs` own top-level item sequencing.
- Item modules such as `data.rs`, `domain.rs`, `machine.rs`, `trait_definition.rs`, `operator.rs`, `library.rs`, `platform.rs`, `target.rs`, `export_item.rs`, and `use_item.rs` own the grammar for their corresponding source forms.
- `parser/expression.rs` owns expression precedence parsing.
- `parser/expression/membership.rs` owns executable domain membership parsing,
  including `in`, domain intersections, and domain unions.
- `parser/expression/primary.rs` owns literals, grouped expressions, array literals, path names, and struct literals.
- `parser/expression/postfix.rs` owns calls, argument lists, indexing/ranges, member access, and casts.
- `parser/machine.rs` owns machine headers, body/member sequencing, implicit entry construction, and attached-data path splitting.
- `parser/machine/clauses.rs` owns machine `satisfies`, external-realization
  `via <Binding>`, `terminates [by ...]`, `reaches`, `invokes`, `suspends`,
  `blocks`, `crashes`, `requires`, and `ensures`
  clauses. Every authored `reaches` keyword is retained independently from its
  member span, including a memberless clause, so later stages can distinguish
  explicit empty publication from omission without reparsing source text. A
  machine `requires` or `ensures` clause may retain one explicit
  evidence-term binding (`name: proposition`); a named clause contains exactly
  one proposition. An `ensures` section also admits one
  `ExactResultCase -> { guarantees }` group per declared result case. Group
  entries are ordinary named or unnamed guarantee rows; the braces have no
  expression, aggregate, package, or group-value node. `=>`, `when`, and an
  unseparated case literal are not alternate guard spellings.
  `parser/statement.rs` also recognizes the separated
  proof-output binding `let (value; public_output: local_term) = call()` and its
  evidence-only form. Erased call arguments, evidence assignment, producer
  selection, and proof-output validation belong to later stages. `via` is
  terminal and mutually exclusive with an executable body.
  Standalone `decreases` and the old termination block diagnose their current
  `terminates by ...` replacement rather than entering the syntax tree.
- `parser/transition.rs` owns transition block assembly.
- `parser/transition/guards.rs` owns transition subjects, guard patterns, wildcard matching, and guard expression synthesis.
- `parser/transition/targets.rs` owns transition target parsing and
  classification.
- `parser/transition/targets/copy.rs` owns expression-to-statement-table copying
  for transition target paths and arguments.
- `parser/statement.rs`, `state.rs`, `type_reference.rs`, and `proof_fact.rs` own source-shaped subgrammars reused across items.
  Statement parsing retains `crash Trap;` and `crash Abort;` as explicit
  non-return exits, distinct from an ordinary terminal transition. The retired
  `trap;` spelling diagnoses the replacement rather than silently producing a
  successful terminal edge.
- The retired `capability { entry ... }` host scaffold, `library { entry ... }`
  import block, explicit machine-member `entry`, and trailing
  `boundary host` / `boundary Name` clauses are not accepted grammar. The
  migration on-ramps must diagnose them rather than preserve syntax nodes.
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
| Calls | Syntactic call expressions/statements plus ordered `suspend` / `block` acknowledgements. |
| Transitions | Syntactic transition statements and targets. |
| Reach | Reach clauses and synchronous invocation ceilings as unresolved names. |
| Boundary edges | Parsed `boundary` traits and operators, exact `satisfies ... via` realizations, and target policies. |

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

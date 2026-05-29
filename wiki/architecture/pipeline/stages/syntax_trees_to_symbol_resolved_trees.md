# Syntax Trees To Symbol Resolved Trees

[Pipeline](../pipeline.md) | Previous: [Tokens To Syntax Trees](tokens_to_syntax_trees.md) | Next: [Symbol Resolved Trees To Typed Trees](symbol_resolved_trees_to_typed_trees.md)

This stage attaches symbol identity to definitions and references while preserving the source-shaped program structure.

## Stage Contract

Input: `SyntaxTrees`.

Output: `SymbolResolvedTrees`.

Primary responsibility: attach symbol identity to definitions and references.

## Semantic Ownership

This stage owns symbol identity only. It may say which declaration or member a
name points at, but it must not prove that the resolved construct is type-correct,
borrow-correct, callable, reachable, or safe.

| Noun | Ownership |
| --- | --- |
| Places | Names and members resolve to symbols; place validity is deferred. |
| Values | Expression producers gain resolved names, not proven runtime value identity. |
| Facts | Proof facts may reference resolved domains, symbols, and members. |
| Loans | Not owned. |
| Moves | Not owned. |
| Drops | Not owned. |
| Calls | Call targets become symbol-facing candidates. |
| Transitions | Target states become symbol-facing candidates. |
| Effects | Effect names become symbol-facing candidates. |
| Boundary edges | Boundary declarations point at resolved constructs, but provider validity is deferred. |

## Ownership Rules

Must own:

- Constructing symbol identity for definitions.
- Stamping references with symbol handles when lookup is source/scope based.
- Keeping source names available for diagnostics without letting strings become
  semantic identity.

Must not own:

- Type checking or signature compatibility.
- Flow invalidation, borrow overlap, move/drop scheduling, or proof discharge.
- Backend shape, storage homes, ABI placement, or object/image names.

## Implementation Map

The implementation should stay split by identity task:

- `program.rs` owns stage entry and the top-level lowering conveyor. Integration
  coverage belongs in `program/tests.rs`, not inline with the entrypoint.
- `symbols/symbol_table.rs` creates the root symbol tree and reserves top-level
  child order. `symbols/symbol_table/children.rs` is only the declaration-child
  export surface; `symbols/symbol_table/children/{builtin,data,machines,operators,platforms,traits}.rs`
  own child layout for each declaration family. Machine child layout includes
  inherited attached-data fields and state locals.
  `symbols/symbol_table/names.rs` owns symbol-name seeding and operator display
  names.
- `symbols/lookup.rs` owns reusable symbol-table lookup helpers.
- `symbols/top_level.rs` owns only root-level stamping order.
  `symbols/top_level/{data,domains,operators,platforms,traits}.rs` stamp
  declaration-family symbols and type-reference symbols for their owned
  signatures.
  `symbols/top_level/machines.rs` owns machine symbol stamping, including
  contained objects, owned data, state parameters, state locals, trait
  conformances, and inherited attached-data field offsets.
- `symbols/type_references.rs` owns only the type-reference export surface.
  `symbols/type_references/assignment.rs` stamps type-reference symbols, while
  `symbols/type_references/queries.rs` answers read-side type-reference symbol
  and call-target questions.
- `symbols/scoped_paths.rs` resolves machine/state-scoped name paths for places,
  calls, indexed paths, and transition targets.
  `symbols/scoped_paths/base.rs` owns the base-segment lookup policy across
  state parameters, machine children, attached data, and top-level symbols.
- `symbols/scope.rs` owns `MachineScope`, the local identity context shared by
  statement, expression, call, and transition resolution.
- `symbols/domain_facts.rs` stamps domain/proof fact references.
- `symbols/statements.rs` walks machine states and stamps statement-local calls,
  locals, transition targets, and statement-owned expression references.
- `symbols/expressions.rs` walks expression tables and delegates expression-local
  reference stamping after nested expression children are processed.
  `symbols/expressions/references.rs` owns stamping names, members, calls, and
  domain membership references inside expression tables.
- `symbols/expression_paths.rs` owns only the expression-path export surface.
  `symbols/expression_paths/calls.rs` resolves call targets after receiver
  identity is known, while `symbols/expression_paths/receivers.rs` resolves
  expression receiver/member paths and indexed receiver paths.
- `symbols/expression_paths/stamping.rs` owns writing resolved receiver
  head/final symbols back into expression-table nodes.
- `symbols/targets.rs` owns only target-resolution exports.
  `symbols/targets/transitions.rs` stamps transition targets, while
  `symbols/targets/calls.rs` resolves call target symbols after receiver
  identity is known.
- `symbols.rs` owns only pass sequencing and publication of the final symbol
  table onto `SymbolResolvedTrees`.

## Known Gaps

The symbol-resolution implementation is now split by task, but several modules
still have policy-heavy functions. Keep pressure on `symbols/top_level.rs` and
`symbols/expression_paths.rs` so lookup policy remains separable from tree
traversal and later phases can rely on handles without inheriting string
identity.

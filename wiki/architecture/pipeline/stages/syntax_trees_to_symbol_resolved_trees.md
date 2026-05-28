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

- `symbols/symbol_table.rs` creates the symbol tree and reserves child order.
- `symbols/lookup.rs` owns reusable symbol-table lookup helpers.
- `symbols/type_references.rs` stamps type-reference symbols.
- `symbols/scoped_paths.rs` resolves machine/state-scoped name paths for places,
  calls, indexed paths, and transition targets.
- `symbols/scope.rs` owns `MachineScope`, the local identity context shared by
  statement, expression, call, and transition resolution.
- `symbols/domain_facts.rs` stamps domain/proof fact references.
- `symbols.rs` should continue shrinking toward orchestration plus the remaining
  statement/expression/call/transition stamping seams.

## Known Gaps

Root/operator/domain symbol handling is still too concentrated in implementation
code. Keep splitting symbol-table construction, lookup, and reference stamping so
later phases can rely on handles without inheriting string identity or resolver
control flow.

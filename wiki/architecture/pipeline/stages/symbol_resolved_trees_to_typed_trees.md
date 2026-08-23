# Symbol Resolved Trees To Typed Trees

[Pipeline](../pipeline.md) | Previous: [Syntax Trees To Symbol Resolved Trees](syntax_trees_to_symbol_resolved_trees.md) | Next: [Typed Trees To Checked Trees](typed_trees_to_checked_trees.md)

This stage attaches type and signature meaning to resolved program structure.

## Stage Contract

Input: `SymbolResolvedTrees`.

Output: `TypedTrees`.

Primary responsibility: attach type and signature meaning.

Representation shape: `TypedTrees` keeps top-level program entry spans under a
`TypedTreeRoots` root and typed arena/table storage under `TypedTreeTables`.
This keeps the source-facing typed program spine visible without forcing later
proof, borrow, and flow code to chase nested source syntax.

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
| Reach | Typed reach declarations, invocation ceilings, and call surfaces. |
| Boundary edges | Typed boundary contracts and operator signatures. |

## Ownership Rules

Must own:

- Type identity, type compatibility, and signature compatibility.
- Typed call, transition, operator, domain, effect, and boundary surfaces.
- Enough value/type information for checked trees to build durable facts without
  reverse-engineering source syntax.
- Typed machine contracts retain the optional named evidence binding separately
  from the proposition fact it names; this stage does not infer or select the
  evidence producer.
- Typed call telescopes retain a nested conformance application's own
  lifetime/type/const/static-machine argument kinds. Every non-lifetime slot is
  present explicitly; ordinary lifetime constraints remain available for the
  checker to resolve. The expected evidence binder supplies a compatibility
  target, never omitted conformance arguments.
- Typed named transitions retain their erased evidence-identifier lane without
  assigning it a runtime argument type or storage position.

Must not own:

- Final proof discharge, liveness, borrow overlap, move/drop scheduling,
  graph/control-flow shape, ABI layout, or target storage.
- Concrete boundary calling plans, including registers, stack locations, ABI
  classes, and target machine state. Typed trees retain only the semantic
  boundary key and canonical contract fingerprint; Omega orchestration retains
  the selected realization plan for native lowering.

## Implementation Map

The Psi product role owns this stage and its eventual hosted source belongs
under `compiler/psi/`. The current Rust implementation makes typed semantic
surfaces visible by file:

- `bootstrap/onramps/omega-rust/psi/pipeline/psi-symbol-resolved-trees-to-typed-trees` contains the
  stage implementation. All workspace consumers invoke it directly.

- `bootstrap/onramps/omega-rust/psi/foundation/psi-language-semantics` contains canonical
  const-value atoms and normalized wire scalar ranges used by typed
  normalization.
- `bootstrap/onramps/omega-rust/psi/foundation/{psi-extents,psi-layout-plans,psi-access-plans}`
  contain the normalized author-selected geometry and placed-access semantics that
  typed `Placed<P, T>` surfaces retain. Concrete ABI selection and target
  lowering remain Omega-owned.
- `bootstrap/onramps/omega-rust/psi/representations/psi-typed-trees` contains the typed source
  representation. Consumers depend on this Psi owner directly.

- `lowerer.rs` owns stage entry and the top-level lowering conveyor. Behavior
  coverage belongs in `lowerer/tests.rs`, not inline with the entrypoint.
- `TypedTrees::with_roots` and `TypedTreeRoots::with_roots` are the
  representation seams for joining typed top-level root spans, typed tables,
  and the inherited symbol table.
- `expression.rs` owns the typed-expression lowering entry surface.
- `expression/table.rs` owns only the recursive expression-table export surface.
  `expression/table/lowerer.rs` owns the `ExpressionTableLowerer` context that
  carries the source table, target table, optional program context, and optional
  self-substitution through recursive lowering.
- `expression/domain_membership.rs` lowers executable domain membership into
  typed boolean fact expressions.
- `expression/name_paths.rs` lowers typed name-path members and preserves the
  head/final symbol handles needed by later place and call checks.
- `expression/operators.rs` owns resolved-to-typed operator-kind mapping.
- `expression/tests.rs` owns expression-table lowering canaries.
- `type_reference.rs` owns type-reference shape lowering for reference, slice,
  constrained, generic, fixed-array, named, self, and unit type surfaces.
  `type_reference/direct.rs` lowers inline resolved type references, while
  `type_reference/table.rs` lowers table-backed type-reference handles.
- `type_reference/constraints.rs` owns typed constraint lowering, including
  named constraints and range constraints whose bounds lower through typed
  expressions.
- `statement.rs` owns statement-kind dispatch only.
  `statement/arguments.rs` lowers statement-local expression spans and name
  paths, `statement/calls.rs` lowers typed call statements, and
  `statement/transitions.rs` lowers typed transition guards/targets.
- `state.rs` and `machine.rs` own typed state/machine signatures, not
  checked-flow liveness or borrow legality.

## Known Gaps

- Value identity should start becoming more explicit here so checked trees are
  not forced to reverse-engineer it from expressions.
- Boundary contracts should keep typed provider/capability surfaces distinct
  from later checked-flow boundary events.

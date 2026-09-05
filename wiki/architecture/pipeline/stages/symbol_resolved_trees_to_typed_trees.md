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
| Boundary edges | Typed boundary declarations, requirement contracts, exact realization bindings, and operator signatures. |

## Ownership Rules

Must own:

- Type identity, type compatibility, and signature compatibility.
- Typed call, transition, operator, domain, effect, and boundary surfaces.
- Preserve statement receiver roots separately from final receiver members
  through lowering, copying, and specialization. State-scope checks consume the
  root declaration; projected member ancestry is not storage-root identity.
- Method candidates on call-produced receivers follow the producer's exact
  declared return type after receiver children lower. Member and array/slice
  projections retain their nominal declaration identity; a same-spelled foreign
  declaration cannot select an attached method. This supplies a previously
  unresolved target, not a proof of receiver origin, effects, or borrow legality.
- Enough value/type information for checked trees to build durable facts without
  reverse-engineering source syntax.
- Typed machine contracts retain the optional named evidence binding separately
  from the proposition fact it names; this stage does not infer or select the
  evidence producer.
- Typed reach custody preserves the resolved owner, every authored keyword
  occurrence, and every exact target-symbol/span pair. The semantic row remains
  normalized separately, allowing package projection to distinguish authored
  empty publication from omission and to verify source custody without
  reconstructing names.
- Typed operational custody preserves every authored `suspends` and `blocks`
  keyword on machines and structural signatures independently from the boolean
  ceiling. Copies and generic specializations retain that custody so later
  package explanation can join source to checked meaning without reparsing.
- Typed call telescopes retain a nested conformance application's own
  lifetime/type/const/static-machine argument kinds. Every non-lifetime slot is
  present explicitly; ordinary lifetime constraints remain available for the
  checker to resolve. The expected evidence binder supplies a compatibility
  target, never omitted conformance arguments.
- Typed named transitions retain their erased evidence-identifier lane without
  assigning it a runtime argument type or storage position.
- Authored nominal type spellings retain their exact resolved declaration in
  the package-agnostic selection ledger. The lowering context classifies public
  data, domain, machine-head, trait, and wire positions separately from private
  declarations, internal state signatures, locals, and casts. Primitive types,
  local binders, and source-free compiler nodes do not become package
  selections.
- Closed generic applications retain the exposure of each original use, not
  the shared generated instance's declaration visibility. Lowering records the
  original base and argument tokens once under that use's enclosing exposure.
  Derived instance fields do not create public-interface selections, while
  authored public template fields and public concrete applications still
  require public selected declarations.
- Closed-data method signatures likewise do not invent source selections from
  substituted types. Suppression requires a verified association to the exact
  authored template and closed owner, with unchanged visibility and supply mode.
  Original template signatures remain checked; body call and operator custody
  continues through the existing authored-copy reconciliation.

Must not own:

- Final proof discharge, liveness, borrow overlap, move/drop scheduling,
  graph/control-flow shape, ABI layout, or target storage.
- Concrete boundary calling plans, including registers, stack locations, ABI
  classes, and target machine state. Typed trees retain only the semantic
  boundary key and canonical contract fingerprint; Omega orchestration retains
  the selected realization plan for native lowering.

## Implementation Map

The Psi product role owns this stage and its hosted source belongs under
`source/psi/`. The current Rust implementation makes typed semantic
surfaces visible by file:

- `omega-rust/psi/pipeline/psi-symbol-resolved-trees-to-typed-trees` contains the
  stage implementation. All workspace consumers invoke it directly.

- `omega-rust/psi/foundation/psi-language-semantics` contains canonical
  const-value atoms and normalized wire scalar ranges used by typed
  normalization.
- `omega-rust/psi/foundation/{psi-extents,psi-layout-plans,psi-access-plans}`
  contain the normalized author-selected geometry and placed-access semantics that
  typed `Placed<P, T>` surfaces retain. Concrete ABI selection and target
  lowering remain Omega-owned.
- `omega-rust/psi/representations/psi-typed-trees` contains the typed source
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
  self-substitution through recursive lowering. Its binary-expression path is a
  thin dispatcher/helper pair, so deep left-associated trees do not retain the
  much larger nonbinary typing frame at every node; ordinary frontend
  correctness must not depend on a larger host thread stack.
- `expression/domain_membership.rs` lowers executable domain membership into
  typed boolean fact expressions.
- `expression/name_paths.rs` lowers typed name-path members and preserves the
  head/final symbol handles needed by later place and call checks.
- `call_results.rs` owns exact declared-state lookup shared by call-result
  temporary typing and computed-receiver method selection. Method selection
  consumes declaration types, never a callee-body or returned-place proof.
- `expression/operators.rs` owns resolved-to-typed operator-kind mapping.
- `expression/tests.rs` owns expression-table lowering canaries.
- `type_reference.rs` owns type-reference shape lowering for reference, slice,
  constrained, generic, fixed-array, named, self, and unit type surfaces.
  `type_reference/direct.rs` lowers inline resolved type references, while
  `type_reference/table.rs` lowers table-backed type-reference handles. Both
  retain exact source-backed nominal selections and thread their public/private
  declaration exposure through nested type shapes.
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

# Syntax Trees To Symbol Resolved Trees

[Pipeline](../pipeline.md) | Previous: [Tokens To Syntax Trees](tokens_to_syntax_trees.md) | Next: [Symbol Resolved Trees To Typed Trees](symbol_resolved_trees_to_typed_trees.md)

Psi's pre-resolution frontend normalization first closes target-neutral generic
data instances. This stage then attaches symbol identity to definitions and
references while preserving the resulting source-shaped program structure.

## Stage Contract

Input: `SyntaxTrees`.

Output: `SymbolResolvedTrees`.

Primary responsibility: attach symbol identity to definitions and references
after Psi-owned closed-instance normalization.

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
| Reach | Reach and invocation names become symbol-facing candidates. |
| Boundary edges | Boundary declarations point at resolved constructs, but provider validity is deferred. |

## Ownership Rules

Must own:

- Closing nameable generic data applications and applying exact contextual
  construction identities before name resolution.
- Constructing symbol identity for definitions.
- Stamping references with symbol handles when lookup is source/scope based.
- Keeping source names available for diagnostics without letting strings become
  semantic identity.
- Preserving an explicit named `requires`/`ensures` evidence binding while its
  proposition references acquire ordinary resolved identity. Selection of a
  producer conformance remains a checked proof concern.
- Resolving every nominal and proposition-local binder leaf in a transparent
  proposition's proof-only `zero_value<T>()` target. The expression retains the
  exact type graph; this stage does not decide what all-zero storage denotes.
- Resolving the declared package-scoped conformance symbol in a nested generic
  application while preserving that application's own argument pack separately
  from the enclosing machine telescope. This stage does not infer missing
  arguments or decide that the application satisfies an evidence binder.
- Preserving each exact machine-requirement edge's authored target-trait
  lifetime names beside its resolved trait and requirement selection. Binder
  membership and declaration-order ordinals settle only when the realizing
  machine telescope is available to typed lowering.
- Recording each exact source-backed static conformance argument as an authored
  declaration selection at the argument path's span. Nested static applications
  are walked recursively; this records explicit source choice, not an inferred
  conformance selected later by checking.
- Recording source-authored statement calls from the state statement tree
  before table rebuilding consumes that form. Exact targets and static
  conformance arguments settle here; unresolved call targets retain an explicit
  checked-call obligation at the target token.
- Partitioning authored selections copied into compiler-instantiated trait
  defaults by their exact conformance application. Two applications retain the
  same authored source coordinate but may correctly route a call to different
  realization symbols. The partition is transient compiler join custody: it
  survives resolved copying and ledger rebasing but never enters canonical
  package evidence, lock identity, or source diagnostics as semantic identity.
- Preserving authored `suspends` and `blocks` keyword spans on machine and
  structural signature owners while their booleans acquire resolved context.
  Synthesized trait-default machines inherit the originating requirement's
  spans; this stage does not reconstruct them from source text.
- Recording every source-backed static argument path recursively. A resolved
  conformance uses the conformance kind; every other selected type, machine, or
  forwarded binder or named const uses the common static-argument kind.
  Literal arguments select no declaration. A named const in ordinary
  expression position still retains its declaration in the separate
  const-substitution custody path. An unresolved static path is an explicit
  late obligation, never an omitted row.
- Propagating declaration exposure while lowering source expressions. Public
  declaration contracts and predicates retain public-interface custody while
  executable machine states and bodies remain private. Proof-membership facts
  retain their domain path through a pre-assignment sidecar and become exact
  domain-selection rows after symbols exist. Lexical parameters and locals are
  excluded from declaration custody; later selected path segments remain.
- Retaining callable and trait conformance-bound authority after symbol
  assignment. Bound subjects and evidence binders are lexical; the right-hand
  trait, or the carrier plus named conformance in a qualified bound, receives
  exact declaration custody with the enclosing declaration's exposure.
- Preserving named-transition evidence identifiers separately from runtime
  transition arguments while the target state acquires symbol identity.
- Preflighting signature-free requirement paths after symbol assignment and
  before normalization consumes their authored form. An overloaded exact
  trait requirement family produces one declaration-anchored compatibility
  diagnostic plus source-ordered diagnostics for every affected nominal
  machine binder and authored domain route.
- Resolving an explicit top-level `boundary requirement` to one exact package,
  attached carrier/path, static telescope, signature, visibility, and contract.
  A later `satisfies` or `select_provider` path may cite only that retained
  identity; bodylessness, reach clauses, and catalog presence never synthesize
  it.

Must not own:

- Type checking or signature compatibility.
- Flow invalidation, borrow overlap, move/drop scheduling, or proof discharge.
- Backend shape, storage homes, ABI placement, or object/image names.

## Implementation Map

The Psi product role owns this stage and its hosted source belongs under
`source/psi/`. The current Rust implementation stays split by identity
task:

- `omega-rust/psi/pipeline/psi-syntax-trees-to-symbol-resolved-trees` contains the
  stage implementation. All workspace consumers invoke it directly.

- `omega-rust/psi/pipeline/psi-generic-instances` contains the pre-resolution
  closed-instance and contextual-construction normalization used by that stage
  and by Psi-owned probe frontends. Its public entry consumes one syntax tree
  and returns the normalized tree; the in-place elaborator is private. Omega
  orchestration may sequence that Psi entry while the larger frontend conveyor
  is split, but cannot own or extend the language elaboration.

- `omega-rust/psi/semantics/psi-build-time-evaluation` owns the
  cross-stage evaluation continuation. Pre-resolution returns target-filterable
  syntax beside one opaque, non-cloneable `PreCheckEvaluation` retaining the
  matching plan-laid rows, placed-view rows, and optional package-selection
  authority. Omega may filter targets, resolve names, and type that syntax, then
  consumes the continuation once; it cannot inspect or recombine the retained
  rows or choose a different pre-check authority. Const-length, const-domain,
  plan-laid, placed-view, and wire-plan work remains in its Psi-defined
  fail-fast order.

- `omega-rust/psi/representations/psi-symbol-resolved-trees` contains the stage
  output.

- `omega-rust/psi/foundation/psi-language-semantics` contains the resolved
  semantic identities, service/domain tables, machine supply/termination
  plans, establishment routes, and byte-sequence predicate vocabulary carried
  by this stage.

- `lowerer.rs` owns stage entry and the top-level lowering conveyor. Integration
  coverage belongs in `lowerer/tests.rs`, not inline with the entrypoint.
- `expression.rs` keeps recursive binary-expression lowering behind a thin
  dispatcher/helper pair. Deep left-associated trees therefore do not retain
  the much larger call/desugaring and aggregate-lowering frame at every node;
  ordinary frontend correctness must not depend on a larger host thread stack.
- `signature_free_requirements.rs` owns exact shared resolution policy and the
  pre-normalization overload-compatibility report. Declaration families are
  deduplicated by trait symbol and requirement name; use diagnostics retain
  their authored leaf spans rather than relying on rewritten paths.
- `service_reaches.rs` resolves each authored `reaches` member occurrence once
  to its exact boundary-trait symbol, retains keyword and target spans in a
  compiler-private owner sidecar, and constructs the idempotent parent-closed
  semantic row together with invocation-contributed services. An authored
  memberless row remains distinguishable from omission.
- `SymbolResolvedTrees::with_roots` and `SymbolResolvedRoots::with_roots` are
  the representation seams for joining resolved root arenas, resolved tables,
  and the published symbol table.
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
  Body resolution supplies only the already-stamped prefix of the current
  state's statements. Calls through a local select their attached target from
  that exact local symbol's declared type, peeling reference/constraint shells
  without treating a whole array or slice as an element receiver. A missing
  method, self-initializer, or later declaration remains unresolved; a local's
  spelling cannot select a same-named field, free machine, or type/domain home.
  This stamps a candidate, not evidence of call legality or runtime support.
- `symbols/domain_facts.rs` stamps domain/proof fact references.
- `symbols/statements.rs` walks machine states and constructs the machine/state
  context for statement reference stamping. `symbols/statements/routing.rs`
  owns statement-kind routing for assignments, locals, expression statements,
  calls, transition targets, and statement-owned expression references.
- `symbols/expressions.rs` owns only the expression symbol export surface.
  `symbols/expressions/traversal.rs` walks expression tables and delegates
  expression-local reference stamping after nested expression children are
  processed.
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

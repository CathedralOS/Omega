# Typed Trees To Checked Trees

[Pipeline](../pipeline.md) | Previous: [Symbol Resolved Trees To Typed Trees](symbol_resolved_trees_to_typed_trees.md) | Next: [Checked Trees To State Graph](checked_trees_to_state_graph.md)

This stage validates semantic obligations and builds the checked fact model used by proof, borrow, effect, and flow checks.

## Stage Contract

Input: `TypedTrees`.

Output: `CheckedTrees`.

Primary responsibility: validate semantic obligations and build checked facts.

## Semantic Ownership

This stage is the first durable semantic fact owner. It should be the place
where source/type meaning becomes queryable evidence for proof, borrow, flow,
effect, and boundary validation.

| Noun | Ownership |
| --- | --- |
| Places | First strongly useful place layer via `omega_facts::Place` and checked-flow `CanonicalPlace`. |
| Values | First checked value fact layer via `CheckedValueFacts`, keyed by typed expression handles and value origins. |
| Facts | First-class fact contexts, origins, payloads, proof obligations, and contract facts. |
| Loans | First-class borrow facts, accesses, loans, activations, weakenings, and overlap checks. |
| Moves | First-class checked-flow event arenas/spans exist. Initial producers are type-aware for direct assignments, local initializers, indexed element reads, aggregate literals, binary/range operands, by-value direct-call arguments, nested expression-call arguments, and transition target arguments. |
| Drops | First-class checked-flow event arenas/spans exist. Initial state-exit local drop producers skip copy-like scalar locals. |
| Calls | First-class call facts for contracts, borrows, flow, and effects. |
| Transitions | Checked for proof/arguments; ownership transfer needs more explicit data. |
| Effects | Direct/transitive effect plans are available. |
| Boundary edges | First-class checked-flow events for calls into states supplied by boundary trait signatures. |

## Ownership Rules

Must own:

- Proof obligations and whether current facts discharge them.
- Borrow facts, accesses, loans, activations, weakenings, and overlap failures.
- Effect summaries and boundary contract facts that later stages must preserve.
- Checked value origins for decreases clauses, initializers, statement values,
  call arguments, transition guards/targets, and nested expression children.
- A durable checked-flow representation of calls and transitions.

Must not own:

- Machine instruction shape, ABI placement, final storage layout, relocation
  identity, or platform image policy.
- Rewriting checked obligations into backend convenience data without preserving
  the original semantic evidence.

## Implementation Map

The stage should stay organized around semantic nouns instead of pass history.
Current ownership is:

- `borrow.rs` assembles borrow facts. `borrow/accesses.rs` owns argument access
  routing, `borrow/accesses/collection.rs` owns the shared
  `BorrowAccessCollection` arena/context bundle, `borrow/accesses/read.rs`
  owns recursive read-access traversal,
  `borrow/accesses/place.rs` owns borrow-access place construction,
  `borrow/accesses/contextual.rs` owns state-local contextual name/member
  resolution for those borrow-access places, `borrow/accesses/records.rs` owns
  argument-access fact emission into borrow arenas,
  `borrow/state.rs` owns state-local borrow fact assembly from writable roots,
  loans, call accesses, and last-use updates,
  `borrow/loans.rs` owns local loan creation/rebasing,
  `borrow/loans/types.rs` owns reference-type classification for loan
  creation, `borrow/calls.rs` owns statement-level borrow call-site discovery,
  `borrow/calls/collection.rs` owns the shared `BorrowCallCollection`
  arena/ordinal context,
  `borrow/calls/expression.rs` owns expression-local borrow call discovery,
  `borrow/calls/transitions.rs` owns
  transition-target borrow call discovery, `borrow/tracker.rs` owns per-state
  loan tracker state, `borrow/last_uses.rs` owns loan last-use updates, and
  `borrow/last_uses/usage.rs` owns statement usage routing.
  `borrow/last_uses/usage/expressions.rs` owns expression usage traversal, and
  `borrow/last_uses/usage/transitions.rs` owns transition guard/target usage
  traversal for last-use detection.
- `checks/borrows.rs` is the borrow-check entry point. `checks/borrows/calls.rs`
  owns call-site borrow legality, `checks/borrows/statements.rs` owns local
  borrow and mutation conflicts, `checks/borrows/overlap.rs` owns place/index
  overlap policy, and `checks/borrows/details.rs` owns diagnostic lifetime
  explanations.
- `flow.rs` assembles checked flow facts. `flow/builder.rs` owns the
  machine/state conveyor, `flow/state.rs` owns per-state flow fact assembly and
  entry/exit semantic envelopes, `flow/context.rs` owns the mutable arena
  bundle including ownership-event arenas, `flow/constraints.rs` materializes
  borrow constraints,
  `flow/borrow_lifetimes.rs` owns loan activation/weakening rules,
  `flow/statements.rs` owns statement entry facts, call fact sequencing, loan
  activation, mutation invalidation, and transfer propagation,
  `flow/transfers.rs` owns statement fact transfers, `flow/calls.rs` owns call
  fact assembly, `flow/call_phases.rs` owns call entry/requires/invalidation/exit
  context phase routing, `flow/call_phases/invalidation.rs` owns call mutation
  and domain invalidation, `flow/boundaries.rs`
  owns checked boundary-edge discovery through boundary trait conformances, and
  `flow/exits.rs` owns exit/ensures flow facts. `flow/ownership.rs` is the ownership-event
  entrypoint, `flow/ownership/moves.rs` owns recursive move-event production
  for assignments, initializers, aggregate literals, binary/range operands,
  call arguments, nested expression calls, and transition targets,
  `flow/ownership/calls.rs` owns call-site argument routing,
  `flow/ownership/drops.rs` owns state-exit local drops,
  `flow/ownership/events.rs` owns move/drop fact emission into the ownership
  arenas, `flow/ownership/place_types.rs` owns contextual type-reference
  resolution for canonical places, and `flow/ownership/type_references.rs`
  owns the policy that distinguishes copy-like scalar places from
  ownership-consuming places.
- `flow/domain/*` owns domain dependency and invalidation rules. Mutating a
  place should invalidate facts there, not ad hoc in proof or borrow code.
  `flow/domain/dependencies/expression.rs` owns dependency expression
  traversal, while `flow/domain/dependencies/expression/relative.rs` owns
  relative `self` place projection and member resolution for dependency paths.
  `flow/domain/invalidation.rs` owns context filtering, while
  `flow/domain/invalidation/matching.rs` owns mutation/dependency overlap
  policy.
- `flow/place/*` owns canonical place construction, comparison, and
  type/member resolution used by proof, borrow, and invalidation checks.
  `flow/place/canonicalization.rs` owns conversion from expressions, symbols,
  and semantic fact places into checked-flow `CanonicalPlace` values,
  `flow/place/contextual.rs` owns state-local name/member recovery for
  canonical places, `flow/place/comparison.rs` owns overlap/equality policy,
  and `flow/place/resolution.rs` owns member/type symbol resolution helpers.
- `values.rs` owns the first durable checked value fact layer entrypoint.
  `values/statement.rs` owns statement-role routing, `values/transition.rs`
  owns transition target value routing, and `values/expression.rs` owns nested
  expression traversal. These modules record source expression handles and why
  each value matters, including machine decreases, attached-data field
  initializers, statement values, transition targets, and nested expressions.
  They do not yet decide ownership kind, drop policy, or storage shape.
- `checks/ranges.rs` is the range-check entry point. `checks/ranges/arrays.rs`
  owns fixed-array length discovery, `checks/ranges/indexes.rs` owns
  indexed/subslice validation, `checks/ranges/facts.rs` owns the `RangeFacts`
  storage root, `checks/ranges/facts/values.rs` owns local/field length and
  integer fact lookup/mutation, `checks/ranges/facts/proofs.rs` owns
  index/range-bound proof storage and queries,
  `checks/ranges/facts/proofs/aliases.rs` owns proof alias propagation,
  `checks/ranges/guards.rs` owns guard dispatch,
  `checks/ranges/guards/bounds.rs` owns the comparison-derived fact export
  surface, `checks/ranges/guards/bounds/lengths.rs` owns length fact seeding,
  `checks/ranges/guards/bounds/indexes.rs` owns index and range-bound fact
  seeding, `checks/ranges/guards/bounds/orderings.rs` owns ordering fact
  seeding, `checks/ranges/indexes.rs` owns indexed-expression traversal,
  `checks/ranges/indexes/validation.rs` owns known-length and unknown-slice
  index/subslice proof diagnostics, `checks/ranges/initializers.rs` owns
  data-field and machine-owned integer fact seeding,
  `checks/ranges/proofs.rs` owns proof lookups,
  `checks/ranges/expressions.rs` owns the helper export surface,
  `checks/ranges/expressions/integers.rs` owns scalar integer/range-bound
  expression folding, `checks/ranges/expressions/lengths.rs` owns indexable
  length inference, `checks/ranges/requirements.rs` owns requires-derived proof seeding,
  `checks/ranges/statements.rs` owns statement range routing,
  `checks/ranges/statements/aliases.rs` owns local alias proof seeding,
  `checks/ranges/statements/transitions.rs` owns transition-target range
  routing, `checks/ranges/state_arguments.rs` owns transition argument facts,
  and `checks/ranges/types.rs` owns expression type/slice classification.
- `checks/ranges/state_arguments/calls.rs` owns merging argument-derived facts
  into target state parameters, while `checks/ranges/state_arguments/expressions.rs`
  owns expression traversal that discovers nested state calls, and
  `checks/ranges/state_arguments/statements.rs` owns statement and transition
  traversal for state-argument fact collection.
- `checks/contracts.rs` is the contract-check entry point.
  `checks/contracts/calls.rs` owns call `requires` validation and domain
  invalidation explanations, `checks/contracts/exits.rs` owns exit `ensures`
  validation, `checks/contracts/prover.rs` owns recursive proof orchestration,
  `checks/contracts/direct.rs` owns direct boolean fact matching,
  `checks/contracts/domains.rs` owns domain-membership proof fallback,
  `checks/contracts/labels/calls.rs` owns call-site contract expression label
  substitution, `checks/contracts/labels/domain.rs` owns domain proof label
  substitution, `checks/contracts/places.rs` owns contract-place matching, and
  `checks/contracts/evaluator.rs` owns call-site expression evaluation.
- `proof/*`, `checks/contracts/*`, and `checks/termination/*` should remain
  proof/checking modules. They should consume checked facts and emit
  diagnostics, not invent new durable semantic representations.

## Known Gaps

- Refine checked value facts with ownership kind, drop policy, and
  storage/lowering consequences instead of leaving those decisions attached
  only to flow ownership events.
- Finish move/drop event production across all transfer sites, including
  slice/string operators beyond binary expressions and future user-defined
  copy/drop policy.
- Teach remaining value-expression analysis to append ownership
  transfer/drop events into the existing checked-flow ownership arenas.
- Connect checked boundary edges to backend host-operation boundary summaries
  and target policy decisions.
- Keep contract, proof, borrow, range, termination, and effect checks split by
  noun ownership instead of letting `checks` files become semantic junk drawers.

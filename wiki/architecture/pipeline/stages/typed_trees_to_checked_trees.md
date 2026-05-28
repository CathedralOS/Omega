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
| Values | Partially owned; expressions and symbols still stand in for durable value instances. |
| Facts | First-class fact contexts, origins, payloads, proof obligations, and contract facts. |
| Loans | First-class borrow facts, accesses, loans, activations, weakenings, and overlap checks. |
| Moves | Should become first-class checked-flow events; currently too implicit. |
| Drops | Should become first-class checked-flow events; currently too implicit. |
| Calls | First-class call facts for contracts, borrows, flow, and effects. |
| Transitions | Checked for proof/arguments; ownership transfer needs more explicit data. |
| Effects | Direct/transitive effect plans are available. |
| Boundary edges | Represented through boundary contracts/operators/policies; should become explicit checked-flow events. |

## Ownership Rules

Must own:

- Proof obligations and whether current facts discharge them.
- Borrow facts, accesses, loans, activations, weakenings, and overlap failures.
- Effect summaries and boundary contract facts that later stages must preserve.
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
  collection, `borrow/accesses/read.rs` owns recursive read-access traversal,
  `borrow/accesses/place.rs` owns contextual place resolution,
  `borrow/loans.rs` owns local loan creation/rebasing,
  `borrow/loans/types.rs` owns reference-type classification for loan
  creation, `borrow/calls.rs` owns statement-level borrow call-site discovery,
  `borrow/calls/expression.rs` owns
  expression-local borrow call discovery, `borrow/calls/transitions.rs` owns
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
  bundle, `flow/constraints.rs` materializes borrow constraints,
  `flow/borrow_lifetimes.rs` owns loan activation/weakening rules,
  `flow/statements.rs` owns statement entry facts, call fact sequencing, loan
  activation, mutation invalidation, and transfer propagation,
  `flow/transfers.rs` owns statement fact transfers, `flow/calls.rs` owns call
  entry/requires/ensures/effect/invalidation flow facts, and `flow/exits.rs`
  owns exit/ensures flow facts.
- `flow/domain/*` owns domain dependency and invalidation rules. Mutating a
  place should invalidate facts there, not ad hoc in proof or borrow code.
  `flow/domain/invalidation.rs` owns context filtering, while
  `flow/domain/invalidation/matching.rs` owns mutation/dependency overlap
  policy.
- `flow/place/*` owns canonical place construction, comparison, and type/member
  resolution used by proof, borrow, and invalidation checks.
- `checks/ranges.rs` is the range-check entry point. `checks/ranges/arrays.rs`
  owns fixed-array length discovery, `checks/ranges/indexes.rs` owns
  indexed/subslice validation, `checks/ranges/facts.rs` owns local/field range
  fact storage, `checks/ranges/facts/proofs.rs` owns index/range-bound proof
  propagation and aliasing, `checks/ranges/guards.rs` owns guard-derived facts,
  `checks/ranges/proofs.rs` owns proof lookups,
  `checks/ranges/requirements.rs` owns requires-derived proof seeding,
  `checks/ranges/state_arguments.rs` owns transition argument facts, and
  `checks/ranges/types.rs` owns expression type/slice classification.
- `checks/ranges/state_arguments/calls.rs` owns merging argument-derived facts
  into target state parameters, while `checks/ranges/state_arguments/expressions.rs`
  owns expression traversal that discovers nested state calls.
- `checks/contracts.rs` is the contract-check entry point.
  `checks/contracts/prover.rs` owns recursive proof orchestration,
  `checks/contracts/direct.rs` owns direct boolean fact matching,
  `checks/contracts/domains.rs` owns domain-membership proof fallback,
  `checks/contracts/places.rs` owns contract-place matching, and
  `checks/contracts/evaluator.rs` owns call-site expression evaluation.
- `proof/*`, `checks/contracts/*`, and `checks/termination/*` should remain
  proof/checking modules. They should consume checked facts and emit
  diagnostics, not invent new durable semantic representations.

## Known Gaps

- Add durable value identity so proof, borrow, allocation, and lowering can talk
  about values as clearly as places.
- Add durable move and drop events before graph/control-flow lowering.
- Make boundary edges first-class checked-flow events, not just contract/effect
  side data.
- Keep contract, proof, borrow, range, termination, and effect checks split by
  noun ownership instead of letting `checks` files become semantic junk drawers.

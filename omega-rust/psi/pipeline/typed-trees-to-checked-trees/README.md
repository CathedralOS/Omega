# Typed trees to checked trees

This stage checks typed programs and retains checked facts. Start at
[lib.rs](src/lib.rs). Public borrow requirements are in
[loan resources and compatibility](../../../../wiki/spec/terminal-psi/loans.md).
Authored declaration custody and carried-type dependency production are described
in [authored selections](authored_selections.md).

Source obligations are specified by [state contracts](../../../../wiki/spec/language/state_contracts.md)
and [numeric values](../../../../wiki/spec/language/numeric_values.md). Source
recognition below is implementation coverage, not an additional language rule.

## Fact ownership and queries

`CheckedTrees` retains typed custody under `typed` and durable evidence under
`CheckFacts`. Construct grouped roots through `with_roots`; proof and temporal
flow remain separate. `state_acceptance` and the state/statement/call/exit
`AcceptanceView` APIs gather accepted evidence rather than rerunning checks.
Their summaries derive the aggregate verdict from proof, borrow, effect,
boundary, and termination dimensions. A checked tree is published only after
diagnostics clear; these views are not persisted rejection certificates.

| Owner under `src/` | Responsibility |
| --- | --- |
| `semantic.rs`, `semantic/contracts/`, `semantic/points.rs` | Contract payloads, places, obligation origins, and program points. |
| `semantic_calls.rs`, `semantic_calls/traversal/` | Shared exact state/statement/call coordinates. |
| `flow/`, `flow/call_phases.rs` | Entry facts, call requires, invalidation, guarantees, exits, and transfer order. |
| `flow/place/`, `flow/domain/` | Canonical places and dependency-overlap invalidation. |
| `flow/ownership/` | Type-multiplicity-based moves, drops, argument routes, and result storage. |
| `values/` | Ranking, initializer, argument, transition, and nested-expression origins. |
| `checks/contracts/`, `checks/ranges/`, `checks/termination/` | Proof consumers and diagnostics, not competing durable fact models. |

The representation's `flow/` modules group contexts, invalidations, borrow
lifetimes, ownership, boundaries, and control. `facts/contract_plans.rs` owns
normalized public contracts; source-dependent call-route substitution belongs in
`facts/crash_calls.rs`. Package projection may read an earlier coherent
representation and join checked acceptance afterward. It must not invent a
new semantic stage merely to collect package rows.

Checked evidence contains semantic receipt identities, not target placements,
provider selection, ABI/layout policy, or physical activation plans. Source
specialization retains canonical template bytes, exact arguments, conformance
and contract commitments, and admission custody. Replay its domain-separated
commitment before using it as proof-producer identity; compact reports are not
authority. Omega's coordinator owns the
[ordered settlement boundary](../../../omega/compiler/compiler/checked_settlement.md)
outside this crate.

## Flow, ranges, and progress

Range-state arguments form an entry-rooted all-predecessor fixed point. Rebuild
edge contributions each pass and withhold unconverged inference. Assignment
values share semantic contexts and invalidation with domain facts. Exit checking
uses live assignment evidence, not initializer replay; scalar returns require
exact result/arm binding and checked operator meaning.

Indexed-access checking owns one lazy mutation-summary table for its immutable
program and borrow facts. Incoming-state propagation and branch snapshots borrow
that same table while rebuilding their local bounds. A new check owns a fresh
table; source or borrow changes cannot reuse an earlier invocation's summaries.

`checks/termination/ranking/` separates range membership from descent. Static
single-state integer bounds and the strict-symbol relational tier retain exact
entry parameters, view endpoints, simultaneous self-edge substitution, and
endpoint pinning. Natural distance is `max(limit - index, 0)`. Neither a range
proof nor an acyclic body excuses an invalid authored rank. Named-state,
mutable, custom-view, and call-component range transport require their own proof
support. Slice ranking shares validation's exact nonempty-slice/`1..` rule.

A declared `u64 -> u64` identity measure shares scalar range proofs only when
its body names its exact resolved parameter. The authored custom view remains
private witness identity; it is not relabeled `Nat::Descending`. Constrained
subjects retain their scalar carrier. Direct struct measures require an exact
parameter receiver, its owned field declaration with builtin `u64` carrier,
and the same nominal subject type before publishing even a precheck summary.
Nested projections cannot reuse a direct field's decrement. A single-state
direct-field measure can use its Exact field type's enforced finite bounds for
rank-range membership. Literal endpoints need no transport; declared immutable
Exact scalar endpoints also require direct identity-preserving forwarding on
every self-edge and complete prefix/operand write frames. A sufficient type range
does not permit replacing an invocation's endpoint. Construction, arithmetic and
strict descent are independent obligations; permissive Wrapping storage supplies
no such bounds.
Descent also requires complete write frames preserving the ranked field through
the preceding statements and edge operands; a reset cannot count as progress.
Relational or computed field endpoints, named-state transport, broader
projections and constrained measure parameters still need application/projection
evidence.

Computed state arrivals may use multiple copies of the same authored scalar
rank. Mapping discovery identifies that entry role; the arithmetic edge judgment
separately proves required copies equal on every arrival before using equality
as an invariant. Diverged copies cannot select a convenient representative.
These are private ranking premises, not ordinary store-range facts. Explicit
`self` targets remain current-state edges in both topology and occurrence readers;
the absence of authored arguments supplies no decrease evidence.

`checks/termination/progress/` replays structural qualification correspondence
before transporting entry subjects. Exact formation-state declarations and
Field/Case/literal FixedIndex paths must agree. Owned captures use source values
at the copy point; reference-bearing copies are not snapshots of referents.
Demanded owned-field partitions retain separate alternatives across named
states; growing cyclic projection paths become unknown rather than being cut
off at an arbitrary depth. Reference/array/recursive or unresolved generic
leaves remain conservative.

Runtime component inference uses the complete revalidated call graph, including
proof dependencies, and waits for required external summaries. Preserving-rank
edges must be acyclic; each remaining occurrence proves descent. Private inferred
progress never changes a published guarantee. Growing recursive-reference
premise demand retains `NoGuarantee`, not a truncated promise.

`checks/carry/` combines place liveness with possible suspension using the closed
evaluation schedule: receiver first, eager operands left to right, authored
aggregate-field order, short-circuit Boolean control, and one evaluated dispatch
subject. Nested suspending calls reject; blocking-only calls do not create
hidden continuation state. Terminal production consumes retained crossings;
target task activation and stack realization remain outside checked Psi.

## Mutation and reference origins

The shared implementation and conservative fallback rules live beside
[validation](../../semantics/validation/README.md#write-frames-and-reference-origins).
Flow invalidation projects its complete-or-opaque result into structured places;
do not create a second recursion or alias-admission policy here.

Flow construction borrows one call-frame resolver and one lazy mutation-summary
table across its value-input passes. Only preparation from the immutable program
and borrow facts is shared; each pass rebuilds its changing incoming value facts.
Each call's alias closure still queries its exact statement prefix, and
unavailable resolution remains conservative. A new invocation owns a fresh table.

`checks/borrows/persistent.rs` admits artifact-lifetime borrow storage and exact
persistent copies. Named-state must-analysis preserves stable field/case/index
paths only when every predecessor agrees; runtime index rebinding needs exact
immutable forwarding. Disjoint complete call frames preserve paths, whereas
unknown origins, overlapping writes, or missing predecessors retire them.
General parameter-backed persistent storage and loan-root rebasing remain
unfinished; a successful origin query is not authority to store a borrow.

`flow/ownership/result_storage.rs` distinguishes owned call-result storage from
caller places without crossing references or slices. `checks/multiplicity/temporary_results.rs`
rejects partial temporary moves leaving unselected linear claims. Complete
frontier transfer remains separate from nested-call executable realization.

Borrow recasts use the validated representation footprint, not ordinary
same-carrier cast traversal. Whole-name/member recasts retain the source place;
eligible literal byte-array offsets retain the complete half-open target range.
The shared sealed layout resolver owns exact primitive, recursively literal
array, closed-record, and replayed specialization eligibility. Symbol identity,
padding, extent, concrete const arguments, and bounded lifetime-shell context
must agree. Runtime offsets, unsupported generic/lifetime graphs, invariant or
quotient shapes, and footprints not representable by the retained path decline
the indexed form rather than becoming an element-sized loan.

## Lifetime source correspondence

[Source lifetimes](../../../../wiki/spec/language/lifetimes.md) owns the contract.
[view_link.rs](src/borrow/view_link.rs) supplies one shared result-source query
to declaration checks and loan attribution. It maps explicit result lifetimes
to one input parameter and its complete matching structural leaves, retaining
each result/source path and access. Reusing one lifetime on multiple input
parameters currently rejects; it is not implementation of a general
multiple-source return relation. Unannotated multiple carried sources also
reject rather than selecting one by name.

[Elision checking](src/checks/borrows/elision.rs) distinguishes incomplete
concrete frontiers from template-dependent trait requirements.
[Template call checks](src/checks/borrows/elision/templates.rs) permit the latter
only after exact selected callable/argument substitution proves the complete
result view-free; otherwise the still-uninstantiated call rejects, even when
discarded. Concrete view-returning instances retain the full input/result
check. General caller-side generic returned-view attribution and outlives
constraints remain incomplete. Persistent-field correspondence and recast
footprints follow the separate mutation/origin rules above.

## Checked callable custody

Normalized contract plans retain source-free crash buckets separately from
state/statement site evidence and state/statement/call invocation evidence.
An explicit empty published ceiling is positive evidence, not omission. Private
same-unit body summaries use a conservative call-graph fixed point: unknown
dependencies prune caller closure and recursive edges widen causes rather than
dropping them. Argument substitution removes a route only when proved false;
proved-true routes become unconditional and unknown routes retain their predicate
in the caller namespace. Incoming path conditions remain separate factors.

Crash-site claim frontiers are definitely-live lower bounds for diagnostics and
audit, not permission to execute survivors. Case claims need exact symbol-rooted
membership and composed state-argument maps at every case segment. Joins retain
only common polarity and substitution. Trait/boundary/static-machine calls use
checked callable capsules, not implementation-body inference. Separately compiled
artifact capsule input remains dependent on semantic import/export identity and
certificate binding; local custody is not that artifact format.

Nominal static-machine callback rows are keyed by exact authored use and machine
argument ordinal. They join the registration slot, selected machine/entry,
satisfaction requirement/overload, distinct requirement and actual contracts,
and refinement receipt. Duplicate equal observations collapse; conflicting rows
reject. Target placement rejoins the strong calling-plan commitment, not a
compact fingerprint or signature uniqueness.

Realized envelopes retain reach, invocation, suspension, blocking, termination,
crash, mutation, and capability facts with independent provenance. Per-entry
resource rows and callback receipts bind the same exact machine/entry/contract
to three downstream obligations: stack closure, control/fuel schedule, and
machine-state footprint. These checked rows contain no numerical resource bound,
target footprint, provider receipt, or installation grant. Admission still needs
independently derived downstream evidence.

Authored reach/invocation/suspension/blocking custody begins in typed exact target
and keyword spans; checked effects settle each published or inferred axis. Public
may-ceilings do not claim the body exercised its permission. Raw lifetime
declaration ordinals stay private; provider and package identities publish the
normalized equality partition instead.

## Remaining representation work

Checked value origins do not yet supply a complete ownership/drop/storage model.
Move/drop production still needs full transfer-site coverage beyond the current
assignment, initializer, aggregate, operand, call, and transition producers.
Broader diagnostic-backed acceptance records and backend boundary-policy joins
remain separate work; accepted evidence counts cannot stand in for them.

## Borrow evidence production

[resources.rs](src/checks/borrows/resources.rs) owns checked resource lifecycle
and restored-use planning. Resource and compatibility arenas remain separate.
Automatic non-interference retains zero-premise structural certificates with
formation coordinates, state-owned loan identities, frozen places, normalized
conclusions, and ordered selector snapshots. Replay normalizes the original
typed expression and reconstructs spatial relations and access compatibility.
This precursor does not implement general proof-premise admission or portable
compatibility evidence.

Direct reference-local reborrows require one exact prior parent. Resource rows
retain typed parent handles, activation, weakening, formation availability,
lexical end disposition, and separately checked containment. Rebuild/remap the
complete graph transactionally in loan order. Aggregate/helper transfers,
ambiguous or reassigned aliases, and write-only local loans cannot gain inferred
resource authority from these cases.

## Entry-contract readers

Entry requirements and crash routes retain incoming operands independently of
executable mutable storage. Strict readers rejoin exact live machine/entry-state
owners, symbols, parameter order, builtin type/operator meaning, and field paths.
Same-spelled locals or fields from another owner cannot supply a missing identity.
Unread structural operands and receivers still occupy authored positions;
dense scalar positions count only preceding primitive parameters.

Boolean entry readers admit direct owned formals and plain field paths through
eligible owned/shared/mutable roots. Explicit receiver paths rejoin the exact
machine-owned `Self` alias and nominal field identity. Fixed-integer comparisons
share literal landing, same-carrier construction, and numbered-field handling
with the integer-contract owner; write-only fields supply no readable hypothesis.
Comparison totality on policy-qualified inputs does not admit arithmetic, casts,
calls, or current body values as new entry facts. The Boolean-only result fallback
has a different namespace and must not be widened by sharing this construction.

Boolean polarity, compound equality, and common-consequence extraction use bounded
logical conversion (4,096 work units, depth 64), not eager runtime operations.
Conjunction combines facts; every disjunct must establish a retained common fact.
Keep original published predicate identity and exact entry ordinals. Exhaustion
does not restart the budget for another route. Selected user operators do not
inherit builtin meaning from spelling. Arithmetic-subtree, path, and case checks
remain separate from the logical expansion budget.
Bounds-guard operand recovery likewise checks the actual collection receiver,
length carrier, and builtin computation. A field spelled `len` keeps its declared
type; anonymous or unresolved operands do not inherit a sibling's type. Recovering
a type or finding a comparison declaration supplies no bounds proof.

The focused source-to-artifact regression is
[entry_requirement_crash_coverage.rs](../checked-trees-to-lowered-psi/tests/entry_requirement_crash_coverage.rs).
It verifies retained requirements and unchanged call routes, not execution of
arbitrary host records as entry proofs. Generic/lifetime attachments, broader
entry arithmetic, and mutable value-origin transport remain distinct work.

## Scalar convergence classification

[shared_convergence.rs](src/flow/terminal_unit/shared_convergence.rs) recognizes
sufficient source shapes for a shared Boolean cleanup tail. It is a producer
classifier, not proof authority or the definition of legal integer arithmetic.
The classifier families are organized by their actual expression structure:

| Owner | Recognized structure |
| --- | --- |
| `affine` | Ordered landed-literal offset/coefficient chains and bounded same-/distinct-root arithmetic joins. |
| `cast_chains` | Partial casts, strict widenings, and ordered conversion words between computed families. |
| `products` | Literal-factor multiply chains and bounded literal/runtime-divisor chains. |
| `shifts` | Ordered exact shifts and bounded cross-family/conversion compositions. |

These paths retain exact runtime parameter roots, carrier types, literal siblings,
source order, and independent operation identities. Recognizing a quadratic,
conversion, or cancellation shape does not establish that proof production or
native composition supports it. In particular, historical sufficient bounds
must not replace the operation-local questions in
[integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md).

Each arithmetic prefix and partial cast needs its own proof. A final zero factor,
cancellation, or representable conversion result cannot excuse an earlier unsafe
operation. Missing/reordered/cyclic definitions, changed roots or carriers, invalid
literal landings, and checked analysis overflow decline the candidate. A failed
bounded analysis is not a proof of mathematical falsehood.

Keep source-family limits here rather than adding a language rule per composition.
Extending a classifier alone does not extend artifact admission or erase a
downstream unsupported-shape rejection.

## Bounded Terminal correspondence

[restored-call production](../checked-trees-to-lowered-psi/src/reborrow_restored_call_use.rs)
admits a direct mutable parent with an exclusive child or one complete shared
cohort of one to three children. Final child use ends immediately before one
receiver-free whole-parent mutating call. Multi-member final observation uses
one exact shared-parameter call; earlier completed non-overlapping exclusive
siblings do not invalidate the event. Replay checks the complete cohort, both
resources, lifecycle/containment, captured places, restored access, callee shape,
and exact source call. Four-member/sequential shared cohorts, multihop or
projected restored use, direct assignment, and partial/nonmutating uses remain
outside this producer.

[root handoff](../checked-trees-to-lowered-psi/src/reborrow_root_handoff.rs)
separately admits a finite nonempty exclusive chain rooted in a direct mutable
or write-only loan and ending at state exit. Reverse and rejoin its exact
retired-parent path; reject branches and shared edges. This is direct-root
custody, not a cleanup,
transfer, or linear-discharge operation. Broader Terminal evidence must be
implemented explicitly rather than inferred from a successful checked replay.

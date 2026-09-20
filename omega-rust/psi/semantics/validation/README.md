# Semantic validation

Start at [program_validation.rs](src/program_validation.rs): declaration checks,
machine/state validation, and result publication run there in order.
[Statement validation](src/program_validation/statements.rs) owns ordered value
updates; [contract queries](src/program_validation/contract_queries.rs) owns
entailment queries and pristine-template checks. [lib.rs](src/lib.rs) wires the API.

Standalone validation checks generic contracts itself. Post-specialization
validation requires the earlier template check and explicitly selects required
opaque-property receipts or pending preliminary-build evidence. Its result
retains operational and service-reach analyses for the same immutable program;
checking consumes those analyses instead of rerunning their fixed points.
This is local analysis reuse, not portable verification authority.

Source automation must preserve the
[mathematical proof contract](../../../../wiki/spec/proofs/contracts.md) and
feed the separately reconstructed [Terminal questions](../../../../wiki/spec/terminal-psi/verification.md).

## Domain issuer migration

The [establishment contract](../../../../wiki/spec/resources/authority.md#establishment-routes)
permits exact machine routes and public-domain catalogs containing private issuer
requirements or machines. This does not describe completed compiler support.
Source route normalization, result-provenance checking, package exposure, and
artifact evidence must migrate together under `DOMAIN-ISSUER-ROUTES` on the
[execution board](../../../../TASKS.md). Existing requirement-only and private-route
rejections are implementation limits, not the settled language rule.

An exact issuer result must discharge carrier, predicate, and ordinary custody
obligations before route provenance is introduced. Qualification cannot prove
itself. Open requirement routes, boundary admission, installed parameter subjects,
resource conservation, and classification-specific restrictions keep their own
rules; a machine route is not blanket trust or a new capacity origin.

## Source proof automation

[contract_entailment.rs](src/contract_entailment.rs) handles a bounded contract
fragment: canonical integer polynomials, substitutions from equalities,
difference-bound closure, congruence, correlated intervals, signed remainder
bounds, and accumulator-style self induction. Each recursive hypothesis needs
strict descent at its exact edge. This is trusted source automation, not yet
the source-to-kernel certificate bridge.

The [proof pass corpus](../../../../tests/omega/pass/proofs) and
[false twins](../../../../tests/omega/fail/proofs) test this fragment;
[math_proofs](../../../../samples/cli/proofs/math_proofs) is a readable example.
Unsupported judgments are not proved because this engine stands down. General
quantified contracts, arbitrary mathematical functions/predicates, noncomputable
values, proof views, and broader recursive proofs remain separate work.

`PROOF-CERTIFICATION-BRIDGE` and `PROOF-CONTRACT-MIGRATION` on the
[execution board](../../../../TASKS.md) own portable production: per-component
well-foundedness, per-edge descent, exact normalization licenses, transitive
assumptions, and certificate-derived review. The [proof admission kernel](../proof-admission/README.md)
already has bounded recursion and normalization certificate checkers; that
does not establish that source automation emits them or that general
mathematical foundations are implemented. The
[selected foundation](../../../../wiki/spec/proofs/foundation.md) owns the
general core and its remaining profile joins. `PROOF-KERNEL-CORE` and
`PROOF-CERTIFICATION-BRIDGE` replace parallel general truth routes with one
checked term/declaration model; the bounded automation remains useful as a
producer, not an implementation precedent that changes the foundation.

## Arithmetic and arrival analysis

The source rules are [numeric values](../../../../wiki/spec/language/numeric_values.md)
and [state contracts](../../../../wiki/spec/language/state_contracts.md).
`bound_expression_meaning.rs` checks builtin meaning with original expression
handles and declared operand types before bounds readers interpret comparisons.
Boolean decomposition checks each consumed child; an authored operator in one
conjunct does not erase a sufficient builtin fact in another.

`arithmetic_domains/guard_narrowing/arrivals.rs` binds exact target parameters
before joining guards. Iteration begins with an overapproximation; stopping early
loses precision. Scalar argument snapshots and reference-write invalidation are
distinct. `parameter_bounds.rs` preserves one-sided unsigned bounds even beyond
the interval engine's signed endpoint width. Exact `u64` formation separately
checks the actual unsigned ceiling.
`invariant_bounds.rs` selects fixed carriers by builtin identity and retains
literal landing types and evaluates anonymous subtrees with the existing rational
evaluator. Its immutable expression queries check operand landing,
each intermediate's unsigned representability, and the signed `MIN / -1`
definedness pair shared by division and remainder before reporting bounds.
Direct integer fields on immutable owned parameters use their exact nominal
owner and retained field handle before contributing declared bounds. References,
locals, mutable parameters, and nested projections supply no snapshot-independent
field interval. The same arithmetic rules retain each field's carrier and policy;
neither a small final result nor algebraic cancellation excuses overflow.
Guard and entry-requirement readers check the selected equality or ordering
before treating a literal or singleton field as a primitive bound. An authored
`==` or `!=` supplies no builtin range or non-NaN fact merely through its spelling.

`struct_literals/guard_bounds.rs` passes the selected transition arm's numeric
facts to constructor checks only for exact immutable owned inputs and builtin
arithmetic. It validates field identities before narrowing. Unsupported operand
effects discard those facts for the whole target; references and mutable inputs
need their own write-frame evidence. Storage-width and declared-range checks
remain independent of termination. `enforced_integer_type_bounds` exposes only
finite bounds enforced by Exact integer storage and representable by the interval
engine's signed endpoints, not permissive policy ranges.

Strict arithmetic call implication admits immutable same-carrier scalar actuals
and checked Exact add/subtract/multiply trees. It substitutes actuals in the caller
namespace before binding any formal and consumes simultaneous surviving contexts.
Prior-call/instantiated postconditions need their own substitution; mutable
snapshots, projections, result-producing calls, and other policies do not enter
this adapter. This query does not discharge arithmetic formation or produce a
Terminal certificate.

`arithmetic_domains/ordered_values.rs` tracks exact storage, selected builtin
meaning, and value-call target/inputs. Relations rebind through explicit arguments,
intersect across arrivals, and retire on nested-input writes. Repeatability needs
effect/observation-free closure and input custody. Final predicate validation
also checks unconditional termination and crash freedom.

`call_cycles/runtime_ranking.rs` owns whole-component runtime tail-call ranking;
proof-only components require strict structural subterms at every resolved call
occurrence. Members may select a declared identity measure: the classification
in `ranking_range/identity_views.rs` (the body forwards its parameter, the
parameter, result and subject share one unsigned carrier, and every declared
range refinement covers the subject's enforced bounds) is shared with the
checked termination stage, the produced rank is the subject itself, and the
authored measure stays private witness identity -- members share the order
only through the same measure, never with `Nat::Descending`. A member may
also select a declared computation view (`declared_scalar_view` with a
`+`/`*` body): the produced rank is the body over the subject
(`RankingRangeMeasure::Computed`), strict monotonicity remains the only
reason the subject's descent stands for the rank's descent, the call
judgment proves membership, nonincrease or descent, and formation inside the
carrier on both sides of every call from the caller's own hypotheses, and
members share the order only through the same measure and carrier. No parallel edge may supply an unclassified call's certificate.
Members may issue component calls from subordinate states: the state's
discovered telescope aliases each carried entry role onto the site's own atom,
so authored subjects and endpoints keep naming the same value. Internal
arrivals and self transitions belong to the member's own witness, requires
clauses stay entry-site evidence, and mixed-range endpoint conservation still
only covers call sites at the entry state. A prefix store before a component
call is judged against the member's premise carriers
(`ranking_range_premise_symbols`: subjects, endpoints, requires-named inputs,
and range-constrained entries), located in the site state through that same
telescope. A slot sharing its role with another slot, and every role-carrying
slot of a mixed-range component, stay protected too: the site query installs
the copies' equality and endpoint conservation reads each entry input's arrival
atom. A mutable input or role-less slot outside those sets may be stored to,
while any store into a protected slot still invalidates the entry-relative
ranking.

`contract_entailment/ranking_range/` uses one arithmetic entry/edge judgment for
scalar ranks, slice lengths, and exact direct-field coordinates. A declared
scalar view beyond identity (`identity_views.rs`: `declared_scalar_view`,
`computation_body_shape`) admits a `+`/`*` body over its single parameter
whose operators select builtin meaning in the selecting machine's scope and
whose normalized polynomial is strictly increasing on the naturals; the
judgment then produces the rank from that body with the parameter bound to
the subject (`RankingRangeMeasure::Computed`) and proves, from the same entry
hypotheses, that the rank forms inside the shared carrier, so `{ value * 2 }`
on an unbounded `u8` subject rejects at entry. Subtraction, constant bodies,
authored operators, and widening carriers keep rejecting, and a nonlinear
body's goals exceed the engine's linear reasoning, so it rejects too. `fields.rs`
binds an immutable owned parameter's selected builtin `u64` field, not its whole
record or another same-named projection. `field_coordinates.rs` binds additional
authored direct `u64` projections as independent coordinates, including fields
of a second owned parameter. Reconstruction retains the nominal owner and
declared field identity, substituting every demanded field simultaneously.
The shared judgment proves simultaneous
endpoint pinning, next-rank membership, and strict descent; an entry-only
requirement is not silently renewed. `projections.rs` collects the expression
roots for metadata binding without constructing source expressions. Direct-field
transport currently covers root self-edges and direct or computed field/scalar
endpoints with independent formation and supported normalization. Mere membership
in a changed endpoint does not prove pinning. References, nested projections,
and named-state field mappings remain unbound here; `identity_views.rs`
classifies a nested projection path and the checked struct-view prover
handles nested and borrowed subjects outside this relational route. This is source automation, not
a Terminal custom-view certificate.

`ranking_range/telescope.rs` owns the entry-role discovery shared by the
state-edge and call-component judgments: identity arrivals anchor each
telescope, computed arrivals reuse an already-anchored role, and conflicting
proposals remove the state. The call-site query substitutes these roles as
atom aliases rather than as equality evidence; a duplicated role stays
unbound.

Closed anonymous arithmetic inside strict relational expressions shares the
ordinary rational evaluator. Only an integral final subtree becomes an integer
constant; this does not relabel typed division or discharge carrier formation.

The shared `ranking_range/requirements.rs` adapter also proves ordinary call
requirements with no ranking assumptions. Separate caller and goal engines
preserve their namespaces on self-calls; the goal receives one simultaneous
map of scalar, field, and slice-length actuals. Only surviving caller contexts
and store-enforced bounds become hypotheses. Calls and unknown operand effects
do not enter this immutable arithmetic adapter. Exact arithmetic formation
remains independently required. Its graph-coverage query recognizes only
completely readable authored scalar/length requirements, not opaque Boolean
or nominal facts ignored by the arithmetic graph proof.

## Write frames and reference origins

`expression_types/reference_values.rs` shares exact reference correspondence
between named bindings, projected fields and call results. Raw fixed arrays
can supply readable slice views with the same element type and permitted access.
Known reference mismatches cannot fall back to syntax-only name matching.
Mutable views do not erase array-carrier predicates; their invariant-window
obligations remain separate from shape correspondence. Call admission still
checks the originating binding and every enclosing reference permission.

`calls/write_frames/` owns one complete-or-opaque may-write analysis used by
ordinary state transfer, cycle equations, and public call/value/store queries.
`LocalWriteOrigin` distinguishes exact paths, absorbing collection-coarse origins,
and private scratch. Project origins before filtering private writes. A binding
replacement updates only that binding; overlapping owner and alias facts are
invalidated together without changing borrow access routes.

Public demand queries recover the statement prefix once. Raw call inference
does not recursively reconstruct its own caller prefix. Recursion guards cover
bodies, not finite caller-argument composition; unknown or recursive bodies stay
opaque. Cycle equations retain exact frames through supported bijective
write-capable argument transport, not arbitrary alias-changing backedges.

Assignment queries use one prefix result for both the direct target and its
alias closure. Binding replacement never closes over the former referent.
The located state and statement index also serve local-origin recovery without
another owner search. Before-statement, reference-result, and assignment queries
keep their distinct transfer rules; this is not a shared mutable replay cursor.

`value_expressions.rs` expands one level into a shared finite worklist. Every
eager receiver/index/argument/field/element contributes producer writes, including
unselected literal children. Primitive computations and concrete caller-isolated
record/case/fixed-array literals use exact contextual types. Each nested call
needs its own complete non-rebinding frame; calls below scalar computations need
caller-isolated results. No numeric nesting cutoff replaces type evidence.

The first index coarsens storage to its nearest collection; subsequent member
or index suffixes cannot regain precision. Each effectful index passes the same
call-tree checks. Declared types, numeric eligibility, and bounds remain separate
obligations. Compiler-owned slice views preserve backing-array origins, whereas
an identically named resolved method keeps its actual body effects. Unknown
builtin-like names never supply complete empty frames.

Boundary frames require one exact trait signature on the canonical receiver,
exact arity, and supported formal/referent storage. A concrete generic receiver
supplies the trait's symbol-keyed type bindings; method type inference extends
that environment. The frame includes receiver writes and exclusive arguments,
plus operand effects. Passing a reference-free value does not authorize writes
to the storage it was copied from.
Forwarded references need exact caller declarations or proven helper origins;
`mut` binding syntax cannot amplify reference access. Signature-only boundary
results provide no returned-place proof. Generic signatures remain unavailable
to fact-seeding consumers that cannot retain their substitution. For direct
field and parameter receivers, failed boundary selection stays opaque through
the fallback consumers, including when the receiver is generic. Nested receiver
forms remain subject to the unresolved-receiver limitations on the R5 board.

Aggregate transport retains declared Field/Case/FixedIndex reference leaves,
frozen origins, and selected/possible case evidence. A coarse array demand unions
reachable leaves; owned fields have no caller write, but missing demanded leaves
cannot become an empty frame. Slot/carrier replacement or mutable exposure
invalidates frozen relations. Payload writes do not replace their enclosing case.
Ordinary transfer, cycle equations, and public closure share this evidence.

`result_origins.rs` derives helper result leaves from the complete body transfer.
It requires an exact target, arity, normalized result type, and a final expression
or unconditional value transition. Substitute actual origins only after body
validation; helper writes remain separate producer effects. Frozen local copies,
input carrier moves, nested literals/helpers, and exact projections compose.
Possible input cases do not authorize a payload projection in the body; actual
case substitution replaces the complete proven result subtree, including absent
leaves. Conditional body refinement and graph-level result routes remain open.

`reference_subjects.rs` enables shared-leaf discovery only for exact storage
identity queries, not ordinary write caches. It retains one validated Field/Case
reference boundary from owned or one-readable-reference input carriers and frozen
locals. Each earlier operand must preserve the slot; referent writes do not
replace it. Unknown shared leaves stay explicit unknowns. Loaded extra reference
boundaries, indexed/coarse origins, missing declarations, and unproven helper
results cannot recover identity by spelling or type alone. Shared/exclusive
lifetime attribution and loan authorization remain separate checks.

Keep unsupported expression, generic/recursive carrier, binding-reborrow, and
unrepresentable-origin forms opaque. A successful frame or origin query is not
full source acceptance or executable Terminal realization.

## Quotient correspondence

The [published correspondence contract](../../../../wiki/spec/proofs/quotients.md#published-quotient-correspondence)
is implemented through [quotients/terminal_bridge.rs](src/quotients/terminal_bridge.rs)
and the relation-plan bridge. Current direct `define`/transport-backed `lift`
retention and proof-only package review do not admit executable quotient calls.
Adapted, permuted, repeated, generic/private applications and broader lift forms
still require complete correspondence and package-projection integration.
Do not retain schema-bump history as a substitute for those acceptance conditions.

## Structural algebra automation

[structural_judgment.rs](src/contract_entailment/structural_judgment.rs) retains
operation licenses and paired add/multiply semiring licenses. The paired form
requires both operations' associativity/commutativity and a conformed
distributivity law. Natural-coefficient polynomial expansion is bounded;
failure to normalize or different normal forms are not refutations.

Zero/one constructor bridging uses checked unfolding and citation, not an
unlicensed identity rewrite. Broader polynomial/index normalization, additional
Int/Rat structures, and additive/multiplicative group licenses need their own
checked contracts. Unit dimensions need additive group reasoning; positive
rational scales need multiplicative group reasoning, not integer linear
arithmetic alone.

The specialized entailment engine does not establish that arbitrary mathematical
function/predicate binders, nonconstructive values, or full foundation semantics
are implemented. Preserve useful derivations while replacing obsolete source
and evidence machinery under `PROOF-CONTRACT-MIGRATION` on the
[execution board](../../../../TASKS.md).

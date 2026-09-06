# Typed Trees To Checked Trees

[Pipeline](../pipeline.md) | Previous: [Symbol Resolved Trees To Typed Trees](symbol_resolved_trees_to_typed_trees.md) | Next: Checked Trees To Terminal Psi

This stage validates semantic obligations and builds the checked fact model used by proof, borrow, reach, and flow checks.

## Stage Contract

Input: `TypedTrees`.

Output: `CheckedTrees`.

Primary responsibility: validate semantic obligations and build checked facts.

The current compiler orchestration wraps that checked program in one
`CheckedProgramSurface`. At the ownership-moving boundary it retains the exact
Accepted-only machine order and each machine's optional normalized generic-
template report fingerprint plus the domain-separated commitment to its
canonical pre-substitution contract. The same phase surface captures exact
machine, contract, fact, and closed-reason rows whenever checked-implementation contract
entailment stands down on the pristine typed predecessor. Trust reporting and
package review consume those retained phase facts; the driver does not keep a
separate typed-tree snapshot or courier raw typed-derived rows around checking.
Missing or duplicate template-classification rows reject, and package review
rejects every unresolved stand-down. The narrow identical-assumption discharge
is separately projected as canonical source-handle-free evidence only after an
independent semantic/kernel recheck; it grants no accepted package authority.

Generic specialization retains the canonical pre-substitution template bytes,
normalized template/instance identities, exact type and const arguments,
selected machine-contract commitments, closed-conformance commitments, and any
accepted-template grant. This stage binds those inputs into a domain-separated
SHA-256 `MachineSpecializationCommitment`. Checked-to-Terminal lowering replays
that commitment from retained typed custody and checked contract plans before
using all 32 bytes as proof-producer identity. The adjacent aggregate FNV is a
diagnostic and cache report coordinate only.

Owner-policy admission uses the template commitment, not that compact report.
The persisted trust digest separately frames the admission subject kind and
human commitment, so a generic template, ordinary checked machine contract, or
selected provider plan cannot settle against a compact-equal substitute.

Checked per-entry resource envelopes and callback resource receipts retain the
same machine-contract commitment beside their compact axis and roster reports.
Reconstruction therefore rejects a compact-equal contract substitution before
the receipt crosses into provider or backend planning.

Target-dependent callback closure crosses this boundary as one explicit
`TypedToCheckedSettlementInput`. Before checked Psi enters shared ownership,
the transition closes the exact boundary calling-plan realizations and
validates their nominal callback placements. It then transactionally binds the
selected provider receipts and returns the settled provider facts plus callback
placements on `CheckedProgramSurface`. The compiler coordinator consumes that
complete result; it neither recovers unique ownership with `Arc::get_mut` nor
replaces the checked program with an out-of-band provider settlement.
Preliminary package-selection validation uses a separate checked observation;
it does not fabricate a final surface with absent settlement fields.

The next consuming transition owns selected execution as one ordered
settlement. It constructs component progress from the exact retained entry
root before any execution redirection, settles operator and float dispatch,
retains compiler-intrinsic package-review provenance, settles boundary-adapter
dispatch, and only then elaborates task activations from the rewritten call
tables. `SelectedExecutionSettlementSurface` returns the final checked program,
provider facts and provenance, callback placements, component progress, task
activations, accepted-template classifications, and entailment stand-downs.
The compiler coordinator does not courier or mutate any of those results.

The selected-dispatch owners also return `SelectedDispatchSourceEdits`, retained
privately by `CheckedCompilation`, not in package policy or review identities.
Each owner seals its actual replaced expression and statement calls together
with exact reachable operand, binding, and type custody before publishing the
staged checked program. Source-semantic queries validate those batches in
reverse settlement order and restore only the replaced nodes in one shared
scratch typed tree. They borrow the current tree when no edits exist. This is
the pre-selected-dispatch view, not pristine source or a pre-specialization
snapshot: unrelated source edits and appended generated declarations remain.
In particular, package mutation review rederives the complete source write
frame through that view and still requires exact equality with the checked
frame. Dropping a selected service receiver from executable dispatch does not
erase its source write effect, and restoration does not conceal a changed
untouched source write.

## Semantic Ownership

This stage is the first durable semantic fact owner. It should be the place
where source/type meaning becomes queryable evidence for proof, borrow, flow,
reach, and boundary validation.
The representation root is `CheckedTrees`: typed syntax remains under `typed`,
while durable semantic evidence lives under `CheckFacts`. Checked flow evidence
is grouped under `FlowFacts` roots for contexts, invalidations, borrow
lifetimes, ownership, boundaries, and control. Root construction should flow
through `CheckedTrees::with_roots`, `CheckFacts::with_roots`, `ProofFacts::with_roots`,
and `FlowFacts::with_roots` so later stages can see the semantic spine at a
glance. `CheckedTrees::state_acceptance` is the first unified query doorway over
that evidence: a checked tree exists only after diagnostics are clear, and the
acceptance views expose the proof, borrow, boundary, reach, invalidation, and
call/exit evidence that made each state operation admissible.

Declared receiver typing retains case-tagged payload projections, including
payloads rooted at `self` and ordinary fields reached through them. It selects
the payload under the receiver's nominal declaration and exact case; flattening
the path into field names cannot select a different case's same-named payload.
This supplies a declared type, not evidence that a call is valid or terminating.

Normal returns owe their postconditions whether they produce a value or Unit.
An implicit return is positioned after the final statement; a transition exit
also retains its exact target handle so a returning continuation cannot share
the other arm's facts. Named state jumps are transfers, and crash edges are not
normal returns. Exit proof consumes the facts live on the returning path, not
the guarantees it is trying to establish.

Machine `requires` assumptions are scoped to the generated entry state, not
the machine-wide context. Internal jumps check the target state's authored
requirements without reapplying machine-entry contracts or publishing machine
return guarantees. Actual invocations retain both contract sides. A `self`
back-edge to entry must re-establish its machine preconditions; a named-state
back-edge owes that state's own arrival requirements.

Bounded integer returns consume the shared arithmetic arrival query in
`validation/src/arithmetic_domains/guard_narrowing/arrivals.rs`. Each source
argument binds to its exact target parameter before incoming guards are joined;
matching parameter names do not transfer facts. Calls, named transitions,
continuations, and backedges contribute their evaluated arrivals. Iteration
starts from an overapproximation, so stopping before convergence loses precision
rather than assuming an inductive invariant. Scalar arguments retain their own
evaluation snapshots; reference facts must survive later argument writes. The
return query crosses the consuming state's prefix and expression write frames
before exposing a bound. Unknown or overlapping writes retire the affected
facts. Authored requirements remain body-checking assumptions whose callers
must independently establish them.

`arithmetic_domains/ordered_values.rs` retains ordered operands by resolved
storage identity, builtin computation meaning, and exact value-call target and
inputs. Its temporary relations are rebound through explicit state arguments,
intersected across arrivals, and retired by writes to any nested input. Repeated
calls require the shared effect and observation-free closure plus concrete
value-input and builtin-operator custody; an empty write frame alone does not
establish repeatability. A subtraction consumes the live relation without
widening its independently computed upper bound. Machine-entry requirements do
not apply to later same-spelled parameters.

Normal-return equality does not establish total fact denotation. Final checking
validates eligible scalar calls in authored requirements against the finalized
unconditional termination facts and absence of crash routes. A runtime guard
may instead compare repeated results conditional on normal completion, while
its invocation retains the ordinary crash obligations. Broader call adaptations
remain outside this concrete value-call analysis.

Integer bitwise range analysis retains the exact signed or unsigned carrier
and operand arithmetic policy. AND, OR, and XOR transfer bits fixed throughout
each operand interval and then recover a numeric hull; evaluating only the
interval endpoints is not sound for these operations. Complement reverses the
carrier-bounded interval through the shared integer evaluator. Full-width
unsigned constants remain exact even above the signed interval window.
Bitwise results never acquire Boolean bounds, and both integer complement and
Boolean negation validate arithmetic nested in their operands. Bitwise
operations themselves impose no overflow-policy condition; surrounding
arithmetic still consumes the retained operand policy.

Builtin remainder formation is checked before value or proof validation. A
wholly anonymous numeric operand tree does not obtain integer meaning from a
destination or a constant fold, including when an intermediate division is
fractional. The check covers retained range bounds and contract expressions as
well as executable values. Declared operator candidates and typed leaves retain
their ordinary selection and typing boundaries; this is not a new evaluator for
proof-level `Int` or authored const operators.

Exact `u64` arithmetic checks the actual unsigned carrier ceiling separately
from its i64-backed interval projection. An unknown projected ceiling is not
proof of representability. Direct unsigned literal bindings retain exact
values in the flow environment, subject to the same write invalidation and
arrival intersections as other value facts. Landed integer literals retain
their own carrier inside larger expressions, including beneath negation or a
wider destination.
Resolved named operators contribute their declared integer return carrier,
policy, and range before provider selection. This signature information does
not establish a body result, purity, or repeatability.
Strict live ordering also proves a bounded unsigned increment below its
ceiling. Builtin slice/array lengths retain the exact collection operand for
this relation and rebind through explicit state arguments; nominal fields use
their actual field declaration rather than an occurrence's accessor symbol.
Neither a non-strict bound nor a stale pre-write relation supplies the missing
distance. Anonymous comparison literals retain exact mathematical values
without acquiring a guessed carrier.

Local integer guards also project immutable operand intervals through builtin
ordered comparisons. `guard_narrowing/parameter_bounds.rs` uses the existing
`invariant_bounds.rs` owner, retaining one-sided carrier bounds: an unrestricted
`u64` still has a zero floor when its ceiling exceeds the interval engine's
endpoint width. Thus `n > floor` with unsigned `floor` proves `n >= 1` without
requiring a duplicate literal guard. Projection uses exact state-parameter
symbols, preserves guard polarity, intersects the subject's declared range,
and checks selected operator meaning. Mutable operands and same-spelled
foreign parameters cannot supply immutable bounds.

Arithmetic requirements, Boolean guard wrappers, and index/loop-bound readers
retain selected operator meaning before interpreting primitive comparisons or
bound arithmetic. The shared `validation/src/bound_expression_meaning.rs` query
uses the original expression handle and declared operand types; a false branch
does not substitute the complemented token when checking operator identity.
Boolean and explicitly landed integer literals retain their actual builtin
types. Unknown computed operand types remain conservative candidates. This
meaning check grants no range, effect, or lifetime evidence by itself.
Boolean decomposition checks each consumed child independently: an authored
comparison in one conjunct does not erase a sufficient builtin bound in another.

Selected-arm range checks feed bare dispatch conditions and Boolean-wrapped
subject guards through that same analysis. The later bounded-argument proof
also preserves logical-negation polarity. Guard and argument writes still
retire stale bounds at their evaluation points.

Loop-counter monotonicity also retains arithmetic policy. Wrapping updates
need a no-wrap proof at their actual statement snapshot; the sign of a literal
step is insufficient. Entry constants and incoming guards establish independent
head bounds, and normal-result intervals transfer through subsequent updates
and named-state arrivals. A widening endpoint becomes unknown rather than
assuming convergence. The proposed monotonicity fact never supplies its own
no-wrap premise. Direct counter identity follows the exact attached self field,
and other stores must have a complete disjoint write frame.

Scalar exit proof binds the synthetic `result` to the exact final expression
or selected returning arm. Authored output parameters instead follow retained
per-contract reference origins into that state. Domain and scalar proof share
the live assignment-value lookup and Boolean/comparison evaluator; integer
literal comparisons retain full precision. Neither consumer replays local
initializers. Scalar exits also check selected operator identity and require a
complete empty write frame before evaluating a return expression from exit
storage facts. Arithmetic and effectful return values need their own evaluated
snapshots, not mathematical reinterpretation or post-effect operand reads.

Reference origins across named states are a finite, entry-reachable dataflow
calculation. Renaming and identity-preserving loops retain the entry subject;
conflicting, unknown, or rebound references do not. This transports the subject
of an obligation, not the truth of an authored entry assumption. Rebased facts
stay scoped to their exact state or exit. Global and machine declaration
contexts are shared, never populated with a sibling state's derived facts.

A `self` back-edge must re-establish both authored arrival contracts and
declared field domains. It consumes the exact selected arm's live contexts
after guard effects, including that arm's guard polarity. These snapshots share
immutable fact-reference spans; they do not reintroduce entry assumptions.
Raw field qualifications use the same proof checks as contract qualifications,
and evidence markers are not automatically satisfied obligations.

Index checking distinguishes a proved scalar access, a proved range window,
a rejected obligation, and an unsupported collection shape. An unsupported
shape does not establish bounds. Nested arrays use the selected element type's
extent, with a separate check for each indexing occurrence. Calls retire range
premises through their caller-visible write frames before later expressions
reuse them; unknown effects retain no value-dependent range premises. These
bounds checks do not grant element-domain, borrow, or mutation authority.

Collection-relative upper bounds and endpoint ordering do not prove that a
signed operand is nonnegative. Unknown-length scalar accesses and all range
windows also require lower-bound evidence from the operand's enforced type,
constant value, or live facts. A nonnegative start and endpoint ordering can
prove the end nonnegative; omitted endpoints retain zero/length defaults.
Known-array scalar checking retains its complete bounds judgment. Writes
invalidate value-dependent lower bounds before later accesses consume them.

Indexed syntax can match fixed-array or slice storage to a shared slice
operator parameter. Only the collection shell adapts: the element binding,
remaining operand types, domain participation, and ambiguity checks remain
exact. Named calls do not acquire this conversion. The checked operator use
and its closed boundary application retain the selected declaration and element
argument. Bounds checking consumes that same selection and its actual bound
contract; another declaration with the same token cannot supply the premise.
Additional requirements must be discharged independently or reject. Authored
selection finalization remains a separate exact-occurrence gate.

Named-state range facts are a fixed-point calculation rooted at machine entry.
Every reachable incoming edge contributes: an unknown value removes a constant
or exact length, and an empty relation set participates in the intersection.
Minimum lengths remain lower bounds, not exact extents. Each pass rebuilds the
edge contributions, and unconverged inference is withheld. Raw incoming guards
may refer to shared machine storage; parameter facts instead follow explicit
argument bindings, never matching names in different states.
Both range walks seed machine requirements only at entry and authored state
requirements at their declaring state. Assignments retire stale scalar and
collection bounds through one shared invalidation rule before replacement
facts can flow into a later state.
Declaration initializers are not seeded as named-state integer invariants;
only live values transported along explicit edges can establish those facts.

Literal assignment values belong to the common semantic fact contexts. They
are attached to exact stable places and use the same mutation invalidation as
domain facts; unknown writes supply no value and uninitialized storage supplies
no assumed zero. Domain predicate checking can use these live values. Existing
arithmetic and structural entailment contributes only explicit positive results
for exact machine and expression identities. An unjudged contract, an admission,
or a normalized content equation is not a proof result.

Assignment evidence is captured after right-hand-side call effects and before
the destination write invalidates its old facts. Whole copies and constructors
transport live field predicates to the destination; byte concatenation uses the
shared predicate law. The resulting facts name the new storage, not mutable
source expressions to replay later. Independent declared field facts have
separate contexts, while a proof with multiple dependencies remains coupled.

Exact `satisfies Trait<'...>::requirement` edges retain raw declaration-order
ordinals into the realizing machine's lifetime telescope under that embedded
typed custody. Checking requires complete in-scope target-trait applications
and substitutes them through the requirement parameter and result types,
including nested references and generic lifetime applications. Provider and
package-review identity never publishes those private ordinals: it retains only
their first-occurrence-normalized equality partition, shared by checked and
opaque external supply.

| Noun | Ownership |
| --- | --- |
| Places | First strongly useful place layer via `facts::Place` and checked-flow `CanonicalPlace`. |
| Values | First checked value fact layer via `CheckedValueFacts`, keyed by typed expression handles and value origins. |
| Facts | First-class fact contexts, origins, payloads, proof obligations, and contract facts. |
| Loans | First-class borrow facts, accesses, loans, activations, weakenings, and overlap checks; destination work joins canonical captured-place compatibility certificates without merging them into proof custody. |
| Moves | First-class checked-flow event arenas/spans exist. Initial producers are type-aware for direct assignments, local initializers, indexed element reads, aggregate literals, binary/range operands, by-value direct-call arguments, nested expression-call arguments, and transition target arguments. |
| Drops | First-class checked-flow event arenas/spans exist. Initial state-exit local drop producers skip copy-like scalar locals. |
| Calls | First-class call facts for contracts, borrows, flow, reach, and synchronous invocation. |
| Transitions | Checked for proof/arguments; ownership transfer needs more explicit data. |
| Reach | Direct/transitive reach and direct synchronous invocation facts are available. |
| Boundary edges | First-class checked-flow events for calls into states supplied by boundary trait signatures. |
| Semantic dependencies | Exact package-neutral declaration dependencies for carried nominal identity, layout, ownership behavior, and compiler-selected cleanup, with private/public disposition. |

## Ownership Rules

State values follow chapter 4's explicit parameter frontier: transitions name
the values delivered to the target state. Executable reads, writes, and call
receivers use the exact current-state declaration and preceding local bindings;
state arrival contracts use only that state's parameters. Entry and sibling
bindings are not recovered by spelling. Declared `self` attachment remains
explicit, while qualified operator namespaces are declarations, not storage.
Explicit state arguments retain nominal member identity without requiring
physical copies. Checked member selection does not recover entry-local types
for a receiver absent from the current state.

Must own:

- Proof obligations and whether current facts discharge them.
- Literal destination checks use exact resolved storage, parameter, and return
  types. Integer and float suffixes retain their chosen type or format through
  statement calls, expression calls, state transitions, and returned values;
  incompatible destinations reject rather than erase the suffix. Anonymous
  float arguments land at the selected parameter's format, excluding implicit
  receivers from the argument pairing. Direct terminal expressions and
  transition-value returns likewise use the declared result format. Explicit
  casts remain conversion boundaries.
  Anonymous fixed-integer arguments retain exact rational intermediates until
  the selected parameter requests an integral, in-range value. Bounded call and
  named-state proof use that same landing query. Width custody for ordinary
  resolved calls is attached to the exact call/argument edge, excluding implicit
  receivers and unsupported shared uses; named transitions consume their resolved
  target rather than recovering it by spelling.
  Scalar exit checking can transport a closed retained operand through an exact
  call's unconditional builtin equality between its result and an immutable
  fixed-integer formal. It joins the call occurrence, guarantee owner, formal
  symbol, and argument plan. A live local result may retain that call occurrence
  as assignment provenance, but it is not literal evidence and does not authorize
  source evaluation. Overwrites retire the provenance; runtime argument snapshots,
  mutable-formal identities, and more general result contracts need their own
  retained value evidence.
- Landed float values retain their format at scalar destinations, including
  parameters, storage, and returns. Named values, field/index projections,
  resolved call results, and explicit cast outputs cannot implicitly change
  between `f32` and `f64`. Binary expressions require the selected operator's
  result type, not an inference from operand formats. The destination checker
  consumes finalized root/domain/trait selection and only derives a builtin
  result from operands when that builtin meaning remains selected. Exact call
  parameters, storage fields/elements, and returned values supply destinations;
  named conversion outputs keep their own declared format. Integer narrowing
  and direct literal suffix checks retain their separate obligations.
- Borrow facts, accesses, loans, activations, weakenings, and overlap failures.
- The two-ledger borrow join: resource facts retain owner lineage, polarity,
  temporal containment, and restoration, while proposition-derived rows prove
  only relationships over already-existing captured places. The complete valid
  proof context participates in one compatibility judgment; literal,
  symbolic, domain, arithmetic, and theorem solvers are derivation tactics, not
  separate fallback obligation families.
- Loan-formation certificates keyed by the exact authorized event, captured
  place occurrences, normalized relation, premise fact tokens, and derivation.
  Premises must dominate and be valid at the captured versions, and cyclic
  authorization rejects. The certificate never manufactures or widens loan
  authority.
- The exact shared, write-only-exclusive, or read/write-exclusive access mode
  of every loan. Write-only checking admits only content-independent
  projection and non-observing writes, composes the restriction through calls,
  and retains exact outcome write footprints for dependent-fact invalidation.
  A direct checked-call argument may retain one exact `Field`-only write-only
  subloan when the existing common-field leaf referee and the one-parameter,
  single-state checked Unit call shape both hold. This does not admit general
  projected expression formation or reusable local-reference aliases. The same
  call shape accepts one direct write-only literal fixed-array parameter root
  projected by a finite nonempty suffix of ordered in-bounds literal
  `FixedIndex` segments through recursively literal fixed arrays, either
  directly or after the eligible field prefix, to an unrestricted non-Atomic
  primitive leaf. Checked and Terminal replay retain the complete suffix.
  Recursively literal fixed arrays whose ultimate elements are unrestricted
  primitive scalars or eligible material nongeneric, invariant-free `[copy]`
  records or sums admit whole replacement, static length metadata, and literal
  or ordinarily proven dynamic element stores through direct roots and eligible
  plain-record paths.
  Aggregate elements remain atomic array positions: mutation and caller-visible
  frames retain exact `FixedIndex` or conservative runtime `Index` identity
  without child fields, cases, or payloads. Statically normalized closed ranges
  over the same arrays admit exact-width array-literal replacement and retain
  the normalized half-open element-ordinal `FixedRange`. Atomic, qualified,
  constrained, generic, noncopy, erased, symbolic/open range, slice-range, and
  nonliteral forms remain fenced, as do matching, sum projection, and indexing
  inside an already selected nested-array element. Each nested array is one
  atomic outer element; this creates no executable Terminal write authority.
  An eligible plain-record path may also replace one whole nongeneric,
  invariant-free `[copy]` record leaf as a single freely discardable value. Its
  mutation/frame place ends in exactly one `Field` segment and never decomposes
  or observes child members. A closed material nongeneric, invariant-free
  `[copy]` sum may likewise be replaced atomically through a direct root or as
  the final leaf of such a record path; the incoming value supplies the whole
  tag and payload, and the retained place is only the root or final `Field`.
  Affine/linear, generic, qualified, invariant-bearing, quotient, erased, and
  array-of-record leaves remain fenced, as do sum arrays, case/payload
  projection, member observation, matching, take, swap, and read-modify-write.
  The executable whole-root primitive lane accepts direct integer, IEEE float,
  and Boolean literals. It also retains one exact native fixed-integer or
  Boolean scalar parameter as a runtime replacement when the structural
  destination and scalar source are the machine's complete parameter partition
  and have the same primitive type. One immutable local may instead bind the
  primitive result of an immediately preceding, scalar-only ordinary checked
  call or exact selected boundary-operator realization. The Unit plan retains
  that producer as an explicit `ScalarCall` or `SelectedOperatorScalarCall`,
  with its dense result binding, exact target state, complete contract
  commitment, service reach, and scalar arguments; the selected form also
  retains its exact requirement and ProviderPlan join. A later Unit call may
  consume an ordinary result local without reconstructing its expression. The
  exact two-statement sibling may instead assign either result local directly
  into the machine's sole whole-root unrestricted mutable or write-only
  primitive parameter when both sides are the same native fixed-integer type.
  Further arithmetic locals, structural or
  service-bearing scalar producers, Boolean and IEEE call results, IEEE runtime
  parameters, and additional parameters remain fenced.
- One exact fixed-integer literal, same-typed fixed-integer or Boolean
  parameter, or immediately preceding fixed-integer ordinary/selected call
  result assignment may instead target a relevant
  primitive field below the machine's sole mutable or write-only structural
  parameter. Before the optional literal index, every carrier segment must be
  a common record field in a plain, invariant-free, nongeneric, nonquotient data
  shape. The complete mutation frame must contain exactly that destination, or
  the sole containing-array field for the indexed form. The checked operation
  retains the parameter position, ordered carrier-field identities, final
  field identity and primitive type, and the exact source expression. A direct
  root field has an empty carrier path; a nested record field retains every enclosing
  field. One additional form retains a common-field prefix followed by one
  in-bounds literal fixed-array index and a final relevant primitive field of
  its closed material `[copy]` record element. The caller-visible mutation
  summary remains conservatively rooted at the containing array field while
  the executable store carries the exact index. No referent read is introduced.
- Reach summaries, invocation edges, and boundary contract facts that later stages must preserve.
- Bounded installation-row facts keyed by exact boundary-requirement path,
  including the declared service upper bound, symbolic dependencies through
  the owning installation closure, and rejection when an unresolved row would
  escape into an ordinary callable package or component contract.
- Nominal static-machine callback-use facts keyed by call site and machine
  argument ordinal, including the selected machine/satisfaction row, exact
  requirement overload, separate published and actual envelopes, their
  refinement judgment, and the target placement recipe. Signature matching or
  visible uniqueness never manufactures such a fact.
  Pre-specialization selection validation derives service reach while its
  operational inference plan is transient, then projects machine-keyed
  suspension and blocking rows and threads those axes independently through
  callable-shape refinement. Missing inferred rows fall back to the matching
  authored axis only; suspension never supplies blocking or vice versa. A
  transient call row whose exact typed path and arity select one named operator
  retains that operator symbol separately from machine/state targets. Early
  build-time admission rechecks the operator owner's package authority; an
  ambiguous named operator remains unresolved rather than becoming a fake
  machine edge.
- Checked value origins for ranking witnesses, initializers, statement values,
  call arguments, transition guards/targets, and nested expression children.
- Checked scalar expressions for named-transition arguments skip an implicit
  `self` target parameter but retain each explicit argument's raw target
  position. Later executable plans may use dense scalar carrier indices, but
  they must rejoin this source-position coordinate rather than renumbering it.
- A durable checked-flow representation of calls and transitions.
- Exact positional binding from each named-transition evidence identifier to
  the target state's witness-bearing arrival requirement after ordinary
  transition-argument substitution; enclosing machine evidence stays live.
- Closed generic-conformance applications keyed by their declared
  package-scoped symbol, with the complete normalized telescope, instantiated
  subject and trait application, and selected row map. Non-lifetime arguments
  are explicit; ordinary lifetime elision is resolved before the fact is
  published.
- Finalized authored-selection custody for checked calls, members, operators,
  and conformances. Operators with no declaration-spelling surface—logical,
  bitwise, and shifts—finalize as compiler intrinsics after ordinary type
  checking, including when their operands are nested expressions without an
  independent value-origin type reference.
  Exact checked wire-schema `Schema::encode(..)` and `Schema::decode(..)`
  statement calls likewise finalize as distinct compiler-owned intrinsics;
  the schema and value type remain separate nominal selections, so this does
  not manufacture package declaration authority from the codec spelling.
- Exact inferred-conformance custody for generic specialization and trait-backed
  operators. Unique unbound generic-bound validation retains the selected
  package-scoped symbol in the machine specialization, separately from explicit
  evidence arguments, and includes its package-qualified identity in the
  specialization fingerprint. Finalization attaches that inferred selection to
  the authored call token; trait-backed operators attach their checked selected
  conformance to the operator token.
- Finalized statement-call custody joined through exact checked flow-call
  coordinates. Late receiver/result/generic targets resolve at the retained
  source target span, and inferred generic conformances attach there. Closed
  compiler-owned build markers and lowered assembly operations finalize as
  typed intrinsic selections rather than invalid declaration symbols.
- Exact carried-semantic-dependency custody assembled after successful checking
  from machine heads, checked call targets, ownership places, and automatic
  cleanup. Cleanup machine selection joins the exact attached nominal symbol;
  presentation spelling is never enough. These compiler-private handles feed a
  later package projection and are not a persisted evidence format.
- Producer-side custody for each outcome-specific guarantee, separate from the
  unconditional contract-fact lane. The checked row retains exact machine,
  result-data, result-case, public-selector, proof-fact, and optional evidence-
  term identity. Package review may rejoin that internal row after successful
  checking, but must not infer guarded publication from source text or install
  it as an unconditional postcondition.
- Complete operator-crash custody. Checked lowering retains one row keyed by
  exact operator symbol for every root and domain-homed declaration, including
  an explicit empty ceiling. Cause-normalized buckets retain unconditionality
  or exact typed proof-fact joins. Package review rederives this compiler-
  private table, then projects guarded facts through its package-qualified
  structural expression vocabulary; runtime predicate display fallbacks do not
  become operator API identity.
- D29 boundary-operator application demand. Each checked use retains the exact
  selected requirement telescope and an ordered typed application rather than
  arity plus display strings. Type arguments retain structural checked type
  custody; a later owner-aware stage freshly derives exact normalized identity
  rather than trusting a cached string. Const arguments use the canonical
  evaluated value in the declared carrier. A use inside generic code may
  retain exact references to its
  enclosing typed binders, but that row is symbolic demand only. Provider-plan
  selection, final substitution, role-specific realization checking, and
  physical-plan coverage occur in later owners and cannot be inferred here.
  Lifetime, static-machine, and proposition operator applications remain
  explicit fences.

  The implemented first cohort is narrower than the completed D29 contract:
  monomorphic selected boundary-operator uses retain the canonical empty
  application, while spelled and named operators retain complete closed inferred type/const
  bindings keyed by requirement owner, category, and declaration ordinal. One
  structural operand judgment is shared by spelling selection and application
  derivation, including repeated and nested fixed-array const positions and
  synthesized generic-data origins. Explicit type and integer const arguments
  must exactly corroborate the complete operand-derived application; they do
  not fill missing binders. Type arguments satisfy every declared
  copy/linear/carry property bound. Const arguments retain a display-independent
  canonical evaluated value beside the exact declared carrier, which validation
  decodes and checks for carrier identity and integer range. Validation retains
  the exact expression or statement use and selected requirement; checked
  lowering replays the declaration telescope, bounds, const carrier/value, and,
  for retained named expression uses, reconstructs the application from the
  selected operands. Property lookup prefers exact nominal symbols; a
  same-spelled foreign declaration cannot supply a bound or const carrier.
  Unit-returning named statement syntax is
  normalized to a generated expression while preserving its authored call
  occurrence and source span. No display identity, digest, or marker that an
  audit happened participates.

  The first structural-operand Unit composition retains a separate
  `SelectedOperatorStructuralScalarCall` row. It accepts one primitive local
  initializer in a free Unit machine only when the selected realization has no
  scalar parameters and the authored operands are an exact permutation of all
  direct, whole, claim-free owned affine source parameters. Source and
  realization structural shapes must agree before the caller adopts the
  realization's specialized carrier identities. The row retains the exact
  provider-plan and realization contract commitments plus empty service reach;
  consumed roots do not reappear in the Unit-return discard frontier. Content
  evidence, contracts on the caller, paths, borrows, services, mixed operands,
  and structural results reject rather than falling back to scalar planning.

  Open caller binders, nested open arguments, return-only or forwarded consts,
  unresolved nominal identity, named explicit const declarations, and
  constraints without a closed structural replay remain unavailable rather
  than being mislabeled concrete. D29's mention of `where`
  requirements means requirements expressible by the operator model; this
  work does not incidentally invent a general operator `where` surface.
  Missing rows are not coverage and cannot be filled from the pre-D29 indexed-
  provider scaffold.

  Omega's selected-provider owner now supplies the exact generic checked-body
  requirement/provider symbols before final checked lowering. Psi alternates
  ordinary generic-machine specialization and selected-provider specialization
  to a fixed point while retaining an immutable authored provider template.
  This closes type/const applications that become concrete only inside an
  ordinary single-instantiation specialized helper, nested expression tree, or
  newly specialized selected-provider body. Open applications on authored templates remain
  symbolic non-coverage and are never used to mint a specialization. Psi
  independently rederives every closed application from the concrete authored
  operands and clones one private authoritative provider specialization per
  distinct application. Each specialization retains the exact closed operator
  realization and commits it with its template, substitutions, selected
  conformances, and machine contracts. Package review rejoins that custody to
  the strong selected plan. Canonical-empty
  checked-adapter applications use the same exact join for named and fixed-
  token uses in ordinary value machines; an attached-`Unit` plan supplies an
  independent consistency derivation when present. Nested machine applications
  remain rejected by language checking rather than becoming package-review
  work. Fixed-token generic type and const applications enter the same
  authoritative specialization and exact package-review join as their named
  counterparts. The implemented local closure does not define an exported
  symbolic-demand format or a universal generic provider proof. External
  generic, symbolic cross-artifact, remaining unsupported telescope categories,
  Terminal companions not supported by the concrete call form, and D32
  physical-child work remain fail-closed.

The package projector reads each fact from the earliest coherent compiler-owned
representation in which that fact is semantically complete, then joins checked
acceptance only after compilation succeeds. `CheckedTrees` is therefore one
possible source, not a mandatory collection point. This internal coupling may
move with the compiler; it does not make private IR a package format or justify
a nominal `Chi` stage. Add a stage only for a genuine reusable semantic boundary,
and prefer an existing coherent representation such as `Exact` when it preserves
the required meaning with less machinery.

Authored synchronous-invocation declarations are one concrete use of this
rule. Typed lowering binds each target-name span directly to its exact
parameter-symbol/ordinal or boundary-trait symbol. Effect inference consumes
that exact target, and package projection joins it to checked machine facts;
neither stage reselects a target from diagnostic spelling.

Authored service reach follows the same seam. A typed private sidecar retains
each exact target occurrence and each clause keyword, while the normalized row
contains authored targets, invocation-contributed services, and parent closure.
Checked effects settle whether the callable publishes that row or internally
infers it. Package projection rederives and joins both facts; an authored empty
ceiling cannot collapse into omission, and inferred or closure-only members do
not acquire invented source coordinates.

Authored suspension and blocking clauses retain their exact keyword spans on
the typed machine or structural signature. Checked facts settle the published
or internal interface and its effective may-ceiling; package projection
requires that interface, the authored boolean, and source custody to agree.
Public and otherwise contract-supplied machines deliberately expose the
published may-ceiling as their checked operational summary, so this stage does
not manufacture a separate claim that the current body exercised—or did not
exercise—the permission.

Must not own:

- Machine instruction shape, ABI placement, final storage layout, relocation
  identity, or platform image policy.
- Concrete provider-plan selection, target bindings, or installation state.
  Omega orchestration carries the selected-plan sidecar separately after
  semantic checking; `CheckedTrees` retains only semantic receipt identities
  and checked evidence that refer to it.
- Rewriting checked obligations into backend convenience data without preserving
  the original semantic evidence.
- Treating semantic `Content<A>`, logical borrowed-place footprints, and
  physical effect footprints as one structural notion. Relate them only through
  an explicit checked carrier/operation bridge.

## Implementation Map

The stage should stay organized around semantic nouns instead of pass history.
Current ownership is:

- `semantic.rs` owns semantic fact-plan assembly and public semantic lookup
  exports. `semantic/contracts.rs` owns contract semantic fact assembly,
  `semantic/contracts/places.rs` owns contract fact place recovery, and
  `semantic/contracts/payload.rs` owns contract semantic payload construction.
  `semantic/points.rs` owns proof-obligation and contract program-point/origin
  mapping.
- `semantic_calls.rs` owns shared call-site lookup used by proof, borrow, flow,
  mutation, and ownership checks. `semantic_calls/traversal/context.rs` owns
  `CallSiteTraversal`, the explicit state for locating a statement/expression/
  transition call ordinal; expression and statement traversal modules should
  consume that context instead of threading raw coordinates through recursion.
- `flow/carried_semantic_dependencies.rs` owns the checked package-neutral
  sidecar for exact carried type, layout, ownership, and cleanup dependencies;
  `checked-trees/src/flow/semantic_dependencies.rs` owns its durable checked
  vocabulary.
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
  `borrow/loans/aggregate.rs` owns structural aggregate-initializer traversal,
  `borrow/loans/owner_paths.rs` owns owner/place projection conversion and
  matching,
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
  `checked-trees/src/borrow.rs` owns the grouped `BorrowFacts` root and
  constructor for writable-root, access, call, loan-owner-segment, loan, and
  state borrow arenas. Each published loan addresses its owner projection by a
  handle span into the shared owner-segment arena. The first checked-only
  resource-closure rung deterministically rejoins each direct-root loan to its
  exact state, owner path, captured place, access polarity, activation,
  weakening, parent state-invocation lifetime, and restoration obligation.
  Independent checked replay rejects missing, duplicate, or drifted rows, and
  direct-root compatibility certificates consume these joined place/access
  rows rather than manufacturing resource facts. This carrier remains
  non-authorizing and is not Terminal evidence. One further checked formation
  rung now classifies loans as `DirectRoot`, exact `Reborrow { parent_loan }`,
  or `UnretainedDerived`. Only an explicit reference-local reborrow with one
  unique prior state-owned loan retains its immediate parent; multihop chains
  name each immediate parent independently. Checked replay rederives the source
  occurrence, owner projection, formation order, and rebased captured place and
  rejects parent or lineage-tag substitution. The next checked-only closure
  rung retains each such direct reborrow in a separate topological resource
  arena: the child keeps its exact state/owner/place/access and one activation
  and weakening, while a typed handle identifies either its direct-root parent
  resource or the preceding reborrow resource. Independent replay validates
  both arenas before rebuilding either and then remaps every parent handle in
  loan order. The restoration row is explicitly a pending child-to-parent
  obligation only; it does not prove parent activity or reactivation, temporal
  containment, or completed restoration. Compatibility certificates for these
  children must rejoin the exact new resource row. Aggregate/helper transfers
  and ambiguous or reassigned aliases remain `UnretainedDerived` and have no
  row. Each retained child now also joins one checked parent-suspension
  formation boundary: its exact activation handle and the unique parent-loan
  constraint in that statement's entry set. Replay binds both occurrences to
  the existing child and typed parent resources and validates the complete
  boundary before either arena rebuilds. This establishes parent availability
  immediately before child formation only. It does not claim a suspension
  interval, post-formation parent activity, reactivation, completed
  restoration, or later lifecycle authority. Lexical loan weakening may
  precede the child's weakening, so those later lifecycle claims require a
  distinct flow carrier rather than comparing the current endpoints. Neither
  resource arena supplies Terminal authority.
  The exact parent and child weakening handles now also close one checked-only
  lexical end-status join. Its three outcomes say only that the parent retired
  before the child, retired at the same boundary, or remained lexically live
  past it. Replay orders `LastUseExpired` before statement entry,
  `LocalReassigned` after the right-hand side, and `StateExit` after the state;
  arena insertion order is not semantic. Exact handles, child/parent/resource
  identity, and the derived status all replay transactionally. This does not
  establish suspension containment, authority return/reactivation, cascade
  through retired parent carriers, completed restoration, or Terminal
  authority.
  A separate checked-only lifecycle arena now consumes the same exact handles
  in semantic phase batches using the closed nine-cell `Read` / `Mutable` /
  `WriteOnly` relation from the ownership guide. Its replay state distinguishes
  available carriers, one exact exclusive suspension, a mutable carrier frozen
  by a shared cohort, and either form retired while descendants retain the
  pending route. `Read` from `Read` releases without restoration; `Read` from
  `Mutable` joins a cohort and restores mutation only after its final member;
  permitted exclusive children suspend one exact branch and restore the
  parent's original access. Exclusive paths close deepest-first, shared
  cohorts release as a set, and retired carriers retain their ordered weakening
  path.

  The two terminal checked dispositions are exact rather than overloaded.
  A parent carrier ending at the child's same semantic boundary records
  `SameBoundaryLineageClosure`; only `StateExit` ending at the exact
  direct-root lifetime records `StateExitDirectRootHandoff`. Independent replay
  reconstructs the phase, path, and final target and rejects either label in
  the other circumstance before either resource arena is rebuilt. Both remain
  non-authorizing.

  Suspension/freeze containment is now retained in a sibling checked-only
  certificate arena after the complete semantic-phase lifecycle replay
  succeeds. Each permitted exclusive child has one suspension row, and each
  `Mutable`-to-`Read` child has one freeze row; `Read`-to-`Read` release has no
  containment claim. A row binds the exact child and typed parent resources,
  parent and child access plus classified effect, child activation and exact
  parent-entry constraint, both weakening handles, and the frozen parent and
  child places with their ordered projection remainder. Independent replay
  reconstructs that entire join and rejects missing, duplicate, reordered,
  access-amplified, or retargeted rows before rebuilding either resource arena.
  These certificates remain non-authorizing and do not establish completed
  restoration. One further checked-only row now retains an exact use after
  one-hop restoration: a direct mutable parent lends an exact mutable or
  write-only exclusive child, or one exact read child occurrence with no sibling
  for that parent. Other non-overlapping sequential exclusive siblings may
  occur. The exclusive form binds `Reactivate` and `ExclusiveSuspension`; the
  shared form binds `RestoreSharedCohort`, roster `[child]`, and `SharedFreeze`.
  The child ends by `LastUseExpired`, and the same boundary enters
  one runtime-receiver-free call with one exact mutable-reference parameter
  over the bare parent carrier whose mutation summary is the complete restored
  referent. Nominal static qualification is allowed and is not a runtime
  receiver. The row independently
  rejoins both resources, weakening, disposition, containment, flow and borrow
  calls, the carrier-read access, parent-loan entry constraint, captured
  places, restored access, and target. Multi-member or sequential shared,
  multihop, concurrent-sibling,
  state-exit, projected, receiver, extra-parameter, direct-assignment, and
  nonmutating shapes remain outside this transactional replay. The downstream
  Terminal consumer independently replays the exact call shape, ordinal-zero
  call coordinate, bound callee, restoration class, and encoded sole-member
  cohort roster. The shared class also requires exactly one compatible
  whole-parent mutating `CallUnit` in the caller. It then publishes one
  canonical operation-bounded use row; it grants no cleanup, transfer, or
  discharge. The consumer also publishes exact direct-root custody for a
  finite nonempty linear exclusive lineage whose direct root is
  `Mutable`, whose edges are `Mutable`-to-`Mutable`,
  `Mutable`-to-`WriteOnly`, or `WriteOnly`-to-`WriteOnly`, and whose leaf
  independently replays a state-exit handoff to that exact root lifetime.
  Multi-member or sequential shared cohorts, exclusive branching,
  multihop/projected/direct-assignment
  restored use, and non-state-exit root custody remain open. Root handoff and
  restored-use publication grant the borrow layer no cleanup, transfer, or
  linear-consumption authority.
- `checks/borrows.rs` is the borrow-check entry point. `checks/borrows/calls.rs`
  owns call-site borrow-check coordination,
  `checks/borrows/calls/conflicts.rs` owns call-site access/access and
  access/loan conflict legality, `checks/borrows/calls/writability.rs` owns
  mutable argument writable-root validation, `checks/borrows/statements.rs`
  owns local borrow and mutation conflicts, `checks/borrows/overlap.rs` owns
  borrow overlap entry dispatch and root matching,
  `checks/borrows/overlap/segments.rs` owns place-segment overlap policy,
  `checks/borrows/overlap/indexes.rs` owns index and range overlap policy, and
  handles normalized fixed-index/range pairs as well as range/range pairs. For
  admitted zero-premise structural compatibility rows it also captures and
  replays the ordered forming/active path positions of every consulted scalar,
  range-start, and normalized exclusive-end selector. Exact integer and
  immutable-symbol values are formation-frozen; explicit unknown rows remain
  conservative. Replay independently normalizes the exact typed formation and
  requires equality before consuming a prior certificate across an idempotent
  ledger rebuild; a new pair records the current formation normalization;
  `checks/borrows/details.rs` owns diagnostic lifetime explanations.
  Aggregate loan construction recursively preserves exact record, active-case,
  and fixed-index owner paths. Direct helper-call results and moved/projected
  borrow-carrying aggregates nested inside those literals are expanded beneath
  the enclosing path prefix, retaining their source selection and read/mutable
  polarity rather than becoming opaque at the nested expression boundary.
  Denotation-preserving same-carrier value casts recursively reuse that source
  expansion at root and nested positions; the borrow-recast form does not enter
  this path and remains governed by its validated representation footprint.
  Whole-name/member borrow recasts publish a loan on the exact source place;
  one exact literal index into a fixed byte array may instead publish the
  validated fact-free primitive, recursively literal fixed-array, or
  closed-record target footprint as a half-open `FixedRange`. Array eligibility
  requires a nonzero literal length at every array level and ends in either one
  exact fact-free non-Boolean fixed-width primitive or an eligible closed
  record. Primitive-terminal arrays require an exactly tiled shared
  representation. Record-terminal arrays repeat the complete normalized padded
  record extent. Eligible records may contain recursively literal array fields
  ending in the same exact primitive or record shapes. A zero extent
  participates only when its terminal independently qualifies and the whole
  record remains nonzero; its element alignment and padding remain covered.
  Fully specialized type plus scalar-integer `const` or exact-replayed acyclic
  structured-data `const` instances use the exact synthesized symbol and
  substituted fields after validating their concrete base/argument origin.
  Scalar const arguments are unbound canonical decimal leaves within their
  exact declared integer carriers. Structured atoms are completely decoded
  under fixed resource bounds and replayed in declaration order against the
  exact resolved monomorphic record or pure-sum carrier, including selected
  cases, ordered payloads, nested literal arrays/records/sums, and exact
  integer/Boolean leaves. Substituted instance fields remain the only layout
  authority. One direct lifetime-only shell around an otherwise eligible
  synthesized record also uses its exact instance symbol when lifetime arity is
  exact and runtime arguments are empty. One further lifetime-only synthesized
  record shell may occur beneath that root under the same checks. The bounded
  graph memoizes the complete symbol/depth/entry context, preventing a shallow
  sibling from authorizing the same symbol through a deeper diamond. Raw
  checked lifetime spellings stay distinct; the erased physical representation
  and loan size share the same sealed resolver. Array descent disables
  lifetime-shell admission for the complete descendant graph, including
  ordinary named wrappers.
  Generic normalization rewrites
  concrete-machine cast targets and supports recursively nonzero literal-array
  type arguments. Record eligibility and representation recursion use exact
  symbol identity and require a nonzero, quotient-free, acyclic, all-relevant,
  recursively fact-free shape. Runtime or merely bounded offsets, slices, total
  zero-size targets, open/unresolved or mixed/recursive/custom-canonical
  structured-const, lifetime-generic arrays, shapes deeper than two lifetime
  shells, or malformed/nonphantom lifetime applications,
  machine/proposition generic instances,
  invariant-bearing/erased/cased records, and other indexed recasts stay fenced
  because an element path cannot represent their complete target footprint.
  `checks/borrows/persistent.rs` admits borrow-carrying writes backed only by
  immutable artifact-lifetime storage (direct/nested literals, folded literal
  joins, and machine results whose every value exit resolves to such a source),
  and retains that provenance through exact persistent-place copies within the
  same state. Whole persistent fields and stable nested field, sum-case, and
  fixed-index borrow-frontier paths also cross named graph-state edges under a
  must-analysis: every predecessor must carry the exact path, and the field
  symbol rebases it independently of each state's receiver parameter. Stable
  leaf facts can accumulate over several states and survive disjoint sibling
  mutation. An immutable local or state-parameter runtime index also rebases
  across a named edge when forwarded directly, or through a chain of direct
  immutable local copies, into an immutable target parameter. Mutable/computed
  aliases, a missing predecessor fact, overlap, or an opaque statement call
  clear the affected shortcut. Complete R5 statement-
  and value-call frames preserve stable paths when their may-write sets are
  empty or disjoint and invalidate only overlapping paths. Internal wrappers
  compose nested boundary receiver/out-argument frames. State cycles keep exact
  frames through reordered primitive or shared-reference parameters, direct
  stable mutable-alias substitutions, and structurally transparent
  returned-place substitutions, while opaque replacement or a non-bijective
  write-capable backedge remains conservative.
  Flow invalidation shares the validation layer's complete-or-opaque frame
  verdict, then projects direct writes into structured symbol/range places and
  propagates them through resolved internal calls to a finite fixed point. A
  complete empty callee stays empty rather than widening to every mutable
  argument; failed caller-place reconstruction or an opaque body uses the
  ownership fallback. The structured consumer does not define a second SCC or
  alias-admission rule.
  A direct stable alias replacement updates that binding's origin without
  redirecting aliases established from its prior value. Stable mutable-alias
  chains retain exact member projections; an indexed reborrow through an exact
  alias retains its nearest collection, while an already-coarse alias stays
  absorbing, including across a direct member-after-index origin. A validated
  mutable recast local retains its source storage origin when that source
  expression is effect-free; effectful recast sources remain opaque. An
  alias into a primitive scalar, concrete primitive-only record/sum, or nested
  fixed-array local is caller-isolated and contributes no published write;
  structurally transparent helpers preserve that local origin. Recursive,
  generic, or other computed local roots remain opaque unless the shared
  aggregate-leaf transfer below establishes their reference origins.
  Aggregate literal leaves that are view-producing helper calls retain the
  helper signature's exact selected input loan through nested record,
  active-sum, fixed-array, and concrete-generic structure; a call expression
  cannot erase the leaf's borrow merely because it is not itself a canonical
  place.
  The compiler-owned `as_mut_slice()` view preserves its backing array origin,
  including through a structurally transparent helper result or as a direct
  statement-call argument. An effect-free discarded value expression derived
  from either `as_slice()` or `as_mut_slice()` is neutral to a helper's
  returned-place relation. One direct Unit statement call with a complete frame
  is likewise neutral when its arguments do not expose a mutable-reference
  binding for rebinding: writes through references passed by value change their
  contents without redirecting their origins.
  Direct-call expression trees use one finite worklist, shared by statement
  arguments, non-reference assignment values, and effectful indexes. Every
  sibling must pass independently: receivers retain structural member/index
  paths or a proven helper-result origin, and every receiver index passes the
  same scalar call-tree checks. Receiver-producing calls have their own
  non-rebinding complete-frame check; their reference result is not a scalar
  index value and requires separate returned-place evidence.
  No expression may reborrow a mutable-reference binding, and every internal or
  boundary call must have a complete inferred frame. Call nesting has no numeric
  cutoff. Computed arguments to resolved nongeneric internal or boundary calls
  carry the formal parameter's type into the shared value rules below. Every nested call
  supplies its own argument contexts, excluding the attached `self` parameter.
  Direct reference-valued arguments retain their origin rules; calls beneath a
  primitive computation require caller-isolated results. Effectful record and
  selected-case arguments must match the formal nominal type, and fixed arrays
  must match its element type and literal length. Missing, ambiguous, or
  generic formal context does not authorize newly admitted computed values.
  Existing pure places and direct-call trees
  keep their prior admission; this is not an independent full typing check.
  Attached calls on preceding declared locals consume the target symbol stamped
  by name resolution. Their computed arguments use the same formal-type and
  write-frame rules as other resolved internal calls; no frame-side member-name
  lookup is needed. Receivers that still lack a resolved target remain opaque.
  Direct call and value-expression frame queries replay the same state transfer
  up to their owning statement to recover local reference origins. They publish
  canonical storage paths and overlapping live alias spellings, so string-based
  fact consumers cannot retain either spelling across a write. Prefix writes do
  not become writes of the queried call. Stable replacement changes only that
  binding; prior aliases retain their established storage origins. An ambiguous
  query owner or unsupported alias prefix remains opaque.
  Structured call mutation uses the same transient `LocalWriteOrigin` evidence
  after instantiating receiver/argument paths. Exact origins retain structured
  suffixes; collection-coarse origins discard them. Caller-rooted writes through
  local references therefore cannot be dropped as private local writes.
  Borrow authorization retains the original access route separately from this
  storage footprint: a write through an authorized local reference is not a new
  direct access to its borrowed owner. Fact invalidation consumes storage
  origins; loan compatibility checks consume the route used by the call.
  A literal constructor is not a caller storage address. Owned call operands
  reuse the move-event traversal to identify transferred source values; access
  routes project literal fields and elements to their actual expressions before
  applying the existing loan-compatibility checks. Moving a local carrier inside
  a literal therefore retains its local route rather than creating an unknown
  expression-rooted access. Storage-origin expansion remains separate.
  Direct assignment demand stops the same state transfer at the store and
  reports either local-binding replacement or a storage write. Structured
  summaries and statement invalidation use that verdict before projecting local
  reference roots; replacing a binding never writes its former referent.
  Exact origins retain existing field/index/range selectors, collection-coarse
  origins stay coarse, and unresolved storage cannot become a complete empty
  summary. Private scratch writes are filtered only after origin projection.
  Statement and call invalidation also project each storage write through the
  shared live origins to invalidate copied facts on overlapping aliases. This
  closure retains structured selectors and does not alter borrow access routes
  or publish private alias spellings in caller-visible state summaries.
  `calls/write_frames/reference_subjects.rs` separately queries exact bare-local
  reference identity at a statement prefix. It reuses binding transfer with
  shared-reference discovery enabled only for this demand; shared access never
  gains write capability. Initializers and rebindings require declared nominal
  roots and owned Field/Case projections. Body-proven helper results reuse the
  shared returned-place relation for shared and exclusive references. Each
  helper hop needs a resolved callee and a selected parameter-rooted source;
  intermediate aliases cannot recover identity from a missing symbol's name.
  The exact query validates both the relative projection and actual caller
  source, and rejects reference-binding exposure in any call operand.
  Local literal carriers retain shared as well as exclusive reference leaves
  for this query. Loading a frozen Field/Case leaf validates the exact prior
  declaration, nominal selectors, retained case selection, and one canonical
  source. Nested carrier copies preserve that declaration-time binding across
  later alias rebindings. A loaded reference stays live: writes to its referent
  do not become snapshots. Slot or ancestor replacement and explicit mutable
  exposure retire the frozen relation. Implicit mutable receivers at the slot
  borrow the referent; receivers at an ancestor can replace the slot and remain
  opaque. Unknown shared leaves retain query-local markers without widening
  ordinary write frames. Aggregate helper results still need separate shared
  leaf result relations; an empty exclusive-write footprint proves no shared
  reference identity. Name-only, untracked loaded slots, indexed selections,
  coarse, and unresolved helper origins cannot supply exact identity.
  A returned reference identifies storage, not its old
  contents: helper writes still invalidate overlapping progress qualifications.
  Resolved methods named like slice views use their selected body relation;
  unresolved view shortcuts retain only collection-coarse evidence and cannot
  establish an exact reference subject.
  An unresolved read-only binding stays unknown independently of other local
  references. Its marker has no structural source, and copies preserve that
  absence; a later direct rebinding can recover only the replaced binding.
  These query-local markers are retained after initializer effects and binding
  exposure checks, never as permission to bypass them or as cached write
  summaries. Unknown write-capable aliases still make the prefix opaque.
  Ordinary frames treat read-only local rebinding as a private binding write.
  A pure terminal reference expression transports the referent without
  replacing its binding; effectful results retain the exposure fence.
  `flow/reference_places.rs` adapts that structural origin and checks earlier
  operand writes against both the local binding and its referent. Contract
  domain checking may then match an existing call-entry fact for the exact
  referent; it neither reconstructs a declaration-time fact nor mints a domain.
  Arithmetic validation checks the right-hand side against pre-store facts,
  invalidates all overlapping owner/alias spellings, then records the new
  target value. Scalar reference reads retain their pointee's numeric type even
  after a flow fact is invalidated; losing a fact cannot disable overflow checks.
  Structured storage queries distinguish a complete empty frame from an opaque
  result. Empty frames preserve facts; an unknown storage origin clears facts
  rather than publishing a disjoint local-rooted fallback.
  Boundary fallback requires the shared signature-and-origin frame to be
  complete and projects all its storage paths, including receiver storage
  without an explicit `self` parameter and receiver-qualified field arguments.
  Canonical assembly intrinsics affect their declared machine
  services, not caller storage; their result assignments carry separate writes.
  A recursive or opaque call, binding reborrow, or
  unsupported expression rejects the whole tree, including when it occurs in
  just one sibling. Frame inference retains its active-state recursion checks.
  A mutable indexed statement argument additionally needs a parameter-relative
  origin proof at every position in the tree, not just at a particular nesting
  depth. A scalar projection of a computed aggregate can instead use its typed
  value evidence; a borrowed indexed place cannot use that fallback.
  Stable-alias and parameter-relative scalar indexes use the same value
  expansion, with their respective binding-reborrow checks.
  Complete frames remain may-write evidence; admission never suppresses nested
  call writes.
  Boundary argument contexts and frame inference share the same unique,
  nongeneric boundary-trait signature on a canonical `self.field` receiver.
  A same-named field beneath another receiver prefix cannot select that cached
  signature. Exact actual/formal arity is required. The receiver is always in
  the may-write set, and exclusive parameters add their direct mutable-borrow
  places or forwarded exclusive-reference bindings. A forwarded binding must
  name an exact parameter or prior local declaration in the owning caller state;
  foreign, stale, missing, and shared-reference identities cannot supply it.
  The boundary resolver returns the binding path without replaying prefix
  transfer. Whole-state inference and public demand closure apply the shared
  local-origin evidence, including stable replacement and prior aliases.
  Constraint wrappers are peeled without erasing reference access, and both
  the formal and forwarded reference's referent must have supported owned
  storage. A checked free or attached helper can supply a computed reference
  through the shared returned-place relation: its selected formal must refer
  only to owned storage, and its actual must retain a proven caller origin.
  Nested helpers compose exact field suffixes or absorbing collection-coarse
  paths; wrapping a foreign or stale binding in a helper does not admit it.
  Attached methods resolve nominal `Self` through their own attached declaration,
  and receiver projections cannot traverse a loaded reference-bearing member.
  Indexed method receivers carry the same origin precision as arguments through
  shared frame instantiation. A receiver-relative field write becomes the
  nearest whole collection; independent explicit-argument writes retain their
  own exact paths. A collection path requires an exact resolved method target,
  not a cached field or free-call name fallback. Boundary signature selection
  still requires an exact canonical receiver, and a resolved method whose name
  matches a value builtin retains its body's writes.
  Numeric builtins require an exact receiverless builtin target; an unresolved
  method named `min`, `max`, or `sqrt` cannot acquire an empty frame by spelling.
  Receiver-index effects use the shared complete, non-rebinding call-tree proof;
  producer writes remain in the aggregate frame. Unknown or recursive index
  calls and reference-binding reborrows cannot preserve an indexed origin.
  Declaration lookup for that check is shared with boundary forwarding and
  does not replay prefix origin transfer inside raw frame resolution.
  Methods on call-produced references reuse boundary forwarding's owned-storage
  and returned-place proof. Exact receiver origins retain field suffixes;
  indexing a returned collection makes subsequent receiver suffixes coarse.
  Independent argument and receiver-producer writes remain in the frame.
  Declared result types select methods and type projections, but cannot prove a
  storage origin. Unknown targets, recursive results, and loaded reference
  carriers stay opaque; a computed storage path cannot select a method by name.
  Argument type and access checking recognizes a resolved
  call's declared reference result without creating a binding-slot borrow.
  Exact normalized reference identity is required except for the existing
  mutable-to-shared attenuation with the same referee; write-only attenuation
  stays explicit. A reference result cannot use a scalar-call fallback to match
  an owned argument type or an unrelated shared referee.
  Parameter binding mutability is separate from declared reference access:
  `mut` on an owned by-value parameter does not require a caller loan, and a
  mutable shared-reference binding cannot forward exclusive access. Constraint
  shells retain the same access rule for formals and forwarded values.
  The boundary frame describes the call's writes; operand evaluation separately
  contributes every producer write, and whole-state frames include both.
  The same active-state guard spans helper-result proof and boundary inference,
  including cycles through a helper's boundary-call arguments.
  Opaque, recursive, and boundary-produced reference results and reference-bearing
  members remain opaque; none is treated as an implicit reborrow.
  Primitive and concrete caller-isolated by-value parameters add no caller
  writes. Internal call and transition frame instantiation is set-valued:
  a reference-bearing record, selected-case, or fixed-array literal transports
  its demanded exclusive-reference leaves through the same owned-storage and
  helper-result proof as boundary forwarding. Exact field demands select that
  declared field; a coarse collection demand visits every literal element and
  unions its reachable referents. Owned by-value fields remain private and
  repeated origins are deduplicated. Nominal/field/case substitutions, array
  length mismatches, and missing or opaque demanded reference leaves cannot
  become complete empty frames. Operand evaluation still publishes every
  producer write separately from the callee's frame.
  Untracked reference-field or whole-carrier replacement makes the body opaque
  in both ordinary and cyclic state analysis; later writes cannot reuse its
  original literal substitution. Existing stable local-alias replacement keeps
  its shared origin transfer. Replacement classification follows declared
  result types and literal projections, including every candidate array element;
  an effect-free projected reference is still a replacement, not an owned-value
  store. Local record, selected-case, and fixed-array literals use that same
  leaf walker at declaration. Each stored reference retains its local field,
  case, and fixed-index selectors and a canonical source origin frozen against
  the preceding aliases and stored leaves. Later alias rebinding cannot redirect
  it. Owned suffixes borrowed through earlier stored leaves compose those
  origins; coarse array sources retain every reachable referent. Borrowing a
  stored reference slot or whole carrier, including inside helper operands,
  stays opaque because it could replace the established origin.
  Ordinary state transfer, named-cycle equations, and public demand queries
  consume the same evidence. Structured storage projection selects exact local
  leaves, unions overlapping runtime-index demand, and retains private ancestor
  storage for local fact invalidation. Reverse alias closure includes each local
  leaf selector; borrow access routes remain unchanged. Missing or unknown leaf
  evidence cannot become private storage or a complete empty frame.
  An expression-rooted actual is not a storage identity: direct call projection
  and transitive summary propagation reconstruct its complete shared frame
  instead of dropping its reference leaves as private expression roots.
  Moves from preceding local aggregates reuse their established leaves, including
  moves nested in a new local record, selected-case, or array literal. Exact
  field/case and fixed-index selection removes only the selected source prefix;
  runtime-index selection unions matching fixed-element leaves. The source must
  name the exact earlier local declaration, and each selector is checked against
  its declared owning type before the projected type is compared with the
  destination. Unknown/stale/foreign roots, mismatched nominal types or array
  lengths, invalid literal indexes, and traversal through a loaded reference
  remain opaque. Local origin evidence retains selected cases alongside reference
  leaves, including empty cases. Payload selection must agree with every
  possible retained case; an absent payload cannot become a private value with
  an empty frame. Whole empty cases and genuinely shared-only subrecords still
  transport without writes.
  Shared place normalization recovers local declaration types and retains array
  element types for subsequent field selection; normalization alone supplies no
  origin evidence or move permission.
  Owned by-value parameter moves derive their leaves from the exact parameter's
  concrete declared type and use the same projection and local transfer.
  Parameter field origins remain exact; an array origin stops at its collection.
  Type-derived array leaves retain one unknown-element selector per element
  shape, not one row per declared element. These transient may-write selectors
  preserve trailing field/case distinctions but cannot authorize an index access.
  Every possible declared sum case is retained, including empty cases; a type
  alone does not establish which payload is present. Reference parameters,
  generic or recursive carrier shapes, and loaded reference carriers remain
  opaque. Borrowing incoming carrier slots also stays opaque because replacing
  a reference would invalidate its parameter-relative origin.
  Immediate call literals also transport moved local and parameter carriers.
  Raw instantiation derives symbolic reference-leaf paths from the exact source
  declaration and projected type, then filters the callee's demand against the
  declared fields. A demand beneath an exact reference composes its remaining
  suffix; collection origins absorb it. A disjoint owned field has no caller
  writes, while an unknown field cannot become a complete empty frame.
  Ordinary state transfer, cycle equations, and public caller closure resolve
  symbolic local paths against frozen prefix origins before treating them as
  private storage. Raw call inference never recursively reconstructs its own
  caller prefix. Public demand also validates a prefix with incoming carriers
  even when it has no alias-bearing local declarations, so earlier slot
  replacement cannot preserve a stale parameter-relative frame.
  An aggregate-valued internal helper call derives its returned leaves from
  the same state transfer and aggregate walker. Its exact checked-body target,
  argument count, and normalized result type must agree. One final expression
  or unconditional value transition supplies the result; earlier transitions
  and alternate or named result routes still need graph-level relations.
  The transfer validates the entire body, including terminal operand effects
  and reference-slot replacement fences, before freezing result leaves against
  local aliases and stored carriers. Each exclusive leaf must cross an
  exclusive-reference boundary with owned referent storage, either a reference
  formal or a structurally selected reference inside an owned formal.
  Callee-private leaves cannot disappear as an empty caller footprint.
  Actual arguments and attached receivers instantiate those formal origins
  with exact field suffixes or absorbing collection precision. Empty selected
  cases retain their evidence.
  The body-recursion guard ends before caller-argument substitution, so finite
  repeated calls are not confused with recursive result bodies. A consumer
  frame likewise guards only its body, not caller actuals; a result
  producer may invoke that consumer without forming a body-recursion cycle.
  The helper's writes remain producer effects, separate from a later
  consumer's footprint.
  Lifetime-only nominal applications use the same declared field structure;
  lifetime checking retains their region arguments, and actual type-generic
  carriers still require substitution before this analysis can inspect them.
  This result evidence is storage information, not loan or lifetime authority.
  Reference origins retain an exact source symbol and structural selectors
  separately from their may-write footprint. Indexing may coarsen that footprint
  without erasing the later field or case selecting the source reference.
  Result substitution must cross that declared reference boundary; ancestor
  overlap with a referenced sibling is not evidence of a reference value.
  Fixed-index source selection retains its ordinal, while a runtime index unions
  matching leaves. Callee-local runtime index identities are erased when
  substituting caller sources. Stable aliases into frozen local carriers retain
  that structural source through copies and rebinding; they do not choose one
  candidate from a runtime-index union. Ordinary transfer and cycle equations
  use the same alias admission.
  Immediate call literals resolve payload moves using the same frozen local
  cases as intermediate declarations. Inference carries this evidence in a
  body-scoped context, separate from active recursion guards. Public call
  queries recover the caller prefix once before raw inference and reuse those
  aliases and stored leaves for final storage closure. Raw inference does not
  recursively reconstruct its own caller prefix. Nested helper bodies restore
  the outer case context even when their analysis fails; cycle equations retain
  their own local evidence during edge instantiation.
  Case evidence is retained for owned-only and shared-only payloads too: zero
  exclusive leaves cannot bypass a required case selection. Missing payloads
  and runtime selections spanning incompatible cases remain opaque. Frozen
  case-bearing storage cannot be replaced or exposed for replacement; ordinary
  writes beneath a selected payload do not replace its containing case.
  The shared statement exposure check runs before frame fallbacks, including
  implicit exclusive receivers and receivers nested inside value expressions.
  A declared input sum still denotes every possible case. Owned input moves
  retain a structural result-subtree relation alongside those possible cases.
  After checking the helper body, result substitution replaces that subtree's
  cases and reference leaves together from the actual argument. An empty input
  case therefore removes its absent payload leaves instead of failing to find
  a reference that cannot exist. The relation composes through frozen locals,
  nested literals, helper calls, and fixed or runtime array projections; fixed
  output positions retain their cases and runtime selections retain all
  candidates. Constructor-owned siblings keep their own case evidence.
  Symbolic local reference rows are filtered against retained case evidence
  before transport: an excluded branch contributes no leaves, but missing case
  evidence stays opaque rather than proving an empty subtree.
  Case-bearing input replacement or exclusive exposure invalidates the frozen
  relation, including for owned-only or shared-only payloads. Payload scalar
  writes do not replace an enclosing case. Caller case evidence cannot justify
  an unproved payload projection inside the helper body, and result relations
  do not specialize or contaminate cached state-write summaries. Conditional
  helper-body refinement and graph-level result routes remain unfinished.
  Passing an aggregate by value does not erase its references or authorize a
  loan; declared lifetime and borrow checks remain separate.
  Exclusive references to reference-bearing carriers remain opaque; primitive
  slices retain their collection reach.
  A rejected trait-receiver call stays opaque through every fallback consumer,
  including direct-call queries and state-summary equations. It cannot regain
  a receiver-only complete frame from signature-free syntax.
  A recognized trait receiver also prevents builtin-name exemptions from
  erasing a boundary member's receiver writes in value-expression queries.
  The boundary signature supplies argument typing context,
  not a checked implementation body or a returned-place relation.
  An explicitly discarded concrete primitive result from a nongeneric internal
  checked-body call is neutral under the same non-rebinding complete-frame
  rule. Discarded reference-bearing or aggregate results, boundary or generic
  statement calls, and other unsupported discarded expressions remain opaque.
  A free or attached helper whose terminal place is rooted in one
  mutable-reference parameter composes exact member suffixes or absorbing
  collection-coarse indexing onto that argument's origin through its call
  result and later transparent chains. The terminal place may follow a prefix
  of effect-free caller-isolated scratch locals and direct local `&mut` aliases,
  including mutable bindings and results of other structurally transparent
  helpers. A caller-isolated scratch initializer uses the same typed
  value-expression worklist as assignments below, with its declared local type
  as context. Primitive computations and concrete record, selected-case, and
  fixed-array literals may compose around complete non-rebinding calls. Every
  write must additionally resolve into a previously established caller-isolated
  scratch local; the initializer cannot write the local being introduced or any
  caller-visible origin. Stable aliases and projected alias chains into those
  scratch locals keep their private origins, and the write fence rebases alias
  paths before testing isolation. A private origin has no caller-parameter
  symbol and cannot be exported as a helper's returned-place relation.
  Recursive, opaque, externally writing, and unsupported initializer forms
  remain fences. A validated mutable recast local
  with an effect-free source may write through that source without obscuring a
  separately returned parameter origin.
  The exact returned-place relation also composes when such a result is supplied
  directly as a statement-call argument.
  Value-shaped assignments may write through those origins without changing
  the relation when the right-hand side is effect-free or a typed
  non-reference direct-call tree with complete frames. Reference-valued roots
  keep their existing relational handling.
  `calls/write_frames/value_expressions.rs` supplies shared value expansion to
  the assignment/initializer and call-argument worklists. Expansion enqueues one
  level at a time; alternating calls and computations do not recursively invoke
  the value walker. These worklists admit
  finite compositions of primitive computations and concrete caller-isolated
  aggregate literals. Root, field, element,
  member-receiver, and index-collection positions carry their available type
  evidence; no scalar-shell or aggregate-depth counter supplies that evidence.
  Unary and binary operations, primitive casts, member projections, and indexes
  require a primitive destination at computed-value entry. Calls below a direct
  scalar computation additionally require caller-isolated results; whole-value
  and aggregate-leaf calls retain their known non-reference result requirement.
  Every call still needs a complete non-rebinding frame.
  Named record and selected-case literals must have a unique concrete,
  nongeneric, caller-isolated declaration, including all declared variants.
  Their fields supply the expected types for nested aggregates and primitive
  computations. Typed fixed-array literals require an exact literal length and
  matching element types. A literal below a member projection or array index
  is checked in that operand position, not admitted as an arbitrary operator
  operand. A projected array literal has no contextual nominal element type:
  its effectful elements may be arrays, direct calls, or primitive computations,
  but not record literals with an inferred contextual type. Every eagerly
  evaluated field and element publishes its call writes, including unselected
  fields or elements of a projected literal.
  Unsupported type or expression shapes, reference-bearing or generic aggregate
  declarations, recursive calls, and binding reborrows remain fences. Pure
  children are neutral after container validation; a pure root still follows
  the separate alias-replacement rules where applicable.
  Indexed assignment targets, statement arguments, terminal returned places,
  stable mutable-alias initializers, and direct alias replacements share the
  origin algebra: exact member suffixes compose before the first index; that
  index fixes the nearest collection-coarse origin; later indexes or members
  remain absorbed. Each effectful scalar index independently satisfies the
  finite non-rebinding expression rule and publishes all of its call writes.
  The index-operand context admits primitive computations and scalar aggregate
  projections without inventing a fixed integer carrier. Calls beneath a
  computation require caller-isolated results; direct index calls keep their
  prior complete-frame admission. Nested calls supply their own formal argument
  contexts. Bare effectful aggregates and ranges are not scalar index values.
  Numeric eligibility and bounds remain separate typing and proof obligations;
  a complete write frame does not prove an index safe to execute.
  A compiler-owned `as_mut_slice()` view on the collection spine preserves its
  backing array origin. That origin may come from a parameter, a stable local
  alias, or a transparent free or attached helper result; an attached helper
  uses its actual `self` receiver. A transparent helper's selected actual keeps
  the caller's effect-aware indexed-origin checks through nested helper calls;
  wrapping an indexed borrow cannot discard its index writes or bypass binding
  reborrow rejection. Member projections before the view retain
  their exact suffixes until indexing coarsens them. Recursive or opaque
  collection/view producers remain fences, as do binding-reborrow or
  unsupported computed indexes. Boundary statement calls with effectful
  indexed arguments remain unsupported.
  Indexed targets and non-reference assignment values compose independently
  under these rules and publish both sets of writes. A direct alias replacement
  changes only that local's origin; aliases established from its prior value
  retain their origins. A transparent helper may supply the replacement.
  Other computed rebinding and unsupported initializers remain opaque.
  For an
  attached helper, its actual receiver supplies the caller origin when the
  result is rooted in `self`. Other
  nontrivial results remain opaque; signature lifetime elision alone is not
  relational frame evidence. A signature-free call may fall back to its
  receiver plus every potentially exclusive place argument only when those
  arguments are directly representable. Aggregate literals and unproven nested
  call results remain opaque: producer writes do not prove returned-reference
  reach, and a whole-receiver path does not cover unrelated parameter roots.
  Frames whose places cannot be represented remain all-facts fences. Other
  writes into attached or machine-owned persistent storage remain fail-closed
  until
  parameter-backed loan propagation, runtime-indexed transport, broader exact
  R5 summaries, and general state-parameter loan-root rebasing are implemented.
- `checks/carry.rs` joins canonical place liveness with direct/transitive
  possible suspension. Lexical roots are statement-bound; attached-data and
  compatibility machine-owned field paths additionally follow reachable state
  transitions. `checks/carry/intra_statement.rs` preserves the shared
  preorder call identity while walking the language's closed evaluation
  schedule: attached receiver before arguments, eager siblings left to right,
  and aggregate fields in authored literal order. Strict binary operators share
  that order; `&&` and `||` retain their explicit short-circuit control, and a
  transition evaluates its subject once before only the selected arm.
  The settled call surface rejects a suspending call nested inside an argument,
  operator, aggregate, or condition, so partially evaluated operands never
  become hidden continuation state. Blocking-only calls may remain nested and
  preserve ordinary left-to-right evaluation.
  Call-argument policy derivation uses the target declaration's generic bounds;
  unrelated caller type parameters cannot qualify the target by name.
  `facts/carry.rs` derives the canonical contained-machine field topology into
  grouped machine/field/target arenas. Canonical semantic suspension crossings
  join descendant crossing facts. Terminal production consumes each exact
  crossing into one call-keyed suspension row without re-running liveness.
  A separate Omega-owned task-activation sidecar then inherits the retained
  carry demands and combines them with target layouts, calling-plan identity,
  the selected runtime provider, and the WCSU-backed `StackPlan`. None of that
  concrete activation realization is stored in `CheckedTrees` or target-neutral
  Terminal Psi.
- `checked-trees/src/flow.rs` owns the published checked-flow fact model
  export surface. The model is split by semantic noun under
  `checked-trees/src/flow/`: `contexts.rs` owns semantic/borrow
  constraint refs, `invalidations.rs` owns mutation/domain invalidation facts,
  `borrow_lifetimes.rs` owns activation/weakening facts, `ownership.rs` owns
  move/drop facts, `boundaries.rs` owns boundary-edge facts, `control.rs` owns
  state/statement/call/exit facts, and `roots.rs` owns grouped `FlowFacts`
  roots plus query helpers. Flow construction should join each noun root
  through its root constructor rather than hand-building the grouped fields.
- `checked-trees/src/facts/` owns checked semantic facts that are not
  part of the temporal flow spine: `invariants.rs` owns invariant definition
  facts, and `domains.rs` owns domain dependency facts and dependency-path
  accessors. Both expose root constructors so invariant and domain production
  joins arena roots explicitly. Suspension and blocking publication project
  separate machine-keyed rows from the transient operational inference plan
  after flow/service construction; each checked root consumes only its own
  boolean axis and preserves its independent published interface.
- `checked-trees/src/facts/contract_plans.rs` owns each machine's normalized
  public contract. Its settled crash axis is a canonical set of cause buckets
  with source-handle-free predicate identities; route-less and explicit-`true`
  clauses normalize to the same unconditional route. Public
  report fingerprints, reports, and terminal production consume this checked
  carrier rather than re-reading typed crash clauses. Authority-bearing joins
  additionally replay its domain-separated machine-contract commitment, and
  an empty commitment rejects even beside a nonzero compact report value. The
  same plan keeps an
  independent, non-fingerprinted checked-site layer keyed by state and
  state-local statement ordinal. Each site records the body-derived cause,
  selected published coverage, exact incoming guard conjunction, open invariant
  data identities, and a canonical definitely-live claim-frontier lower bound.
  That frontier supports audit and diagnostics; it cannot prove unlisted state
  valid or license survivor execution.
  Unconditional entry claims are included directly. A positive symbol-stamped
  case-pattern guard on a named edge is rebound through the target state's
  arguments and promotes only the selected conditional entry claim. A
  single-predecessor walk composes the complete parameter map through
  intermediate named states. A multi-predecessor meet retains the map only
  when every incoming edge carries the same guard polarity and exact composed
  final-parameter binding; ambiguous convergent edges and disagreements discard
  it. The claim identity preserves that proof across whole-value transfers
  without transferring it to a replacement value. Unknown cases, joins without
  a common argument map, and nested cases lacking proof at every case level
  remain conservatively absent. The argument map and membership facts are
  canonical symbol-rooted places rather than rendered labels; dynamic indexes
  and other source-dependent roots fail closed. A nested conditional claim is
  promoted only when every case segment on its claim path has matching
  membership evidence. A sibling
  checked-call layer uses the flow graph's state/statement/call coordinate,
  retains the callee target and contract report fingerprint, and stores the
  surviving selected summary after invocation argument substitution. Published buckets
  are selected for authored interfaces. Same-unit private bodies instead use a
  conservative monotone summary fixed point over the viable invocation graph:
  every explicit site becomes an unconditional cause bucket, a site-free leaf
  produces positive empty evidence, and a resolved
  nested summary carries a temporary canonical predicate tree and substitutes
  positional arguments through every nonrecursive edge. Recursive SCC edges
  widen to unconditional cause buckets, so argument-changing cycles close
  over a finite conservative bucket set while acyclic wrappers retain guards.
  An unknown dependency prunes its caller closure from the fixed point, so
  partial direct-site evidence cannot erase a nested crash. A published caller
  must cover every
  surviving call route independently with a same-cause bucket. Guard coverage accepts an
  unconditional caller route, the exact surviving predicate, or a retained
  structural consequence of the invocation's incoming path. Exact conjuncts
  and consequences remain separate checked fields. Private inferred callers
  remain body-summary inputs rather than authored-ceiling obligations.
  Callable trait and top-level boundary requirements, plus unresolved compile-time machine parameters,
  instead select a checked crash-contract capsule. The capsule retains the
  normalized public crash buckets and pins them to the complete normalized
  callable-contract commitment, so call refinement never depends on a local
  body or reopens the authored signature after checked lowering. Separately
  compiled imports still require the corresponding artifact-capsule input;
  that input is design blocked until the semantic import/export carrier,
  symbol identity, and certificate binding are specified.
  Nominal static-machine selections additionally publish an independent
  checked per-use identity table before monomorphization consumes their syntax.
  Its key is the statement/expression call site plus static-machine ordinal;
  its payload pins the registration operation, selected machine and entry,
  unique satisfaction trait/requirement, and canonical requirement overload.
  The monomorphization fixed point captures newly concrete forwarded uses after
  each cloning round. Exact duplicate observations collapse, a conflicting row
  at one key rejects, and structural selections do not enter this nominal
  table. When the exact nominal requirement owns an evaluated boundary calling
  plan, the row retains its domain-separated strong commitment as the
  target-placement join key plus a compact report fingerprint. Ordinary nominal
  selections retain no callback placement; the
  target-owned plan and emitted thunk remain outside checked Psi. Registrar
  plan identity separately binds the registration operation's exact nominal
  static-machine slot to one native destination; it never substitutes the
  selected machine into that plan fingerprint. Target lowering joins the fixed
  slot/destination row to this per-use selection and rejects incomplete private
  materialization closure.
  The row also joins the requirement capsule's normalized contract commitment
  to the selected machine's normalized declared contract commitment and retains
  an explicit admission-refinement receipt over those endpoints. Historical
  compact contract, refinement, and calling-plan fingerprints remain report
  compatibility coordinates only. Checked boundary lowering must rejoin the
  strong contract commitment to the canonical checked machine plan or crash
  capsule; the boundary row cannot self-authenticate its own stored digest.
  Target planning recomputes the calling-plan
  commitment from the exact validated plan and rejects a compact-equal
  substitution. The
  published capsule separately carries canonical service reach, synchronous
  invocation, suspension, blocking, termination, and crash axes. One
  exact-machine realized envelope aggregates effective checked reach and
  invocation, transitive suspension and blocking, checked termination and
  crash evidence, mutation frames, and capability flows while preserving their
  independent provenance. Crash evidence is refreshed after path-conditioned
  validation mutates that axis. The realized envelope now also retains one
  declaration-ordered resource-derivation row for every exact machine entry.
  Each row binds its machine, entry, and realized contract identity separately
  to the required Terminal-plus-target stack closure, Terminal control/fuel-
  schedule proof, and selected-instruction machine-state footprint. Checked
  replay proves complete owned-entry coverage and rejects axis fusion or
  identity drift. A checked boundary-callback use additionally retains one
  exact receipt over the selected row's machine, entry, actual-contract, three
  axis, and envelope identities beside its independent target calling-plan
  key. Construction and nominal-use replay reject a foreign entry or endpoint;
  target planning must still rejoin the receipt to the current checked roster
  before backend carriage. The rows and receipts deliberately contain no
  numeric ceiling, realized demand, target footprint, provider receipt, or
  installation authority; their fingerprints are compilation-local join
  summaries only. Neither envelope changes the published fingerprint or
  promotes inferred witnesses into caller facts. Callback resource admission
  remains closed until all three axes join their independently derived
  downstream evidence.
  Published routes are removed
  only when the call evaluator proves them false; proved-true routes become
  unconditional, unknown routes are re-encoded in the caller's positional
  namespace, and an empty surviving set is retained as positive crash-free
  evidence. Exact incoming path predicates remain separate conjunction factors
  on the call row. `facts/crash_calls.rs` owns this source-dependent production
  and substitution step. Canonical
  published buckets have dense plan-local identities. Each site cites every
  unconditional same-cause bucket as structurally proved guard coverage;
  exact incoming/fallthrough predicates and their sound conjunction/negated-
  disjunction, nested-negation, and Boolean-literal relation consequences add
  path-conditioned guarded coverage. Comparison operand reversal and negated
  equality/inequality are also retained as equivalent consequences. Negated
  ordered comparisons use the complement only when both operands have checked
  integer types; integer strict order also entails its non-strict bound and
  inequality, and integer equality entails both non-strict bounds. Unknown,
  user-defined, and float operands remain opaque. The site
  separately retains its exact incoming-predicate conjunction for downstream
  refinement and reporting.
- Predicate checking admits only total terms. Exact arithmetic owns and
  discharges its representability obligation at formation; Wrapping and
  Saturating own total overflow denotations after independent primitive
  obligations are discharged; direct Trapping arithmetic rejects.
  Explicit fixed-integer/address `embed` expressions produce proof `Int` plus
  exact source-carrier range facts, while a same-carrier `as` removes the
  policy and forms an ordinary Exact operation. Float denotation uses the
  format-specific `FloatMeaning` projection. These specification terms emit no
  crash site. Executable Trapping operations independently select a
  compiler-owned primitive guard, and crash coverage proves each
  path-conditioned derived guard implies the authored same-cause route
  disjunction.
  D40 requires each FloatMeaning projection to retain a canonical semantic
  source term separately from its authored occurrence: exact contract
  parameter/result, Terminal value, structural float leaf, or exact-bit
  literal, plus format and recognized declaration/catalog contract. Equal
  tuples share one proof-value identity; dense encounter order and source spans
  are not semantic correspondence. The checked-to-Terminal join and verifier
  independently replay this key before `FloatMeaningEqual` reaches the kernel.
  Exact-bit literals now realize this requirement end to end: checked facts
  retain raw `u32`/`u64` bits, equal tuples deduplicate independently of
  occurrences, and Terminal/verifier replay uses no producer coordinate.
  Projection admission additionally requires the exact toolchain-owned
  `Float::meaning32`/`meaning64` symbol from `float_operations.omg`, and checked
  binding replays that origin/path/identity join plus the exact toolchain
  `FloatMeaning` result from `float_meaning.omg` before emitting any row.
  Structurally identical local operator or result declarations reject rather
  than borrowing the closed catalog semantics. The recognized declarations
  must also remain private and contract-free. Checked projection rows retain
  the rooted tuple `(32, 1, 1, 1)` or `(64, 2, 2, 1)` plus its canonical
  commitment; deduplication and equality replay include that exact descriptor,
  and cross-format equality rejects before emission.
  A checked-only prerequisite now distinguishes one nonliteral cohort: a direct
  resolved `f32`/`f64` parameter in a top-level contract owned by the parameter's
  machine retains exact owner-machine and parameter symbols plus its primitive
  format and fallback transitional input. Binding rejoins the symbol to the
  owner's entry-state parameter table and keys deduplication by owner,
  parameter, and format. A bare reserved `result` in the owning machine's
  top-level `ensures` has a separate direct-result carrier keyed by that exact
  owner and primitive format; a real entry parameter named `result` shadows the
  pseudo binder. The exit checker retains the validated equality's source
  expression only long enough to rejoin the current contract occurrence. It
  admits structural reflexivity only when both checked operands canonicalize to
  the same proof value and that value is the exiting owner's direct result.
  This does not make raw IEEE equality reflexive and does not prove a distinct
  result/parameter pair. Checked-to-Terminal lowering erases the source handle
  and, when the owner is emitted with an exact scalar result, binds the carrier
  to source-free `(MachineId, result ValueId, format)` identity. Nested state,
  call, and operation results, locals, members, casts, const parameters,
  non-floats, structural leaves, and foreign owners remain transitional.
- `checked-trees/src/proof/` owns proof-facing checked facts:
  `obligations.rs` owns explicit proof obligations, `contracts.rs` owns
  contract proof facts/call/exit indexes, and `roots.rs` owns the grouped
  `ProofFacts` arena root and constructor. A named machine `requires` or
  `ensures` must normalize to one witness-bearing proposition. The checker
  mints a distinct erased evidence-term arena identity for each binding and
  retains its label, requires/ensures lane position, exact normalized
  proposition application, and carrierless evidence interface. The term
  identity is deliberately separate from proposition identity and producer
  provenance. Bare-name forwarding retains the exact incoming term. A concrete
  subjectless producer assignment instead retains the exact conformance,
  evidence-trait symbol, complete normalized lifetime/type/const/static-machine
  telescope, and complete normalized realization rows. Expected subject/trait
  shape validates rather than fills non-lifetime arguments; wrong arguments,
  ambiguous lifetimes, and unresolved open evidence endpoints reject. The
  terminal producer consumes forwarded checked
  terms into dense source-handle-free vocabulary identities with an exact
  proposition application and structured interface; the verifier requires the
  application and term rows to agree, and forwarding contributes one row.
  Canonical positional rows connect the selected terminal machine's named
  requires/ensures lanes to exact term IDs, with one shared ID across a
  forwarded pair. A selected producer emits a separate canonical proof-bundle
  provenance identity keyed to its ensured term, retaining the exact
  conformance, evidence trait, and normalized realization rows without source
  handles. The verifier admits an ensures-only term exactly through that row;
  provenance affects proof identity, never terminal semantic identity or
  execution. A pure Unit proof producer erases; if that producer contains
  runtime body work, typed lowering retains one ordinary Unit call and checked
  proof facts bind the output row to its exact call coordinate. Argumented
  proof-output calls apply the ordinary call-contract substitution to every
  input and output proposition. Explicit erased inputs retain exact target
  position and caller source term; an ensured term forwarded from one of those
  inputs preserves that witness identity, while a producer-backed result stays
  distinct. A generic proof-output target is accepted only after ordinary
  specialization has closed its static telescope; its checked specialization
  fingerprint retains the complete conformance application independently of
  the callable's post-specialization name. Each ensured
  realization pipeline retains the public proof-output
  selector beside its exact term ID; required lanes have no output selector. A
  concrete non-generic trait satisfier now checks its inherited named lanes
  against the requirement before lowering. Incoming aliases remain local, but
  lane cardinality/order, normalized proposition, and evidence interface stay
  exact; outgoing selectors are pinned, with authored strengthening appended
  after the inherited prefix. The inherited machine-state facts point to the
  satisfier's exact checked terms and lane positions. The first static
  requirement-call carrier is deliberately narrower than general dispatch:
  specialization of an attached caller's explicit proof-static conformance
  binder retains one
  call-local closed application and its exact public-requirement/private-
  realization row. The admitted requirement and realization are concrete,
  non-generic, one-state Unit callables, and the requirement owns one
  subjectless named input plus one subjectless unconditional named output.
  Checked contract-call facts and proof-output terms are sourced from the
  requirement signature, not the satisfier. The ordinary call still targets
  the concrete realization, while a captured output mints a distinct
  caller-local requirement witness even when the satisfier forwards a private
  input. Concrete strengthening stays visible only to direct concrete calls.
  Generic substitution on the public trait/requirement/satisfier surface,
  inherited rows, defaults, scalar results, subject-bearing or wider public
  lanes, direct named-conformance calls, and dynamic named-witness dispatch
  remain conservative fences. The separate contract-free dynamic scalar lane
  admits one attached Unit caller, one exact closed conformance row, one
  borrowed field source, and an exact `bool` or `i32` result. It retains either
  one never-rebound selection or an initializer plus one reassignment and
  latest call as distinct direct/rebound checked catalogs. A scalar
  reassignment may name a different closed conformance when both selections
  retain the same carrier, trait interface, borrow access, and normalized
  requirement roster; each version keeps its own conformance and row identity.
  It grants no implementation-private witness or authored contract lane. Package review
  already owns the declaration-level lane
  compatibility surface; this private call-site carrier adds no package
  schema. For a mutable receiver, the first mutation-bearing callable shape
  retains exactly one literal assignment to a direct primitive `self` field
  before the existing exact scalar return. The retained write joins the sole
  authoritative mutation path, assignment-value fact, mutable-self parameter,
  field/type identity, and source statement; other body shapes publish no
  callable row. A separate checked-only Unit catalog admits one terminal,
  argument-free requirement call through either a direct local descriptor or
  one exact same-interface reassignment. Like the scalar lane, each version may
  name a different closed conformance. It retains the complete application
  and operation-free callable roster, source borrow/path, contracts, and exact
  call reach while carrying no scalar result binding or home. Nonempty bodies,
  arguments, service reach, realization state contracts, result discards, or a
  later statement fail closed. One transparent descriptor-parameter hop is
  also retained when the outer transfer rejoins one bare parameter of
  identical access and the helper's sole statement is that Unit requirement
  call. The plan keeps both call coordinates and the parameter identity.
  Every Terminal/native carrier remains a later boundary. A
  proof-static `term.member` binder argument resolves in its named-contract
  scope to the exact checked evidence-term handle and one unambiguous direct or
  inherited requirement row. The row retains the declaring trait's normalized
  argument pack; unknown and ambiguous members reject, and the erased
  projection cannot select an executable machine parameter. Boolean,
  membership, fact-only, or non-nominal bindings reject.
- Result-case guarantee groups resolve their source path only against the
  machine's declared result sum and retain one exact nominal case identity on
  every checked row. A declaration layer admits at most one group per case;
  public named selectors stay machine-wide unique. Producer coverage is
  path-sensitive: named rows require exactly one evidence assignment on every
  ordinary exit producing the case, while unnamed rows require one proved
  proposition on every such path and retain no source-bindable term. Other
  cases and crash exits discharge neither. Caller fact import and named capture
  occur only after the matching case refinement. Each imported row derives its
  validity from the result occurrence, normalized referenced occurrences, and
  evidence-interface scopes, so intersecting writes invalidate borrowed or
  revision-scoped guarantees rather than leaving stale sibling facts.
- `checked-trees/src/admissibility/` owns checked operation acceptance
  views. These views do not re-run proof, borrow, or effect checks; they gather
  the already-accepted evidence behind state, statement, call, and exit query
  methods so later stages and reports have one obvious doorway. Each view also
  exposes an `AcceptanceSummary` with borrow, proof, effect, boundary, and
  termination dimensions. The summary derives its aggregate accepted/rejected
  verdict from those dimension records instead of duplicating a caller-supplied
  decision. `AcceptanceView` is the shared query surface for states,
  statements, calls, and exits, so later stages can ask the same question of
  every checked operation-like thing. Each dimension carries verdict, evidence
  count, diagnostic count, and provenance so accepted-by-construction checks and
  future diagnostic-backed rejections share one durable status shape.
  `types.rs` owns the public acceptance handles/verdict/summary records,
  `operation.rs` owns the state-local statement/call/exit operation wrapper,
  `state.rs`, `statement.rs`, `call.rs`, and `exit.rs` own the corresponding
  view APIs, and `helpers.rs` owns shared arena-span accessors.
- `flow.rs` assembles checked flow facts. `flow/builder.rs` owns the
  machine/state conveyor, `flow/state.rs` owns per-state flow fact assembly and
  entry/exit semantic envelopes, `flow/context.rs` owns the mutable arena
  bundle including ownership-event arenas, `flow/constraints.rs` materializes
  borrow constraints,
  `flow/borrow_lifetimes.rs` owns loan activation/weakening rules,
  `flow/statements.rs` owns statement entry facts, call fact sequencing, loan
  activation, mutation invalidation, and transfer propagation,
  `flow/transfers.rs` owns statement fact transfers and emits the narrow
  checked-only parameter- or prior-state-local-rooted structural qualification-
  correspondence ledger; its producer requires every source/source-occurrence/
  destination root to belong to the formation machine or exact formation state,
  requires an exact-state local to have one declaration strictly before the
  formation statement, independently re-resolves its declared type, and walks
  exact Field/Case plus literal in-bounds FixedIndex segments,
  `flow/calls.rs` owns call
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
  ownership-consuming places. That policy consumes the typed tree's normalized
  type multiplicity; it does not maintain a second partial primitive-name list,
  so constrained copy primitives cannot manufacture owned call transfers.
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
  each value matters, including machine ranking subjects, attached-data field
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
  index/subslice proof diagnostics,
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
  validation, `checks/contracts/prover.rs` owns contract fact and call-entry
  proof dispatch, `checks/contracts/prover/booleans.rs` owns recursive boolean
  expression proof traversal, `checks/contracts/direct.rs` owns direct boolean
  fact matching,
  `checks/contracts/domains.rs` owns domain-membership proof fallback,
  `checks/contracts/labels/calls.rs` owns call-site contract expression label
  substitution, `checks/contracts/labels/domain.rs` owns domain proof label
  substitution, `checks/contracts/places.rs` owns contract-place matching, and
  `checks/contracts/evaluator.rs` owns the call-site expression evaluator
  context and entry surface, `checks/contracts/evaluator/booleans.rs` owns
  boolean expression folding, `checks/contracts/evaluator/integers.rs` owns
  integer expression folding, `checks/contracts/evaluator/collections.rs` owns
  collection-length folding, and `checks/contracts/evaluator/resolution.rs`
  owns call-site parameter, local, indexed-literal, and struct-field expression
  resolution.
- `checks/termination.rs` is the termination-check entry point.
  `checks/termination/order.rs` owns ranking-order recognition,
  `checks/termination/graph.rs` owns direct recursive graph shape checks and
  the shared named-transition target-state normalization consumed by ranking
  and checked-progress subject correspondence,
  `checks/termination/ranking/mod.rs` owns supported ranking dispatch,
  `checks/termination/ranking/ranges.rs` checks the range of the produced rank,
  including constraints authored on acyclic bodies. Its immutable single-state
  tier consumes exact endpoint handles and enforced integer parameter bounds;
  natural distance is `max(limit - index, 0)`, not the cursor value. Nonzero
  floors and exclusive ceilings are accepted when proved. A dependent endpoint
  may reuse the exact view bound only because the decrease proof separately
  requires it to remain pinned. Named-state transport, other dependent endpoints,
  mutable premises, and custom-view range facts remain unproved in this tier;
  normalized display strings and concrete caller literals are not evidence.
  `ranking/ranges/relational.rs` adds an exact single-state relational tier
  when the static range proof is insufficient. It establishes entry membership
  from declared bounds and machine requirements without borrowing a backedge
  guard, including on acyclic bodies. Each actual self-edge then proves
  membership, fixed range endpoints, and strict descent using simultaneous
  parameter substitution and its live guards. The shared
  `validation/src/contract_entailment/ranking_range.rs` query uses the existing
  strict-symbol arithmetic engine; evaluated prefix expressions undergo
  builtin-meaning checks separately from guard hypotheses. An `IncreasingTo`
  limit remains pinned, while either ranked `BoundedDistance` subject may
  change. Raw distance represents the natural rank only after its nonnegative
  branch is proved; the original clamped static tier remains intact. Range
  membership alone cannot authorize termination. A successful static range
  proof first follows the existing decrease recognizer. If that recognizer
  cannot prove the actual step, the complete relational entry-and-edge proof
  may establish descent, retaining its membership and endpoint-pinning checks.
  Named-state, mutable, custom-view, and call-component range transport still
  need further proof support.
  Parameter delivery establishes the range these proofs consume:
  `validation/src/calls/argument_bounds.rs` applies the shared store-containment
  check to numeric arguments using their evaluation snapshots and guard polarity.
  `validation/src/transitions.rs` resolves both machine-entry and named-state
  target symbols before validating their signatures. Immutable singleton
  parameters may supply literal-equivalent guard bounds; varying parameters
  cannot be treated as constants.
  `checks/termination/ranking/patterns.rs` owns shared recursive-transition and
  parameter-expression matching, `checks/termination/ranking/nat.rs` owns
  natural-number ranking proof shapes, `checks/termination/ranking/nat/guards.rs`
  owns natural-number guard predicates,
  `checks/termination/ranking/nat/arguments.rs` owns natural-number next-argument
  rewrite predicates. `checks/termination/ranking/slice.rs` owns runtime
  slice-edge routing; it shares `validation/src/slice_ranking.rs` with
  proof-machine recursion. The shared rule requires the exact slice parameter,
  its nonempty length guard, and its `1..` tail. Each caller establishes guard
  dominance and binding stability; a false-arm call or rebound descriptor
  cannot consume that premise. Retained ranking subjects, view arguments, and
  ranges resolve in the entry signature before checking, not by display-name
  recovery at a recursive edge.
  `validation/src/call_cycles/runtime_ranking.rs` checks runtime machine-call
  components against one natural or authored lexicographic ranking. Each tail-call
  occurrence must preserve the rank or strictly decrease it; the graph of
  preserving occurrences must be acyclic. A strict branch therefore cannot
  hide a stalled alternative or subcycle. The projection retains the exact
  entry parameter, nominal record, measure occurrence, and ordered field
  declarations. Ordinary unstamped member expressions resolve only within
  that exact record; mismatched nonzero field symbols reject. Unsigned
  component descent consumes the current arm's guard or an earlier failed
  dispatch guard over that same entry binding, with builtin operator meaning,
  never a same-spelled authored operator. State rebinding and
  unknown effects require a separate arrival judgment and are not admitted by
  this entry-parameter check. Scalar `Nat::Descending` parameters use this
  same whole-component judgment, not a separate pair-level subtraction
  recognizer. Direct stores may preserve a scalar rank when the existing
  alias-closed assignment frame proves every target disjoint from its exact
  parameter. Both assignment operands must retain inert builtin meaning;
  a disjoint destination cannot hide a call, borrow, or authored arithmetic
  effect on its right-hand side. Projected ranks retain the conservative
  write fence. Ranges and view arguments need their own substitution proof.
  The witness remains private implementation
  evidence and does not author a public completion guarantee.
  Proof-only components retain their strict structural-subterm rule. Their
  certificates must cover every resolved cross-machine call occurrence for
  each caller/callee pair; an unclassified receiver call cannot disappear from
  the graph or borrow a parallel call's strict-descent certificate.
  `checks/termination/progress.rs` independently replays
  retained qualification correspondence before deriving checked progress
  summaries; malformed, label-only, out-of-bounds/nonliteral/runtime-indexed,
  later/missing/duplicate/reordered/type- or symbol-substituted local,
  foreign-machine, or sibling-state-rooted correspondence fails closed.
  `checks/termination/progress/lineage.rs` carries finite exact entry-subject
  alternatives across local states. `lineage/places.rs` selects finite owned-field
  partitions along demanded subjects. Discovery follows only their incoming
  dependencies; unused aggregate subtrees are not expanded. A parameter may
  borrow that aggregate; nested references, recursive proof shapes, arrays,
  and unresolved generic
  shapes remain opaque leaves rather than referent snapshots. The most specific
  field partition wins even when unseen or unknown: an enclosing identity cannot
  restore a replaced field. `lineage/transfers.rs` appends the destination's
  field projection to the resolved argument before querying its captured origin.
  Source rebinding consumes the matching partition prefix, so field permutations
  and finite nested replacements retain all exact alternatives. A nonempty
  residual projection edge with a return path grows on a cycle; once seeded,
  that destination and its dependents cannot supply finite caller premises.
  An unseeded cycle stays
  unseen and cannot poison a reachable join. Identity cycles, parameter exchanges,
  and acyclic projected transfers retain their exact alternatives. Unused
  growing parameters do not prevent checked termination. The remaining
  correspondence converges without a projection-length or iteration limit;
  qualification receipt replay is unchanged.
  `checks/termination/progress/origins.rs` resolves a fully projected call
  premise through preceding owned assignments and local captures. Backward
  substitution uses the source at the copy point, not after later source
  writes. Shared complete call frames and alias-closed storage writes preserve
  only disjoint subjects; same-statement calls use retained execution order,
  not authored call ordinals. The selected stored type must be reference-free:
  constrained references and reference-containing copies are not snapshots of
  their referents. Local-state field transfers use this same origin query;
  disjoint fields retain independent origins even when the aggregate's whole-value
  correspondence is unknown. Bare local shared and mutable references resolve
  their live referent before value tracing, including after an owned capture
  moves the query to an earlier point. A local reference-binding replacement
  does not count as a store to its previous referent. Arrivals through stored
  reference carriers, opaque leaves, and helper-output
  correspondence remain conservative. Exact live receipts and build-bound
  provider subjects retain their existing separate handling.
  `checks/termination/progress/components.rs` closes private summaries within
  each revalidated runtime call component. Validation and this query share the
  complete call graph, including proof dependencies; a runtime subset of a
  mixed component cannot establish progress. Other calls keep their selected
  contract or checked-summary obligations. A component is reconsidered when
  an external private summary becomes available, and unknown external progress
  prevents inference. Premises and build-bound demands converge as exact sets,
  not by discovery order. No inferred summary changes a published guarantee.
  `components/projections.rs` bounds finite premise transport from the actual
  parameter roots, argument-prefix lengths, external schemas, and exact receipt
  paths. A growing recursive-reference demand retains `NoGuarantee`; it is not
  truncated into a finite promise. This is not an arbitrary iteration budget
  or a bound on general local-state lineage. Shared call-parameter lookup maps
  an exact machine-head target to its entry parameters and generic context,
  while an explicit subordinate-state target retains its own parameters.
- The checked-lowering regression root `tests/termination.rs` is orchestration
  only. Its subordinate modules separately own ranking witnesses, operational
  contract publication, exclusive-cycle write frames, indexed-call write
  frames, returned-place write frames, assignment-value write frames, checked
  crash routes, and data-fact propagation. Test fixtures may share the exact
  symbol lookup helper, but they must not recombine those semantic families in
  one permutation-oriented test parent.
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
- Grow the checked operation acceptance summaries from accepted evidence counts
  into persisted verdict records with diagnostic provenance for effect/capability
  authorization, proof discharge status, borrow failures, termination failures,
  and backend policy linkage.
- Keep contract, proof, borrow, range, termination, and effect checks split by
  noun ownership instead of letting `checks` files become semantic junk drawers.

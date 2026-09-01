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
rejects every unresolved stand-down.

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

| Noun | Ownership |
| --- | --- |
| Places | First strongly useful place layer via `psi_facts::Place` and checked-flow `CanonicalPlace`. |
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

Must own:

- Proof obligations and whether current facts discharge them.
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
  call shape now accepts one direct write-only literal fixed-array parameter
  root projected by exactly one in-bounds literal `FixedIndex` to an
  unrestricted non-Atomic primitive element; checked and Terminal replay retain
  that complete one-segment path.
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
  authored axis only; suspension never supplies blocking or vice versa.
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

  Open caller binders, nested open arguments, return-only or forwarded consts,
  unresolved nominal identity, named explicit const declarations, and
  constraints without a closed structural replay remain unavailable rather
  than being mislabeled concrete. D29's mention of `where`
  requirements means requirements expressible by the operator model; this
  work does not incidentally invent a general operator `where` surface.
  Missing rows are not coverage and cannot be filled from the pre-D29 indexed-
  provider scaffold.

  Omega's selected-provider owner now supplies the exact generic checked-body
  requirement/provider symbols before final checked lowering. For direct named
  call roots, including normalized unit statements, Psi independently rederives
  every closed type/const application from authored operands, keeps the authored
  generic declaration unchanged, and clones one private authoritative
  specialization per distinct application. Each specialization retains the
  exact closed operator realization and commits it with its template,
  substitutions, selected conformances, and machine contracts. Package review
  rejoins that custody to the strong selected plan. This is the local direct-
  call checked-body semantic cohort only: nested calls, fixed-token generic,
  external generic, symbolic cross-artifact, Terminal companion, and D32
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
  `psi-checked-trees/src/flow/semantic_dependencies.rs` owns its durable checked
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
  `psi-checked-trees/src/borrow.rs` owns the grouped `BorrowFacts` root and
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
  one-hop exclusive reactivation: a direct mutable parent lends an exact
  mutable or write-only child while other non-overlapping sequential siblings
  may occur in the state, and that child ends by `LastUseExpired` with exact
  `Reactivate` and `ExclusiveSuspension` evidence, and the same boundary enters
  one receiver-free call with one exact mutable-reference parameter over the bare
  parent carrier whose mutation summary is the complete restored referent. The row independently
  rejoins both resources, weakening, disposition, containment, flow and borrow
  calls, the carrier-read access, parent-loan entry constraint, captured
  places, restored access, and target. Shared, multihop, concurrent-sibling,
  state-exit, projected, receiver, extra-parameter, direct-assignment, and
  nonmutating shapes remain outside this transactional replay. It grants no
  Terminal authority. The downstream Terminal consumer now publishes exact direct-root
  custody for a finite nonempty linear exclusive lineage whose direct root is
  `Mutable`, whose edges are `Mutable`-to-`Mutable`,
  `Mutable`-to-`WriteOnly`, or `WriteOnly`-to-`WriteOnly`, and whose leaf
  independently replays a state-exit handoff to that exact root lifetime.
  Shared cohorts, exclusive branching, and post-reborrow-use publication remain
  open. Root handoff grants the borrow layer no cleanup, transfer, or
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
  exact, runtime arguments are empty, and recursive fields contain no lifetime
  application. Raw checked lifetime spellings stay distinct; the erased
  physical representation and loan size share the same sealed resolver.
  Generic normalization rewrites
  concrete-machine cast targets and supports recursively nonzero literal-array
  type arguments. Record eligibility and representation recursion use exact
  symbol identity and require a nonzero, quotient-free, acyclic, all-relevant,
  recursively fact-free shape. Runtime or merely bounded offsets, slices, total
  zero-size targets, open/unresolved or mixed/recursive/custom-canonical
  structured-const, nested/array or malformed/nonphantom lifetime applications,
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
  generic, reference-bearing, or other computed local roots remain opaque.
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
  contents without redirecting their origins. Sibling direct value-call
  arguments are independently admitted when each one's receiver and arguments
  are non-rebinding and every call frame is complete, including nested direct
  calls to a maximum call-tree depth of two. An explicitly discarded concrete
  primitive result from a nongeneric internal checked-body call is likewise
  neutral under that complete-frame rule. Other discarded call results,
  explicit binding reborrows, deeper computed arguments, and any opaque node
  remain fences.
  A free or attached helper whose terminal place is rooted in one
  mutable-reference parameter composes exact member suffixes or absorbing
  collection-coarse indexing onto that argument's origin through its call
  result and later transparent chains. The terminal place may follow a prefix
  of effect-free caller-isolated scratch locals and direct local `&mut` aliases,
  including mutable bindings and results of other structurally transparent
  helpers. A caller-isolated scratch local may be initialized by a direct-call
  tree through depth two when every inferred frame is complete and all writes
  resolve into previously established caller-isolated scratch locals. Deeper,
  recursive, computed, opaque, or externally writing initializer calls remain
  fences. A
  validated mutable recast local with an effect-free source may write through
  that source without obscuring a separately returned parameter origin.
  The same exact returned-place relation composes when such a result is supplied
  directly as a statement-call argument.
  Value-shaped assignments may write through those origins without changing
  the relation when the right-hand side is effect-free or a typed
  non-reference direct-call tree through depth four with complete frames;
  sibling branches are admitted independently and all nested-call writes remain
  published. One deeper, binding-reborrow, recursive, or opaque branch fences
  the whole right-hand side; reference-valued roots keep their existing
  relational handling. A direct primitive scalar assignment value may wrap
  complete caller-isolated call producers in up to twenty unary, binary,
  primitive-cast, member-projection, or indexing shells under the same call
  budget; a twenty-first shell and generic/reference/unknown call results remain
  conservative. Aggregate fields and projected concrete record, selected-case,
  or fixed-array literals retain their separate two-shell computation budget.
  One top-level concrete primitive-only record or
  selected-case literal may likewise contain an independently bounded call
  tree in each direct common or payload field. Direct typed assignment values
  may nest concrete primitive-only record, selected-case, and literal
  fixed-array aggregates through depth three, with the same rule at every
  primitive leaf. A declared primitive field at any admitted aggregate level
  may also contain up to two nested scalar
  computation shells made from unary or binary operators, primitive value
  casts, member projections, or indexing; their effectful leaves are
  independently bounded non-reference call trees. This direct aggregate
  depth-three rail and computed depth-two rail do not change the depth-four call
  budget. A fourth direct aggregate level, generic, recursive, or
  reference-bearing call result, and other computed field shapes remain
  fences. Projected record/case fields and direct fixed-array indexing retain
  their separate aggregate-depth-two rail, so a third projected aggregate also
  remains conservative.
  A primitive assignment may also project one direct field from a concrete
  caller-isolated record or selected-case literal whose effectful fields are
  bounded direct-call trees. That projection may sit below one further unary,
  binary, primitive-cast, member-projection, or indexing shell: the projection
  consumes one of the existing two computation shells, so a third shell still
  fails closed. A direct primitive index projection from a fixed-array literal
  follows the same shared budget. Every eagerly evaluated element publishes
  its bounded call writes; either one element computation shell or one outer
  scalar shell may use the remaining depth, while combining both remains a
  third-shell fence. A third aggregate level remains conservative at this
  projection site.
  Targets may project through a stable helper-local
  mutable alias or an exact transparent call-produced place. An indexed target
  may contain one or more indexes whose non-rebinding direct-call trees are
  independently complete through depth two; the first index fixes the
  collection-coarse write, later indexes are absorbing, and every index-call
  write remains published. The compiler-owned `as_mut_slice()` view may occur
  on the collection spine, including after a stable helper-local alias, a
  transparent free helper result, or an attached helper result rooted in its
  actual `self` receiver: it preserves that source's backing array origin before
  the first of one or more indexes coarsens it; later indexes stay absorbed and
  each bounded index frame publishes. Recursive or opaque free/attached view
  producers remain fences. An exact member projection carried by a stable alias
  or produced by a helper may precede the view: its suffix composes before the
  view preserves that exact origin for later indexing; any member after that
  index remains absorbed by the coarse backing collection. A transparent free
  helper result or an attached helper result rooted in its actual `self`
  receiver likewise supplies the collection origin without an intermediate
  binding. An exact member
  projection may follow that result before one or more indexes:
  the suffix composes first, the first index coarsens to that nearest
  collection, and later indexes or members remain absorbed while every
  independently bounded index frame publishes. Deeper or binding-reborrow
  index trees and recursive
  or opaque free/attached collection producers remain fences. The
  bounded indexed target and bounded non-reference value tree may coexist on
  one assignment; their frames compose independently, while either side
  exceeding its rail fences the relation. A compiler-owned mutable-slice view
  on the target collection is neutral to that composition: the target index
  and value tree retain independent depth-two and depth-four budgets,
  respectively, and publish all call writes. Other ordinary exact frames
  remain published, and effect-free
  discarded expressions and direct Unit statement calls with complete
  non-rebinding frames are neutral, including exact sibling direct value-call
  arguments and their bounded two-level direct-call trees. An
  internal statement call may also take a mutable indexed argument whose index
  is such a tree: caller-alias-aware instantiation coarsens callee writes to the
  argument's collection and publishes index-call writes. The compiler-owned
  `as_mut_slice()` view is neutral on that argument spine, including after a
  stable helper-local mutable alias, a transparent free helper result, or an
  attached helper result rooted in its actual `self` receiver: the callee write
  rebases through the alias or helper and view to its backing array before the
  index coarsens it. Deeper index trees and
  recursive or opaque free/attached view producers remain fences. An exact
  member projection may be carried by the stable alias or follow a free or
  attached helper result before the view; the suffix composes before view
  preservation and index coarsening, while a member after the index remains
  absorbed by the coarse backing collection. With repeated indexes, the first
  fixes that coarse collection, later indexes stay absorbed, and every
  independently bounded index frame publishes. The indexed argument may
  project through a stable helper-local mutable alias; that alias's established
  origin supplies the collection. It may also index a
  structurally transparent helper result directly; the helper's returned-place
  relation
  supplies the collection without an intermediate binding. This includes an
  attached helper rooted in its actual `self` receiver. An exact member
  projection may follow the helper result before one or more indexes. The
  member suffix composes first, the first index coarsens to that nearest
  collection, and later indexes or members are absorbed; each index expression
  independently satisfies the same bounded-call rule.
  Recursive or opaque free/attached collection producers, boundary calls, and
  deeper or binding-reborrow index trees remain fences. A direct
  helper-local alias rebind updates that local's origin while
  prior reborrows retain theirs; a structurally transparent helper result may
  supply the replacement through the same origin algebra. Other computed
  rebinding and other computed initializers remain opaque. An explicitly
  discarded concrete primitive result from a nongeneric internal checked-body
  statement call is neutral when its inferred frame is complete and its
  receiver and arguments obey the same bounded non-rebinding rule; the frame's
  side writes remain published. Discarded reference-bearing or aggregate
  results, boundary or generic calls, statement calls with binding reborrows or
  opaque frames, and opaque or recursive result producers remain opaque.
  Terminal returned places, stable local mutable aliases, and direct alias
  rebind replacements may contain one or more indexes whose non-rebinding call
  trees are independently complete through depth two. The first index fixes
  the coarse collection origin; later indexes are absorbing, every index frame
  publishes, and only the rebound name moves while prior reborrows retain their
  origins. A compiler-owned `as_mut_slice()` view before the first index
  preserves the returned, initializer, or replacement collection's backing
  origin. Deeper, binding-reborrow, recursive, or opaque index forms remain
  fences.
  For an
  attached helper, its actual receiver supplies the caller origin when the
  result is rooted in `self`. Other
  nontrivial results remain opaque; signature lifetime elision alone is not
  relational frame evidence. An unsummarized body falls back to its receiver
  plus every potentially exclusive place argument.
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
- `psi-checked-trees/src/flow.rs` owns the published checked-flow fact model
  export surface. The model is split by semantic noun under
  `psi-checked-trees/src/flow/`: `contexts.rs` owns semantic/borrow
  constraint refs, `invalidations.rs` owns mutation/domain invalidation facts,
  `borrow_lifetimes.rs` owns activation/weakening facts, `ownership.rs` owns
  move/drop facts, `boundaries.rs` owns boundary-edge facts, `control.rs` owns
  state/statement/call/exit facts, and `roots.rs` owns grouped `FlowFacts`
  roots plus query helpers. Flow construction should join each noun root
  through its root constructor rather than hand-building the grouped fields.
- `psi-checked-trees/src/facts/` owns checked semantic facts that are not
  part of the temporal flow spine: `invariants.rs` owns invariant definition
  facts, and `domains.rs` owns domain dependency facts and dependency-path
  accessors. Both expose root constructors so invariant and domain production
  joins arena roots explicitly. Suspension and blocking publication project
  separate machine-keyed rows from the transient operational inference plan
  after flow/service construction; each checked root consumes only its own
  boolean axis and preserves its independent published interface.
- `psi-checked-trees/src/facts/contract_plans.rs` owns each machine's normalized
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
  than borrowing the closed catalog semantics.
  Other source forms remain explicitly transitional pending their artifact-
  relative carriers.
- `psi-checked-trees/src/proof/` owns proof-facing checked facts:
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
  lanes, direct named-conformance calls, and dynamic dispatch remain
  conservative fences. Package review already owns the declaration-level lane
  compatibility surface; this private call-site carrier adds no package
  schema. A
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
- `psi-checked-trees/src/admissibility/` owns checked operation acceptance
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
  checked-only parameter-rooted structural qualification-correspondence ledger;
  its producer requires every source/source-occurrence/destination root to
  belong to the formation machine or exact formation state and independently
  walks exact Field/Case plus literal in-bounds FixedIndex segments,
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
  `checks/termination/ranking.rs` owns supported ranking dispatch,
  `checks/termination/ranking/patterns.rs` owns shared recursive-transition and
  parameter-expression matching, `checks/termination/ranking/nat.rs` owns
  natural-number ranking proof shapes, `checks/termination/ranking/nat/guards.rs`
  owns natural-number guard predicates,
  `checks/termination/ranking/nat/arguments.rs` owns natural-number next-argument
  rewrite predicates, `checks/termination/ranking/slice.rs` owns slice-length
  ranking proof shapes, `checks/termination/ranking/slice/guards.rs` owns
  slice-length guard predicates, and
  `checks/termination/ranking/slice/arguments.rs` owns slice-tail next-argument
  rewrite predicates. `checks/termination/progress.rs` independently replays
  retained qualification correspondence before deriving checked progress
  summaries; malformed, label-only, out-of-bounds/nonliteral/runtime-indexed,
  foreign-machine, or sibling-state-rooted correspondence fails closed.
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

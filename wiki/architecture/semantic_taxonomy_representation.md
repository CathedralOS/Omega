# Semantic Taxonomy Representation Rework

Status: **high-priority compiler architecture task**, loaded 2026-07-18.

The domain-facet, machine-taxonomy, core-multiplicity, service/operational, and
termination/ranking settlements are not documentation-only classifications. They make distinctions that affect
resolution, interface identity, admissibility, ownership, diagnostics, and
lowering. If the compiler stores only the old undifferentiated shapes and
reconstructs the distinctions later, it loses a clean representation and will
eventually encode policy in scattered conditionals.

This record audits the current loss and defines the migration target. Decision
22 supplies the reach algebra and decision 23 supplies the termination
guarantee/ranking-witness firewall. The compiler must represent both directly
rather than extending old booleans and bitsets by convention.

## Current representation loss

### Domains

`psi-symbol-resolved-trees/src/domain.rs` and
`psi-typed-trees/src/domain.rs` represent every domain as one
`DomainDefinition` containing independent predicate-body, semantic-role,
establishment-route, alias, fact, and operator records. Operator-bearing source
declarations are projected once during syntax-to-resolved lowering into the
closed
`DenotationDimension` role; downstream consumers do not infer semantic roles
from operator presence. Establishment relationships are normalized after
symbol assignment as exact source identities and copied through typed domain
definitions and binding-site constraints. The record does not yet represent:

- denotation schema and implicit-weakening certificate/sealed theory;
- the distinction between flow knowledge and binding-site semantic
  qualification; or
- canonical representation-qualification conformance.

`psi-checked-trees::DomainFacts` is appropriately fact-shaped for predicate
membership, while qualification casts and emitted semantic commitments consume
the declaration's stable semantic identity and inspect predicate-body presence
only when a proof obligation is required. Operator selection consumes the
explicit semantic-role record. Arithmetic policies survive through
compiler-specific `ArithmeticDomain` paths and contribute the closed
`ArithmeticPolicy` role during conjunction validation.

### Machines

The symbol-resolved and typed `Machine` records carry `boundary: bool`, a
normalized `termination_guarantee`, a private `RankingWitness`, a flat
effect-name span, contracts, and states. They do not yet
carry a normalized semantic contract, supply mode, consumption eligibility,
observation surface/floor, progress contract, or boundary-facing contract
identity. Requirement, provider, checked body, and accepted declaration are
therefore liable to be inferred from syntax and lookup context repeatedly.

The guarantee/witness split now represents an inherited guarantee with an
implementation-local witness and lets a witness change leave interface
identity untouched. Checked summaries derive local completion independently;
the cycle gate covers state SCCs, same-shaped runtime machine SCCs, and
structural non-tail proof-only SCCs. The remaining gap is the full conditional
guarantee with pinned progress premises and its boundary-facing contract
identity.

### Multiplicity and permissions

`Multiplicity` is first-class from source properties through typed trees.
Checked flow records `Establish`, `Transfer`, `Consume`, and `AffineDrop`, and
those events survive through machine bytes and backend reports with
multiplicity, explicit `Owned | Shared | Exclusive` access, transfer-stable
establishment provenance, and conditional-payload debt. Legacy-derived affine
cleanup is deliberately `Unknown` provenance rather than fabricated evidence.
Borrow activations/weakenings now feed the same context as
`Unrestricted + Shared` and `Affine + Exclusive` entries, respectively. The
linearity judgment now consumes only the qualified permission events. Affine
cleanup derives directly from typed state ownership, so the legacy drop arena
is compatibility output only. Semantic transfers and consumes run canonical
typed move discovery through an independent event sink, so the legacy move
arena is also compatibility output only. The deliberately deferred work is the
multi-resource/nested obligation algebra, not reconstruction from lossy
move/drop summaries.

### Service reach and operational behavior

`omega-effects` now keeps symbol-resolved service reach separate from recursive
suspension/blocking inference. Both plans retain the same grouped
machine/state/call topology, but service rows are interned canonical trait
identities while operational summaries are independent booleans. Authority,
trust, resources, failure, and mutation remain in their separate semantic
homes.

## Target representations

### Authority values and proof-boundary data

Authority-bearing runtime values use ordinary data layout. Their normalized
type representation derives from published fields, while domain facts carry
authority, validation, provenance, and rights without runtime metadata.
Provider-owned backing may be addressed by an ordinary key field whose
operations remain behind the provider boundary.

`DataSupplyMode::BoundaryOpaque` remains the representation mode for
proof-boundary data whose carrier is supplied by admission, such as abstract
`Real`. Runtime authority declarations migrate from that mode to ordinary data
as routed establishment, receipt-backed boundary guarantees, and
resource-frontier transformations land.

The compiler records whether each domain fact originated through checked proof,
an authorized checked conformance, validation, resource transfer, or accepted
boundary evidence. The domain-authorized requirement and provider receipt
contribute to trust identity; private proof and transformation witnesses remain
implementation evidence.

Trust composes by the weakest supporting input, while provenance retains every
input that caused the downgrade. A derived arithmetic or graph proof over one
admitted environmental premise produces an admitted composite fact naming
that premise; a dominant derived input never hides it. This one rule applies
to boundary-issuance evidence, provider facts, foreign stack/callback plans,
timing conversion, and later normalized evidence products.

See
[`../design_briefs/authority_values_and_boundary_evidence.md`](../design_briefs/authority_values_and_boundary_evidence.md).

### Domain theory

Introduce a shared domain model used by symbol-resolved, typed, and checked
layers:

```text
DomainTheory {
    carrier,
    static_index_parameters: CanonicalIndexSchema,
    index_constraint: Option<NormalizedIndexExpression>,
    predicate_body: Option<PredicateBody>,
    semantic_roles: NormalizedRoleMap,
    establishment_routes: NormalizedEstablishmentSet,
    alias_expansion: NormalizedDomainExpression,
}
```

These are independent records rather than a mutually exclusive enum because
hybrids are first-class. Checked types/bindings carry normalized static
qualification; flow facts carry proven membership. The deterministic
normalizer owns semantic interface identity. Layout continues to use the
carrier ABI.

The index fields land in three ordered stages: structured canonical const
values, closed indexed domains, then computed open result indices. The source
header binds and reuses the carrier explicitly:
`domain<T, const U: Unit> T::Quantity<U>;`. A closed index stores its canonical
value. An open generic expression also records the exact selected
algebra-instance and normalized public operation-contract identity;
compatibility evidence is retained separately and never rewrites semantic
identity. The normalizer implementation version is artifact provenance, not a
fingerprint input. Index eligibility is structural and cannot be supplied by
an ordinary conformance. Because indices and domains erase, neither field
changes carrier layout or SIMD shape.

Current implementation canonicalizes structured const values before resolution
and uses their type plus structural encoding as generic identity. Closed domain
families preserve typed telescopes, canonical arguments, and per-instance
semantic identity without changing carrier ABI; specialization refreshes that
identity after substitution. Open indices retain their exact algebra authority,
expression, compatibility conditions, and discharge evidence. Unresolved
equality fails closed without ambient theorem search. Quotients and
default-domain-constrained values remain blocked on canonical representative
and proof admission.

Current domain representation keeps predicate bodies, semantic roles,
establishment routes, aliases, and flow evidence independent. Operator homes
come from an exact domain-qualified name or one unique declared-domain
constraint across the operand tuple; operators do not grant establishment.
Conjunction combines roles by axis and rejects competing contributors to the
same role.

Domain propositions normalize from `requires`; exact requirement identities in
the body are the only authored establishment routes. Empty atomic domains may
be qualified vacuously, while predicate-bearing domains require proof and
routed domains require an exact authorized exit. No package ownership,
attachment name, machine placement, or former privileged qualification trait
confers establishment authority.

Checked facts retain an evidence class—proof, validation, authorized route,
checked transformation, admitted receipt, propagation, or vacuous
qualification—separately from program-point origin. Selected provider plans and
linear result claims retain exact identities in checked artifacts, so resource
receipts can match subjects without parsing result types.

Transparent aliases expand recursively to atomic facts before identity,
compatibility, admission, and route normalization. Empty, cyclic,
cross-carrier, unknown, and public-to-private expansions reject. Generic
binding constraints preserve the complete normalized domain records through
substitution and snapshots.

Operator selection reads only static binding qualification and signature
requirements, never flow facts. Candidate matching uses the complete operand
tuple with one generic substitution; return types do not select meaning.
Inactive theories may coexist, but multiple active meanings at a use reject.
The language still has no authored open-family or dispatch-owner-position
surface.

### Carry policy

Retire the provisional `send: bool` projection. Carry is a normalized
compiler-semantic product, propagated through every tree/plan layer without
re-derivation:

```text
CarryPolicy {
    suspension: Forbidden | Allowed,
    cpu: Origin | Any,
    host_thread: Origin | Any,
    address: Stable | Movable,
}
```

This is not ordinary `omega::core` data. Source `[carry(...)]`, transparent
structural derivation, and per-claim permission facts all lower into this one
representation. Accepted resource claims originate with the strict policy.
Their result contracts may establish the four positive compiler facts
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`; `Carry::Portable` transparently expands to the complete
permission conjunction before normalization.
Permission entries retain any provenance anchor needed to interpret `Origin`
or `Stable`. Checked-internal claims derive from the provenance they inherit.
Transfers and conserved splits preserve permissions, combined origins select
the most restrictive demand per axis, and qualification forgetting leaves the
permission entry live until the underlying claim is discharged. Aggregates
share a field traversal with other properties but each axis owns its
composition algebra.

Task-runtime behavior does not form a universal supply record or a fifth carry
axis. Activation planning derives only the obligations that the target machine
actually creates:

```text
StackPlan {
    bytes,                 // whole-call-graph WCSU + entry/calling overhead
    alignment,             // target/layout derived
    representation,       // fixed stackful lowering identity
}

ActivationCarryObligations {
    preserve_cpu: bool,
    preserve_host_thread: bool,
}
```

The settled stackful representation transfers one fixed, nonmoving
`StackLease` satisfying `StackPlan`; address stability follows structurally.
Suspension permission remains a local canonical-liveness judgment. Portable
activations demand no scheduler fact. An activation that may retain
`SameCpu`/`SameThread` values requires the selected scheduler to establish the
corresponding preservation conformance or admission receipt. Cancellation is
an operation/conformance, stack availability is resource reservation, and
inline completion belongs to the concrete `start` operation. There is no
`SafePoints | Asynchronous` provider mode.

The current compiler retains the closed carry axes, explicit claim origin, exact per-axis relaxations, canonical suspension crossings, and fixed nonmoving stack plans. Boundary admission is owner-authorized and receipt-bound; direct adapter calls mint no permission. State and call transfers preserve origin independently of later qualification. Remaining path-indexed aggregate, partial-move, multi-output, and provider-preservation work is tracked in `TASKS.md`.

Executable provenance and control-flow integrity must also remain separate.
`Artifact::AdmittedExecutable` plus linear placement states prove which bytes
may be installed. Backward-edge return integrity in checked Omega is derived,
not a second authored contract: memory safety and non-addressable compiler-owned
control state prevent ordinary code from forging or overwriting a live or
parked continuation, while WCSU proves provisioned stack capacity is
sufficient. Optional final-byte return validation and CET/PAC/shadow-stack
realizations reduce trust in the compiler or harden the target; they do not
define language semantics.

Forward-edge integrity remains a real representation property. Fixed direct
targets come from lowering; indirect calls and tail calls consume sealed,
requirement-compatible entry IDs; dynamic descriptors retain satisfier/contract
identity. Opaque executable providers either supply an admitted
`CallPlan + StatePlan` covering their exits or remain hardware-isolated.
Local dynamic descriptors, per-requirement operational envelopes, object
safety, and the no-cross-component rule are settled in
[chapter 14](../language_guide/chapter_14_traits.md) and
[calling plans](../design_briefs/calling_plans.md).

### Named conformance map

A whole-trait conformance is one closed implementation block, not a set
recovered by searching attached machines. Its normalized representation owns:

```text
NamedTraitConformance {
    subject: ConformanceSubject | None,
    trait,
    name: ConformanceName | Home,
    rows: [(declaring_trait, complete_requirement_overload) -> ConformanceRow],
    laws,
    provenance,
}

ConformanceRow =
    CheckedMember { machine }
  | ExistingMachine { machine }
  | InstantiatedDefault { declaration, substitutions }
  | SynthesizedMember { rule, evidence }
```

The inherited trait closure determines the required row keys. Complete overload
identity includes the normalized parameter signature and dispatch-bearing
result-domain set; source leaf names never serve as row identity. Each key
occurs exactly once; an uncovered row uses that exact overload's separately
instantiated default or rejects. Calls made by a default resolve through the
same map. The declaring package owns the closed row set, while other packages
may declare separately named conformances under the ordinary visibility and
collision rules. Public conformances may retain private member identities
because consumers name and invoke the authorized conformance surface.

An exact `machine ... satisfies Trait::requirement` realization is a different
semantic edge. It can supply a provider slot, operator, establishment route, or
proof citation without claiming the inherited trait closure. It therefore
cannot satisfy a whole-trait bound or license `dyn`. Dynamic descriptors,
carrierless selected evidence, and law-bearing consumers use only the complete
normalized conformance map above. Backend deduplication may share physical code
between rows without merging their semantic identities.

### Machine semantic contract

Introduce a normalized `MachineSemanticContract` (name provisional) containing
the complete substitutable contract plus an explicit `MachineSupplyMode`.
Syntax trees may retain source spelling, but provider admission, proof
artifacts, component manifests, compile-time evaluation, task-activation
checking, and lowering must consume this normalized object rather than
re-derive it from `boundary`, bodies, and effect names.

Consumption eligibility should normally be derived views/queries over the
contract, not stored independent booleans that can drift.

The supply representation must preserve the four settled variants directly:

```text
MachineSupplyMode =
    CheckedBody
  | RequiredBody
  | ExternalRealization { binding: NormalizedBindingId }
  | AcceptedDeclaration
```

`ExternalRealization` is sourced by `satisfies ... via <Binding>`. The binding
expression is compile-time evaluated and normalized before checked-plan
construction. It is not an executable body, and it does not author a trust
class or a second reach row. The satisfied requirement supplies the public
contract/ceiling; validation and admission check the binding/provider behavior
as a refinement and produce any trust receipt. `ProviderPlan` is then derived
from explicit conformance closure rather than authored rows.

Target profiles expose the same selection through typed slots:

```text
TargetSlotDeclaration {
    identity,
    schema,
    direction: EnvironmentToProgram | ProgramToProvider,
    binding_shape: ExactRequirement(requirement)
                 | CompleteConformance(trait)
                 | EntryMachine(entry_shape),
    lifecycle,
    cardinality,
    index_policy,
    installation_authority,
}

TargetSlotBinding {
    target_profile,
    slot,
    exact_machine_or_conformance,
    normalized_contract_or_map,
}

EntryShape {
    arrival_requirement,
    visible_parameters,
    result,
    receiver_provisioning: None | ProvisionedZii,
}
```

The slot declares the accepted semantic tier. Exact bindings expose only one
requirement contract; complete bindings retain the whole conformance map and
laws; entry-machine bindings select one source machine whose visible shape is
adapted by the target-owned entry schema. Direction distinguishes external
roots from outbound providers, while lifecycle and cardinality remain
orthogonal. The selected target owns required-slot completeness, so a
cross-profile binding, duplicate binding, or missing required build-bound slot
rejects.

An entry schema selects one physical arrival requirement, contributes
calling/state policy, and declares the source-visible continuation shape.
Generated bridges retain the arrival identity, provision an optional exclusive
receiver beneath an admitted entry root, and receive a compiler-derived
`MachineSemanticContract` whose crash, reach, write, work, stack/state,
provisioning, introduction, and provenance rows compose with the bound source
entry closure. The selected source machine is not a second physical entry
requirement.

Selected-provider closure also derives executable TCB metadata independently of
the machine contract:

```text
ExecutableEntryOrigin = StaticSelection | OmegaRuntimeAdmission

ExecutableEntry {
    provider_identity,
    executable_identity,
    provider_plan_identity,
    implementation_evidence,
    origin,
    execution_scope,
    containment_guarantees_with_evidence,
}

ScopeCompleteness =
    Complete { scope, evidence }
  | Incomplete { scope, attributed_providers }
```

Containment guarantees name memory isolation outside explicitly shared
authority, forcible termination, fault containment, and bounded resource use.
They compose by proved implication/set inclusion only where their scopes and
evidence agree. An opaque uncontained in-process provider forces
`Incomplete` for that address-space scope; known entries remain useful but are
not presented as exhaustive. This metadata is a selected-artifact property and
does not enter source service-reach identity.

The selected-plan carrier derives checked and compiler-known entries and marks
unadmitted opaque in-process rows incomplete. Opaque admission requires an
exact selected-row match and independent implementation/containment evidence;
pinning does not imply closure. Before installation, the profile gate checks
the exact manifest and retains attributed incompleteness. Paths, modules,
symbols, and table slots remain locators rather than executable identities.

The compiler's programmatic `ExecutableTcbBuildPolicy` keeps deployment trust
inputs outside source syntax. It binds each opaque admission candidate only
after exact provider selection, evaluates an optional profile over the
resulting manifest, and carries the sealed acceptance to the output installer;
a rejection occurs before the installer creates an output path. The existing
unprofiled compile entry remains available during migration. Selecting named
profiles through the ordinary `Build` package API remains pending API design.

Runtime admission uses a separate append-only ledger for one exact execution
scope. Its public admission boundary requires a pinned executable identity,
implementation evidence, and an Omega-mediation receipt; paths and loader names
cannot enter the ledger, receipt replay rejects, and callers cannot append a
manifest entry directly. Union with the static manifest marks every added entry
`OmegaRuntimeAdmission`. An admission without executable-closure evidence is
still a known entry but adds an attributed runtime incompleteness cause;
independent closure and containment evidence remain visible. Union is a
canonical idempotent set operation and rejects scope mismatch.

Isolation is represented as a manifest set rather than by flattening child code
into the caller. A selected closure chooses its nonzero isolated-scope identity
before opaque admission. The parent manifest receives an exact
`IsolatedProviderEndpoint` entry with independent endpoint/manifest admission
receipts and containment evidence; the child retains its own scope-relative
manifest. Attachment rejects child-scope drift, duplicate scope identities,
and entries attributed to the wrong scope. Parent and child completeness—and
therefore profile acceptance—remain independent.

Source `boundary` remains insufficient to reconstruct this enum: a checked
exported callable and an accepted bodyless declaration both mention the word
but have different supply modes. Likewise, body absence distinguishes a trait
requirement only in its declaration context. Populate the enum once during
semantic lowering and carry it thereafter.

Termination needs an explicit interface/implementation split:

```text
TerminationGuarantee = NoGuarantee | Terminates {
    premises,
    terminal_outcome_contract,
}

RankingWitness {
    subjects,
    ranking_view_id,
    optional_rank_range,
    cyclic_component_mapping,
    proof_artifact_id,
}

MachineTerminationPlan {
    interface: InternalDerived | Published(TerminationGuarantee),
    checked_summary: TerminationGuarantee,
    implementation_witness: Option<RankingWitness>,
}
```

`InternalDerived` is not serialized as an authored external promise. Exported
omission normalizes to published `NoGuarantee`; only local checked consumers
may exploit the tighter `checked_summary`.

The guarantee and explicit premises participate in published machine-contract
and external requirement-binding identity. `RankingWitness` does not: it feeds checker legality,
proof-cache identity, diagnostics, and provider-local revalidation. Stable
canonical defaults elaborate immediately to an explicit `ranking_view_id`;
the checker never selects a noncanonical view heuristically.

Current implementation: termination legality and checked
view facts resolve ranked subjects and argumented-view bounds from the
normalized `RankingWitness`; view and rank-range identity come from the same
witness. The legacy typed-machine decreases/order/argument/range spans are
compatibility output only and may be cleared without changing the judgment.

Current artifact: visual builds emit
`05_machine_contracts.json`, with authored contract identity and private
implementation evidence in separate nested objects. The contract object never
contains ranking subjects, view, range, or other witness material.

Boundary progress profiles referenced by premises are sealed semantic
commitments with grant/receipt identity. They participate in provider
admission but remain outside the initial ordinary proof-fact catalog.

Task consumption needs a derived artifact rather than syntax booleans. TR1
retired the former synchronous-spawn desugar and parser-erased `Join<T>`:

```text
TaskActivationPlan {
    machine_contract_id,
    entry_plan_id,
    argument_layout,
    terminal_outcome_layout,
    stack_plan,
    canonical_suspension_crossings,
    cpu_thread_preservation_obligations,
    cancellation_and_effect_contract,
}

TaskClaimState {
    provider_provenance,
    activation_identity,
    optional_storage_lease,
    lifecycle: Live | TerminallySettled,
}
```

The activation plan is deterministically derived from the normalized machine
contract and selected target/calling plan. Provider admission consumes it;
proof/debug artifacts retain it. `Task<T>` permission state carries claim and
lease provenance until settlement or transfer. Provider-specific handles and
physical frame locations are lowering details and must not be confused with
machine-contract or result-type identity.

Core owns the linear `Task<T>` claim and ordinary `TaskRuntime::start` /
`try_start` boundary surface. Each closed specialization receives a validated
`TaskActivationPlan` containing exact contract, entry, layout, suspension,
carry, stack, and preservation identities. Provider selection and single-use
invocation receipts bind that exact plan. Call-graph composition into fixed
stacks, dispatch, routed claim establishment, stack leases, and transactional
argument custody remain.

### Multiplicity and permission context

Replace `copy` as the whole usage model with:

```text
Multiplicity = Unrestricted | Affine | Linear
PermissionEntry = place + establishment + multiplicity + access + provenance
OwnershipEvent = Establish | Transfer | Consume | AffineDrop
```

`[copy]` maps to `Unrestricted`; ordinary data defaults to `Affine`;
`[linear]` maps to `Linear`. Zero establishment is derived from the default
domain and zero-reachable shape; the four-axis carry policy remains orthogonal
to multiplicity. Flow joins operate over permission entries with
path-sensitive sum state. Borrow events remain permission operations, not
linear obligations by fiat.

Current implementation carries normalized `Establish | Transfer | Consume |
AffineDrop` events, conditional payload debt, exact place/provenance identity,
and backend realization evidence. Every event maps to selected instructions or
a narrowly checked no-code reason; incomplete or foreign evidence publishes no
ledger. Legacy move/drop arenas are nonsemantic compatibility output.

Permission debt is path-indexed through transparent records, fixed arrays, and
active sum cases. Static projections preserve sibling debt and claim identity;
runtime-indexed extraction and correspondence-bearing symbolic mappings remain
fail-closed. N-to-m content transformations additionally retain their selected
algebra, geometry, backing/custody lineage, external supply, and conservation
witness. These are independent from carry policy and never inferred from
argument order or storage contents.

### Service reach, synchronous invocation, and operational ceilings

Represent service reach as symbol-resolved boundary-trait identities and keep
synchronous invocation, suspension, and blocking as independent contract axes:

```text
ServiceReachId  = normalized boundary-trait identity
ServiceReachRow = normalized set of ServiceReachId + parent closure

ServiceReachPlan {
  interface: InternalInferred | PublishedCeiling(ServiceReachRowId),
  checked_inferred: ServiceReachRowId,
}

SynchronousInvocationContract {
  interface: InternalInferred | PublishedCeiling(Set<BindingPath>),
  checked_direct: Set<SelectedBoundaryBinding>,
}

SuspensionPlan {
  interface: InternalInferred | PublishedMaySuspend(bool),
  checked_may_suspend: bool,
}

BlockingPlan {
  interface: InternalInferred | PublishedMayBlock(bool),
  checked_may_block: bool,
}

CrashCauseId = stable identity                     // closed compiler-owned
                                                   // vocabulary
CrashPredicateId = canonical lowered proof-expression identity

CrashRouteBucket {
  cause: CrashCauseId,
  alternative_guards: NonEmpty<CrashPredicateId>,
}

CrashRouteSet = canonical set of CrashRouteBucket

CrashPlan {
  interface: InternalInferred | PublishedCeiling(CrashRouteSetId),
  checked_inferred: CrashRouteSetId,
}

CheckedCrashSite {                              // checked-tree implementation evidence
  location: (StateId, state_local_statement_ordinal),
  cause: CrashCauseId,
  guard_covering_buckets: Set<CrashRouteBucketId>,
  known_local_frontier_lower_bound: Set<PermissionClaimIdentity>,
}

CrashContractCapsule {                         // abstract callable, no local body plan
  target: (RequirementOwnerId, StateSignatureId),
  target_contract_fingerprint: Fingerprint,
  published_buckets: CrashRouteSet,
}

CheckedCrashCallSite {
  location: (StateId, state_local_statement_ordinal, call_ordinal),
  target: (RequirementOwnerId | MachineId, StateSignatureId | StateId),
  target_contract_fingerprint: Fingerprint,
  exact_incoming_path_conjunction: List<CrashPredicateId>,
  derived_path_consequences: Set<CrashPredicateId>,
  surviving_buckets: CrashRouteSet,
}

CrashTerminatorPlan {
  cause: CrashCauseId,
  derived_guard: CrashPredicateId,
  covering_route_buckets: NonEmpty<CrashRouteBucketId>,
  known_local_frontier_lower_bound: FrontierId,
}

CallOperationalAcknowledgement {
  source_or_synthesized: Source | CompilerSynthesized,
  acknowledges_suspend: bool,
  acknowledges_block: bool,
}
```

Boundary-trait declarations mint service identities. `invokes` publishes the
boundary bindings the current invocation may enter before returning;
composition substitutes each binding path with its selected conformance and
retains the realized direct edges for cycle and stack-topology checks.
`suspends;`, `blocks;`, and `crashes` publish independent may-ceilings;
omission on a public requirement is the corresponding negative guarantee.
Private omission infers each axis. Crash routes normalize by cause, flattening
alternative guards within that bucket. A route-less source clause contributes
the canonical `true` guard. The deterministic normalizer
owns service-row and operational contract identity;
the entailment engine may gate reachability or legality but never rewrite a
published ceiling. `MachineTerminationPlan` remains independent and retains
the positive `terminates` guarantee and private ranking witness.

The crash checker covers every path-conditioned derived site with authored
same-cause routes. At a call, it substitutes arguments and removes only guards
disproved by available facts. Proofs may use a checked body only when that body
belongs to the same fingerprinted verification unit. Imported evidence cites
the published contract and certificate.

Same-unit private bodies use a conservative monotone summary over the viable
invocation graph. While typed expressions remain available, a temporary
canonical predicate tree retains positional parameter references and composes
the exact argument substitution through every nonrecursive private edge. The
tree is not durable checked data: final `CheckedCrashCallSite` rows contain only
the resulting source-independent predicate identities. An edge inside a
recursive strongly connected component widens each propagated route to its
unconditional cause bucket. That widening prevents
argument-changing recursion from generating an infinite predicate family and
is conservative for callers; acyclic wrappers retain their guards and concrete
outer arguments may still disprove them.

Checked lowering first records each explicit body crash as a
`CheckedCrashSite`. That row identifies the statement-handle-free state-local
site and its cause. Checked ownership then retains the stable identities of
every definitely-live, non-conditional linear claim at that exact site. The
set is deliberately a lower bound: a conditionally live sum payload enters
only after canonical symbol-rooted path evidence proves every active case on
its nested claim path. Non-place or dynamic-index argument rebinding and a
partial outer-case proof remain conservatively absent. Obligations outside the
activation are not claimed to be edge-enumerable. Exhaustive crash paths
abandon the retained claims; lowering does not synthesize a cleanup or consume
event for them. Open invariant windows contribute their invariant-bearing data
identities to the same lower bound.

The row is still not a completed `CrashTerminatorPlan` until guard coverage and
frontier reconstruction both succeed.
Canonical route buckets receive dense plan-local identities, and an
unconditional same-cause bucket enters `guard_covering_buckets` structurally
because `true` covers every path guard. Exact retained incoming guards and
their accumulated fallthrough negations join to identical canonical published
predicates. The conservative structural entailment layer also decomposes
positive conjunctions and negated disjunctions, including nested logical
negation, and normalizes Boolean equality/inequality against a literal to the
operand polarity it proves, without accepting their converses. It also records
operand-reversed comparison equivalents and flips equality/inequality under
negation. Ordered-comparison negation flips to the complement only for checked
integer operands; unknown, user-defined, and float operands remain opaque so
unordered values cannot make that rule unsound. Checked integer strict order
also entails its non-strict bound and inequality; equality entails both
non-strict bounds. Checked call
rows retain the same source-independent consequence set for caller-ceiling
coverage. The checked site and call rows separately retain the canonical
conjunction of exact incoming predicates; consequences
only establish bucket coverage and never replace that derived guard. Richer
logical entailment remains. Checked sites are implementation evidence and never enter the
published contract fingerprint.

`CrashTerminatorPlan` is a distinct no-successor terminal with no cleanup. Its
frontier field is explicitly a lower bound: caller frames, external effects,
and unrelated live or suspended activations need not be edge-enumerable. The
frontier is audit evidence and cannot license survivor execution. An explicit
abandonment plan, not an absent cleanup list, distinguishes this outcome from
incomplete lowering.
Terminal production maps retained stable claim identities to dense `ClaimId`s
and rejects a checked identity without a terminal mapping; it never silently
drops a known abandoned claim.

Fault tolerance is a separate composition proof. Continuation after a crash
requires an independently verified closed-custody component boundary, explicit
owner-death handling for every shared resource, reset or transaction semantics
for external effects, and a target realization of the isolation/restart plan.
Without that evidence the crash ends the execution domain. None of those facts
is inferred from `known_local_frontier_lower_bound`.

`CallOperationalAcknowledgement` belongs to syntax/checked-call and diagnostic
artifacts, not `MachineContractPlan` identity. Validation requires its two bits
to equal the statically known call envelope. The source order is fixed as
`suspend block`; a suspending call also carries the direct-position legality
check needed before continuation planning. Compiler-synthesized adapters record
the same facts without pretending a source token existed.

Current implementation gives service reach, suspension, and blocking separate
normalized plans and fixed points. Boundary traits mint canonical service
identities after symbol assignment; checked trees, provider plans, contracts,
snapshots, and manifests carry those identities directly. Public omission is a
negative guarantee, not “unknown,” and authored `suspends;` / `blocks;` clauses
remain independent of `reaches`.

Capability admission matches exact service symbols and normalized call
topology. Authority is never reconstructed from a service name. Grouped
machine/state/call rows are shared by the analyses, while their semantic
summaries remain distinct; no lowercase-name catalog, bitset projection, or
duplicate effect-row carrier remains.

Current implementation: syntax, resolved, typed, and
checked contract records retain the independent direct-invocation axis.
Bodyful inference follows local helpers and nested expression positions while
keeping boundary calls modular; bodyless requirements and published machines
enforce omission as an empty ceiling. Checked-provider composition removes the
selected plan's self-forwarded receiver, substitutes remaining binding paths
against selected boundary slots, and rejects cycles only in the realized
component-boundary graph. Provider schemas, plan and machine-contract
fingerprints, snapshots, and JSON manifests retain normalized direct targets
without replacing them with service-reach closure. Declared invocation targets
also contribute their boundary service to normalized reach and their boundary
contract to call operational inference. Registration alone creates no edge.

Authority possession, provider trust receipts, resource bounds, failure
outcomes, and mutation remain separate fields/analyses. Do not manufacture a
single all-purpose effect record or reconstruct suspension/blocking from
service reach.

Checked facts
name the grouped suspension/blocking/call-topology plan `operational`. The
former `operations` field was an ambiguous internal umbrella name; this rename
does not combine service reach, trust, mutation, termination, or any other
independent semantic axis with the operational fixed point.

## Staged migration

1. **Inventory and invariants.** Add compile-time tests/snapshots showing where
   domain theory, supply mode, multiplicity, carry policy, and contract identity
   must survive.
2. **Core semantic enums/IDs.** Land domain-theory records, establishment-route
   identity, semantic-role vocabulary,
   multiplicity, carry policy, supply mode, termination guarantee/witness,
  progress-profile ID, service-reach ID/row, suspension plan, blocking plan,
  and other identity handles in the lowest dependency-safe crates. No
   behavior change.
3. **Tree propagation.** Carry the representations through symbol-resolved and
   typed trees, snapshots, cloning/substitution, and diagnostics. Eliminate
   re-derivation from body shape/keyword presence.
   Structured canonical const values and the closed indexed-domain family
   representation, explicit indexed qualification, and constrained-position
   const-machine specialization are landed. The first units package and its
   imported conversion/operator canaries are also landed. Open computed index
   expressions, exact algebra authority, and retained closed/normalization/
   local-fact equality evidence complete PDI3.
4. **Checked plans.** Split predicate facts, static semantic roles, and
  establishment evidence; add
  the place-keyed permission plan, service-reach plan, suspension plan,
  blocking plan, termination plan, and
   normalized machine contracts.
5. **Validation and resolution.** Enforce predicate proof, owner-authorized
   establishment, core qualification conformance, role-keyed operator
   selection, multiplicity conservation, carry derivation/local
  transition legality/runtime refinement, service-row inclusion/propagation,
  suspension/blocking propagation, and
   supply/admission rules.
6. **Lowering boundary.** Lower only from checked selections/plans. Preserve
   semantic contract IDs in proof/component/debug artifacts while erasing
   proof-only material from executable operations.
7. **Retire compatibility paths.** Remove compiler-special arithmetic-policy
   routing and boolean/context re-derivation only after their general
   equivalents have differential coverage.

## Ordering constraints

- Domain qualification/operator-family work must not grow on the undifferentiated
  `DomainDefinition` shape.
- Linear `Task<T>`, transactions, or dependent-linear buffers must not grow on
  move/drop-only ownership summaries.
- Component requirement bindings and hot-swap manifests must not pin a body hash or
  flat service row in place of normalized machine contract identity.
- Ranking subjects, views, ranges, SCC mapping, and certificates must not enter
  published machine-contract identity.
- Service-row and operational-ceiling identity must not depend on prover
  strength, provider selection, or a legacy numeric bit.
- External requirement bindings pin authored normalized service and operational ceilings;
  provider admission compares each axis independently and never consults a
  global import scan.

## Acceptance criteria

- No checked-stage query infers predicate bodies, semantic roles, or
  establishment permission from punctuation, body emptiness, or operator
  presence.
- A domain carrying both predicate requirements and semantic roles is representable
  without duplication.
- A closed indexed domain fingerprints one canonical value and preserves the
  carrier ABI. An open index fingerprints its exact algebra instance,
  normalized public operation contract, and canonical expression; normalizer
  implementation version remains provenance metadata, and a compatibility
  proof cannot silently change identity.
- Static qualification survives generics and containers while proven
  predicates remain flow facts.
- A qualified `as` coercion preserves denotation, an explicitly bare target
  erases only non-owning semantic meaning, arbitrary user code is never
  invoked, and every proof used for bounds, divisibility, and domain
  establishment survives until lowering.
- Requirement/provider/accepted/checked supply modes survive into artifacts.
- A requirement's termination guarantee can be inherited while its checked
  implementation carries a private ranking witness.
- Replacing one valid ranking witness with another revalidates only the
  provider/proof artifact and leaves caller/requirement contract identity
  unchanged.
- Runtime lowering may reject a non-tail ranked cycle while proof-time
  evaluation consumes the same checked machine and witness.
- A type has one explicit multiplicity; zero establishment remains a derived,
  independent judgment.
- An established linear obligation is traceable through create, transfer, and
  consume events across branches.
- The executable backend receives already-resolved operators and ownership
  plans; it does not repeat semantic resolution.
- Semantic interface identity and physical ABI identity are distinct and both
  queryable.
- Service reach, possible suspension, possible blocking, guarded crash routes,
  and positive termination remain independent after parsing, normalization,
  inference, diagnostics, and artifact emission.
- An exported authored service/operational contract is stable when prover
  strength changes; internal omissions reach deterministic least fixed points
  in their checked call component.
- A blocking provider cannot satisfy a slot that permits suspension but omits
  `blocks`.
- No semantic decision projects suspension or blocking from service reach, or
  service reach from an operational boolean.
- Crash buckets normalize by exact cause. Call-site refinement may remove a
  bucket but cannot rewrite published contract identity.
- Every verified crash site has an explicit no-cleanup terminator and frontier
  lower bound; absence of cleanup is never used as abandonment evidence.
- The crash frontier is a necessary lower bound for audit and diagnostics, not
  sufficient evidence for survivor safety.

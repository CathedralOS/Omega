# Semantic Taxonomy Representation Rework

The domain-facet, machine-taxonomy, core-multiplicity, service/operational, and
termination/ranking settlements are not documentation-only classifications. They make distinctions that affect
resolution, interface identity, admissibility, ownership, diagnostics, and
lowering. If the compiler stores only the old undifferentiated shapes and
reconstructs the distinctions later, it loses a clean representation and will
eventually encode policy in scattered conditionals.

This record defines the durable representation target and the remaining gaps.
The compiler represents reach and the termination guarantee/ranking-witness
firewall directly rather than extending undifferentiated booleans and bitsets.

## Representation gaps

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

The symbol-resolved and typed `Machine` records carry one normalized
`MachineSupplyMode`, a public termination guarantee, a private
`RankingWitness`, independent service/suspension/blocking/crash surfaces,
contracts, and states. Source syntax alone retains the `boundary` spelling;
semantic consumers do not reconstruct supply from that spelling or body
presence. The remaining gap is their convergence into one normalized semantic
contract with consumption eligibility, observation surface/floor, progress
contract, and boundary-facing contract identity.

The guarantee/witness split now represents an inherited guarantee with an
implementation-local witness and lets a witness change leave interface
identity untouched. Checked summaries derive local completion independently;
the cycle gate covers state SCCs, same-shaped runtime machine SCCs, and
structural non-tail proof-only SCCs. The remaining gap is the full conditional
guarantee with pinned progress premises and its boundary-facing contract
identity.

### Multiplicity and permissions

`Multiplicity` is first-class from source properties through typed trees.
Checked flow records `Establish`, `Transfer`, `Consume`, and `AffineDrop` with
explicit access, stable establishment provenance, and conditional-payload debt.
Borrow activations and weakenings feed the same permission context. Linearity
consumes these qualified events; move/drop summaries cannot invent semantic
provenance. Parallel move/drop summaries are deleted. The remaining cleanup
gap is an explicit per-edge plan with ordered actions, contextual contract
checks, cycle composition, and a retained conservation witness.

### Service reach and operational behavior

Symbol-resolved service reach remains separate from recursive
suspension/blocking inference. Both plans share the grouped
machine/state/call topology, but service rows are interned canonical trait
identities while operational summaries are independent booleans. Authority,
trust, resources, failure, and mutation remain in their separate semantic
homes. Typed machines and state signatures carry only their normalized service
row. Authored service names end during syntax-to-resolved normalization: a
lowering-private sidecar retains them through symbol assignment, row
construction, and source-facing diagnostics, then discards them. Published
symbol-resolved and typed trees contain no parallel spelling contract.

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
equality fails closed without ambient theorem search. A quotient may retain any
representative for runtime storage, but it is ineligible as a canonical index
atom until a proved canonicalization supplies one unique encoding.
Default-domain-constrained index values likewise remain blocked on their
required facts and canonical proof admission.

The first post-ruling quotient-operation carrier is likewise non-authoritative.
Typed calls retain an exact representative entry, exact named conformance
application, and `lift`/`define` kind only for the sealed `Quotient` spelling.
They do not become checked or terminal operations until quotient formation,
compiler-derived relations, correspondence, and contracts are independently
validated. Bare representative calls cannot recover authority through
structural proof-machine discovery. Quotient formation remains a separate
legacy gap: its current boolean-relation pilot structurally scans law-machine
contracts and must be replaced by the declaration's exact named `Equivalence`
selection before the retained operation carrier can be admitted.

Current domain representation keeps predicate bodies, semantic roles,
establishment routes, aliases, and flow evidence independent. Operator homes
come from an exact domain-qualified name or one unique declared-domain
constraint across the operand tuple; operators do not grant establishment.
Conjunction combines roles by axis and rejects competing contributors to the
same role.

Domain propositions normalize from `requires`; exact requirement identities in
`established by` are the only authored establishment routes. Empty atomic domains may
be qualified vacuously, while predicate-bearing domains require proof and
routed domains require an exact authorized exit. No package ownership,
attachment name, machine placement, or former privileged qualification trait
confers establishment authority.

Terminal Psi retains a routed, content-bearing boundary parameter first as a
canonical producer schema: exact requirement and source position, qualified
carrier, normalized domain identity, owner-unique projection and algebra, and
normalized per-occurrence capacity. The schema excludes module-local dense IDs
from its identity and remains non-authoritative. Installation is a separate
semantic join that binds an exact slot occurrence, finite cardinality,
artifact instance, and lifecycle epoch; only that joined occurrence may enter
program-local root lineage.

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
    visibility: Private | Public,
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

Only the complete name-first item creates this selectable identity. An exact
machine satisfaction edge may carry an `as Name` grouping label for
requirement-local proof or provider mechanisms, but that label is not a
`NamedTraitConformance`, cannot satisfy a whole-trait bound, and owns no
standalone visibility. A dynamic value may carry one selected conformance
without making its name visible to the receiver.

An exact `machine ... satisfies Trait::requirement` realization is a different
semantic edge. It can supply a provider slot, operator, establishment route, or
proof citation without claiming the inherited trait closure. It therefore
cannot satisfy a whole-trait bound or license `dyn`. Dynamic descriptors,
carrierless selected evidence, and law-bearing consumers use only the complete
normalized conformance map above. Backend deduplication may share physical code
between rows without merging their semantic identities.

### Machine semantic contract

Converge the complete substitutable contract into a normalized
`MachineSemanticContract` (name provisional). Its `MachineSupplyMode` is
already first-class from symbol-resolved trees onward. Syntax trees may retain
source spelling, but provider admission, proof artifacts, component manifests,
compile-time evaluation, task-activation checking, and lowering consume the
normalized mode rather than re-derive it from `boundary`, bodies, or effect
names.

Consumption eligibility should normally be derived views/queries over the
contract, not stored independent booleans that can drift.

The supply representation preserves the five settled variants directly:

```text
MachineSupplyMode =
    CheckedBody
  | Requirement
  | Boundary
  | Accepted
  | ExternalRealization { binding: NormalizedBindingId }
```

`Boundary` is an externally supplied host/component declaration. `Accepted` is
the axiom-tier form whose trust remains explicit. Neither is interchangeable
with a checked body or a requirement.

`ExternalRealization` is sourced by `satisfies ... via <Binding>`. The binding
expression is compile-time evaluated and normalized before checked-plan
construction. It is not an executable body, and it does not author a trust
class or a second reach row. The satisfied requirement supplies the public
contract/ceiling; validation and admission check the binding/provider behavior
as a refinement and produce any trust receipt. `ProviderPlan` is then derived
from explicit conformance closure rather than authored rows.

`NormalizedBindingId` interns the complete evaluated structured binding, never
a rendered lookup string. The producing build-time-machine closure, enclosing
realization-machine symbol, normalized signature, and target application remain
explicit identity inputs. A payload-free `CompilerIntrinsic` uses the
realization symbol plus target to select a sealed catalog lowering. A DLL
locator is instead one typed object-format variant containing all of its raw
coordinates as fixed byte arrays and scalars. The satisfied requirement's calling
policy supplies the independently evaluated `CallPlan`; it is not duplicated in
the binding. Raw linker bytes are target-package data, enter the binding
fingerprint directly, and never become Omega names or provider keys.

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
    physical_arrival_requirement,
    semantic_arrival_requirement,
    bootstrap_adapter,
    physical_result_map,
    visible_parameters,
    semantic_result,
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

The Rust on-ramp exposes that completeness surface as a closed
`TargetRequiredRootSlotDeclaration` schema catalog behind an exact-size
iterator; its public type does not encode the current catalog cardinality.
Build selection validates named rows against the owning profile and requires
the selected profile's complete catalog. Installation derives the expected
root-slot closure from the same declarations and rejects duplicate declarations
or compact-identity collisions before comparing selections. `ProgramEntry` is
the only current member. A future schema variant therefore forces exhaustive
consumers either to implement it or reject it; it cannot become an ignored
open slot merely because the catalog grew.

An entry schema fixes one physical arrival requirement and separately selects
one semantic arrival requirement for its build-bound continuation. It
contributes physical calling/state policy, an exact target bootstrap adapter,
native-result mapping, and the source-visible continuation shape. The generated
ABI shell and authored bootstrap retain both arrival identities. Together they
install lifecycle-scoped platform providers, establish the semantic occurrence,
provision any exclusive receiver beneath admitted storage, and receive a
compiler-derived `MachineSemanticContract` whose crash, reach, write, work,
stack/state, provisioning, introduction, and provenance rows compose with the
bound source-entry closure. The selected source machine is neither the physical
entry nor a source of hidden platform parameters.

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

Source `boundary` remains insufficient to reconstruct this enum: boundary
supply and an accepted bodyless declaration both mention the word but have
different trust. Likewise, body absence distinguishes a requirement only in
its declaration context, while `via` names an external realization. Populate
the enum once during semantic lowering and carry it thereafter.

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

Termination legality, checked facts, specialization, eligibility, and snapshots
all resolve the normalized `RankingWitness`. Visual artifacts keep authored
contract identity separate from private implementation evidence; ranking
subjects, views, ranges, and other witness material never enter the contract
object.

Boundary progress profiles referenced by premises are atomic domains explicitly
classified by their owner with `satisfies ProgressProfile`. Their
`established by` requirements and exact admitted receipts seal the commitment;
predicate absence or provider use never infers the classification. Published
contracts retain authored premise schemas, checked call edges retain exact
substituted instances, and coverage resolves each instance to a public schema,
local receipt, or manifest-bound provider receipt. They participate in provider
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
ledger. Checked trees, state graphs, and control flow carry no parallel
move/drop arenas.

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

Same-unit private bodies use a conservative monotone summary. Nonrecursive
edges substitute positional arguments exactly; recursive SCC edges widen to an
unconditional cause bucket so the summary lattice remains finite. Durable
checked-call rows retain only source-independent exact guards, sound derived
consequences, surviving buckets, and the pinned target contract identity.

Each explicit body crash becomes a statement-handle-free checked site. Its
frontier contains only claims definitely live at that point; conditional claims
enter only when canonical case evidence closes their path. Guard coverage may
use an exact published predicate, an unconditional same-cause bucket, or a
sound structural consequence, but a consequence never replaces the exact path
guard. These site/call rows are implementation evidence and do not enter the
published contract fingerprint. The terminal-Psi page owns detailed predicate
rules and canonical lowering mechanics.

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

Service reach, direct invocation, suspension, blocking, crash routes, trust,
mutation, resources, and termination remain separate normalized axes. The
analyses may share grouped machine/state/call topology, but capability admission
matches exact service symbols and never reconstructs authority from a service
name. Public omission is a negative guarantee; private omission reaches a
deterministic fixed point. Registration alone creates no invocation edge.

## Migration discipline

`TASKS.md` owns the remaining sequence. Each slice introduces semantic records
in the lowest neutral owner, propagates them without re-derivation, validates
them into checked plans, lowers only from those plans, and deletes the displaced
special case after differential coverage exists. Implementation checkpoints
belong in Git history, not in this taxonomy.

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

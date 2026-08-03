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

`omega-symbol-resolved-trees/src/domain.rs` and
`omega-typed-trees/src/domain.rs` represent every domain as one
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

Implementation status (PDI1–PDI3 complete, 2026-08-01): the pre-resolution
generic pass canonicalizes eligible structured literal constants recursively
and replaces their source reference with a reserved length-delimited value atom. The atom's
type plus canonical structural encoding is generic/monomorphization identity;
its canonical display is diagnostic data. Declaration-aware validation accepts
the atom only at a matching `const` parameter, including generic shapes the
record monomorphizer intentionally leaves structural. Field order normalizes to
declaration order. Current structural `Rat` values additionally reject a zero
denominator, uncancelled signed coordinates, or a non-unit gcd. Closed domain
families now carry typed telescopes, canonical instance arguments, and
per-instance semantic identities through typed snapshots and copying while
preserving carrier ABI. Explicit qualification now selects a closed value or
direct const binder and publishes that exact instance through checked evidence.
Const-generic machine specialization infers canonical values from constrained
parameter and result positions, refreshes exact identities after substitution,
and runs distinct tuples in both engines. The shipped units module now closes
PDI2 with named combinations, visible conversion policy, cross-module
operators, and an imported cross-index rejection rail. Computed open indices
now retain exact operation/algebra authority, canonical expressions, named
compatibility conditions, and closed/normalization/local-fact discharge
evidence. Exact active facts may prove compatibility without changing semantic
identity; unresolved equality rejects without ambient theorem search.
Quotient and default-domain-constrained value kinds fail closed until their
canonical-representative/proof admission path exists.

Implementation status (DOM1/STR2 semantic roles, 2026-07-31): core,
symbol-resolved, and typed layers carry `DomainPredicateBody` and the closed
`DomainSemanticRoles` record independently. An ordinary top-level operator's
exact `Type::Domain::operation` name, or one unique declared-domain constraint
across its operand tuple, supplies its domain operator home before symbol
assignment. Nested operator declarations reject, and the association does not
grant establishment authority. Downstream propagation and resolved/typed
snapshots copy and publish the explicit role record. Qualification and trust
publication consume the declaration's stable semantic identity, qualification
consults predicate-body presence for proof, and operator selection consumes
semantic roles. Domain conjunction validation permits contributions in
different roles (`Degrees & Wrapping`) and rejects multiple distinct
contributors to one role. Exact coercion consumes normalized semantic roles
and proof obligations rather than a privileged qualification trait or name
convention.

Target representation (DOM1 establishment surface, 2026-07-30): domain
propositions normalize from `requires`; exact requirement identities in the
body normalize as alternative establishment routes. An empty declaration has
no obligations and permits explicit qualification from its bare carrier.
Syntax, symbol-resolved, typed, and checked trees must preserve predicates and
routes independently rather than reconstructing either from body presence.

Implementation status (P1a evidence origin, 2026-07-28): checked semantic facts
now carry an establishment-evidence axis independent of their program-point
origin. The normalized origin classes distinguish prover, checked validation,
authorized-route establishment, checked transformation, admitted receipt,
propagation, and vacuous qualification. No package receives ambient
establishment privilege. Call-result binding and ordinary statement transfer
preserve the evidence.
Granted selected provider plans attach their normalized plan fingerprint to
matching admitted facts, and checked artifacts publish
`05_qualification_evidence.json` with origin, source, program point, and receipt
identity. Exact owner-authorized admitted-subject matching is now live.
Selected service methods additionally retain structured linear result claims.
Those claims enter provider identity and the same artifact as `returns` rows;
the external-root ledger can therefore bind a mask-transition receipt to the
exact `Active` guard subject without parsing the normalized result type.
Exact `as` uses retain the normalized domain and derivation before lowering.

Implementation status (P1b vacuous qualification, 2026-07-30): an explicit
`as` into an empty atomic domain is compiler-derived identity work and requires
no user-authored satisfier. Transparent aliases qualify this way only when
every expanded atom is empty. The former core
`RepresentationQualification` trait and its privileged trait/conformance
roles, satisfier selection, erased call lowering, and canonical-use artifact
are gone. Checked artifacts retain the exact cast site and
`vacuous_qualification` origin; predicate-bearing and routed atoms do not enter
through this path.

Implementation status (P1b authored route surface, 2026-07-31): syntax trees
now retain predicate `requires` separately from exact requirement paths in the
domain body. Resolved trees preserve the authored paths and normalize them
once, after symbol assignment, to checked- or boundary-requirement identities;
unknown, ambiguous, and exact-result-mismatched routes reject. Checked
conformance exits consult those identities, prove every predicate on a mixed
domain, and publish `authorized_route_establishment` rather than ambient owner
evidence. `Extent::Granted` uses the authored route surface. Predicate-in-body
syntax is now rejected with directed `requires` migration guidance, and the
source, sample, canary, and embedded-test corpora use the independent predicate
record. Domain operators have moved to ordinary top-level declarations with an
exact or uniquely inferred semantic home; nested declarations reject, and
operators no longer create establishment routes. Owner machines and boundary
contract placement have no ambient establishment privilege: the normalizer
retains only exact checked- or boundary-requirement identities authored by each
domain.

Implementation status (P1a establishment routes, 2026-07-28):
`DomainEstablishmentRoute` records the exact trait-requirement identity
authorized by a domain body. Syntax-to-resolved lowering normalizes those
relationships once after symbol assignment, recursively expands aliases to
their atomic domain facts, and deduplicates without losing declaration order.
Resolved and typed domain definitions, typed binding-site constraints, and
structural snapshots preserve the records. Checked qualification consumers
consult only the normalized route identity instead of reconstructing
permission from attachment names, package ownership, or contract placement.

Implementation status (DOM alias expansion, 2026-07-28): transparent
declared-domain aliases retain independent syntax, resolved, and typed records.
Their constituent symbols resolve after the complete declaration set exists;
uses expand recursively to atomic facts before constrained-type and contract
identity, compatibility, admission, executable predicate lowering, and
establishment-route normalization.
Validation rejects empty, unknown, cross-carrier, cyclic, and public-to-private
expansions, while call diagnostics name the unmet atom. Compiler-owned `Carry`
atoms and `Carry::Portable` remain part of the separate per-claim carry
migration.

Implementation status (DOM1 generic propagation, 2026-07-23): typed
`TypeConstraintNode::Domain` is a normalized binding-site record, not a bare
name. A post-lowering pass resolves the short name only against declarations
whose target matches the constraint's carrier, then stores the declaration
symbol, semantic identity, predicate-body record, and semantic roles. Nested generic arguments and all
type-table copy paths preserve the record. Validation checks the record against
the carrier declaration; checked field/contract facts and byte predicates use
the stored symbol directly instead of repeating a global short-name lookup.
Typed snapshots publish the full record. Generic substitution is therefore no
longer a domain-theory loss boundary.

Implementation status (DOM1 per-axis composition, 2026-07-28): a constrained
type's domain chain is no longer projected to its first member. Predicate
theories compose conjunctively through implicit parameter requirements, checked
writes and constructions, entry/read facts, return/parameter implication, and
post-write re-establishment. Members without predicate bodies never enter that
fact lattice; their normalized identities and role contributions remain on the
type for qualification and operator consumers. Semantic roles compose by key,
with same-role collisions rejected. Establishment routes copy independently
onto every normalized domain constraint and do not enter the predicate-fact
lattice.

Implementation status (DOM2 binding activation, 2026-07-23): checked operator
selection reads only static binding sources: normalized declared constraints,
explicit qualifications, and signature `requires`. The selector has no flow/fact-plan
input, so guards, call guarantees, or later prover improvements cannot change
operator meaning. Operator `requires` clauses remain ordinary flow-sensitive
proof obligations after selection. Candidate matching now consumes the complete
operand tuple for binary, index, and range spellings, sharing one generic
substitution across every known position; return types remain irrelevant.
The old declaration-global same-carrier collision fence is gone: inactive
domain theories coexist, while the checked selector admits only meanings owned
by semantic domains selected on participating operand bindings and rejects
multiple admitted meanings at that use. This realizes closed-family coherence
without permitting unrelated imports to inject an eligible meaning. The
language currently has no authored open-family/dispatch-owner-position surface;
decision 19 defers general open-family linking.

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

Implementation status (2026-07-24): `CarryPolicy` and its four closed axes live
in the dependency-safe semantic vocabulary and are copied through syntax,
resolved, typed, and syntax snapshots. Checked trees own a `CarryFacts` plan
that keeps the authored minimum separate from the effective derived policy.
The parser requires a complete `[carry(...)]` product and rejects retired
`[send]` with guidance. Transparent data and generic bounds use independent
per-axis composition/comparison; concrete generic instantiations substitute
their actual arguments through nested transparent wrappers. Canonical
place-liveness rejects forbidden values across direct or transitive possible
suspension, including persistent fields through reachable state transitions,
arguments carried by the call itself, and later operands under left-to-right
evaluation. Accepted-claim admission, per-claim qualification,
contained-machine runtime admission, and richer artifact/model export remain.
The
parser-unreachable resolved/typed contained-machine span has been retired
end-to-end. Checked `CarryFacts` now derives contained topology exactly once
from authored attached-data fields whose data type has one or more attached
machines, storing machine roots, fields, and targets in grouped arenas/spans.
State-graph metadata and backend reports consume that fact. Canonical semantic
suspension crossings join across the cycle-safe descendant closure. A target
that may migrate execution outside those crossings must establish
activation-wide preservation for any possible CPU/thread-restricted value; it
does not select an alternate all-instruction runtime-supply envelope. There is
no separate `contains` source form or compatibility carrier.

Implementation checkpoint (2026-07-28): `CarryPermission` now supplies the
closed compiler vocabulary, including parser expansion of `Carry::Portable`
and transparent user aliases over the atoms. Boundary call guarantees admit
only an exact owner-authorized result permission and retain its requirement and
provider receipt; a direct call to the checked adapter does not grant it.
Admitted linear routed resource facts additionally seed an independent
born-strict `CarryOrigin`. Local transfers and one-to-one state-parameter
handoffs preserve that origin and its exact per-axis relaxations even when the
qualification fact is later absent. Call exits infer the same carry mapping for
one scalar linear input and one scalar linear output across checked,
generic-slot, and admitted targets; conditional aggregates deliberately wait
for P1c path-indexed mappings. Chained checked helpers therefore preserve the
claim entry without republishing its authority domain or carry facts.
Canonical suspension liveness records and checks each live value's effective
claim policy, while activation-wide carry envelopes conservatively join
established claim origins and permissions without publishing a provider
preemption mode. Qualification and carry artifacts expose the admitted atom and
the effective crossing policy. Remaining work is
path-indexed aggregate and partial-move propagation plus conserved multi-output
mappings under P1c. The task source/artifact canary pins an admitted
suspension-only permission through qualified selected-machine specialization
and canonical safe-point liveness. The activation artifact now carries a
fixed-stack `StackPlan`, canonical suspension crossings, and demanded
CPU/thread preservation; the retired safe-point/all-instruction
`MigrationDemand` compatibility fields are gone. Fixed nonmoving stack storage
supplies continuation address stability structurally; no provider preemption
mode selects an alternate all-instruction supply envelope.

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

Implementation status (TPR3, 2026-07-17): termination legality and checked
view facts resolve ranked subjects and argumented-view bounds from the
normalized `RankingWitness`; view and rank-range identity come from the same
witness. The legacy typed-machine decreases/order/argument/range spans are
compatibility output only and may be cleared without changing the judgment.

Artifact status (2026-07-17): visual builds emit
`05_machine_contracts.json`, with authored contract identity and private
implementation evidence in separate nested objects. The contract object never
contains ranking subjects, view, range, or other witness material.

Boundary progress profiles referenced by premises are sealed semantic
commitments with grant/receipt identity. They participate in provider
admission but remain outside the ordinary proof-fact catalog in v1.

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

Implementation status (TR2/TR4, 2026-08-01): core owns the source-visible
`[linear] Task<T>` claim carrier plus `TaskOutcome<T>`,
`StartOutcome<T, Arguments>`, and the generic `TaskRuntime::start` /
`try_start` ordinary boundary-trait surface. Symbol-keyed generic substitution preserves
conditional payload debt, with pass and scope-loss canaries covering returned
linear results and rejected linear argument bundles.

Concrete static-machine specializations retain their executable instance
symbol. The compiler derives a validated `TaskActivationPlan` for each closed
TaskRuntime start specialization and emits `05_task_activations.json`. The plan
uses checked contract/entry/layout/calling identities, the normalized
transitive suspension plan, canonical crossing liveness/carry facts, and
concrete target layout. The artifact now carries `StackPlan`, canonical
suspension-crossing identities, and demanded CPU/thread preservation; the
retired continuation-size, preemption-mode, and all-instruction runtime-supply
fields are gone. `StackPlan.bytes` is currently the local machine/park-frontier
layout bridge; whole-call-graph WCSU composition remains part of fixed-stack
lowering. Every activation requires the cancellation operation because
cancellation-request authority is part of every `Task<T>` claim. Provider
plans now bind each concrete activation to the exact selected `TaskRuntime`
plan and exact `start`/`try_start` requirement; missing selection and provider
machine-contract narrowing reject. Dynamic provider-instance/invocation
receipts, dispatch, claim provenance, stack leases, and child accounting remain
later task-runtime rungs.

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

Implementation status (CML4 migration, through 2026-07-28): checked flow retains
normalized `Establish | Transfer | Consume | AffineDrop` events, including
whether a conditional sum event carries live payload debt. CML3's second slice
propagates the same typed events through state graph, control flow,
abstract/target/assigned operations, machine instructions/program/bytes, and
the backend report. The older move/drop arenas remain compatibility output only
through control flow and are dropped at the abstract-operation boundary; no
backend representation carries them, and no semantic producer or consumer may
reconstruct permission kind from that lossy pair.

CML4's backend-realization slice preserves the control-flow arena identity on
each abstract event and normalizes selection-time candidates into exactly one
realization row per event. A row is either a sorted unique set of selected
instruction indices or a narrow checked no-code reason. The latter is admitted
only for explicit zero-code terminal consumes, no-live-debt events, and trivial
affine discard; an empty selection site alone cannot prove a live establishment
or transfer. Folded storage
materializations may realize several transfers in one provenance chain; this
does not mint a new origin. Candidate validation is all-or-nothing: missing,
foreign, out-of-plan, or invalid no-code evidence publishes no ledger, and the
backend report marks every event `UNLINKED`. Runtime/direct operation sites and
dispatch-edge and state-call handoffs into target-state entry establishments
cover the complete current ownership pass corpus. State/host call sites retain
exact call ordinals. Named transition targets reserve their canonical ordinal
before nested argument calls, while edge joins use target symbol as well as
statement and ordinal; a nested two-obligation transition now retains a complete
ten-event ledger. A runtime canary also carries a live linear obligation across
a dispatched call's synthesized continuation and consumes it afterward. The
continuation does not mint a permission event: it preserves the caller's
canonical place and provenance; the later consuming call remains the eventful
boundary. Two same-symbol nested calls in one transition retain distinct
ordinals and jointly realize the target state's shared canonical event.
Program-entry establishments are joined to the normalized platform argument
writes before either straight-line or dispatched selection begins. A later
consume cannot retroactively realize StateEntry, and a missing inbound write
leaves the ledger incomplete rather than treating zero storage as
establishment. Linear obligations returned from direct state-local paths or
record-constructor fields are joined to their caller receiving paths without
minting caller-local identities or origins. Checked states now publish complete
normalized output maps, and opaque n-ary calls consume those maps across
expression calls and qualified tail transitions without treating argument
order as authority evidence. Ambiguous routed targets still reject, while the
checked artifact retains every output path and input-relative or established
source. Nontrivial state-exit code actions now target the settled
`EdgeCleanupPlan`: materialize outgoing values, commit the transfer map, clean
the ordered dying affine places, and retain the exact conservation witness.
Composite per-field debt uses the settled path-indexed frontier: explicit
nominal linearity contributes one root, transparent aggregates contribute their
contained child claims. The first implemented slice follows statically named
transparent-record fields through local construction, whole-record transfer,
and extraction; moving one field preserves sibling debt, duplicate moves
reject, and backend permission realizations retain the field paths, independent
source provenance, and transfer-stable claim identity. Carry policy now indexes
that exact identity as a separate checked axis, so n-ary outcome maps preserve
each child policy and suspension checks intersect the policies of every live
claim below an aggregate place. The carry artifact retains each effective
claim policy and its contributing-origin count. Literal-length fixed arrays now
enumerate structured fixed-index paths through construction, literal-index
extraction, partial moves, and n-ary output maps; runtime-indexed extraction
remains fail-closed. Active sums likewise enumerate structured case-plus-field
paths. Payload-field symbols are children of their variants; known construction
activates only the selected case, same-case siblings remain independent, and
checked output maps omit proven-inactive alternatives while propagating live
case identities through opaque calls. Symbol-keyed substitutions already retain
contained claims through nested generic transparent records.
Content-bearing n-to-m transformations additionally retain the selected
compiler-owned algebra, normalized claim projection, per-invocation geometry,
admitted external supply, stable backing identity, fresh-issuance premise,
custody/alias lineage, root-lineage mapping, and exact separated-conservation
witness. Provider succession appends classified predecessor/successor custody
edges rather than rewriting claim origins. The initial
closed vocabulary contains canonical disjoint interval sets and counted
quantities over proof-level natural arithmetic. Entry/current versions belong
to structural-place terms; separated composition, derived residuals, sealed
introduction, and custody-exit rows remain distinct proposition facts. A
qualification with no owner-unique `Content<A>`
conformance participates only in whole-claim frontier accounting.
Correspondence-bearing symbolic mappings and runtime-indexed extraction remain
fail-closed extensions.

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
`suspends;` and `blocks;` publish independent may-ceilings; omission on a
public requirement is the corresponding negative guarantee. Private omission
infers each axis. The
deterministic normalizer owns service-row and operational contract identity;
the entailment engine may gate reachability or legality but never rewrite a
published ceiling. `MachineTerminationPlan` remains independent and retains
the positive `terminates` guarantee and private ranking witness.

`CallOperationalAcknowledgement` belongs to syntax/checked-call and diagnostic
artifacts, not `MachineContractPlan` identity. Validation requires its two bits
to equal the statically known call envelope. The source order is fixed as
`suspend block`; a suspending call also carries the direct-position legality
check needed before continuation planning. Compiler-synthesized adapters record
the same facts without pretending a source token existed.

Implementation status (EFX symbol-resolved service plans, 2026-07-23): `omega-core` now owns
distinct `ServiceReachId`/`ServiceReachRowId` identities, deterministic
service-row set normalization, and independent service, suspension, and
blocking plans. The operational interfaces distinguish private inference from
published `false`, preserving omission as a negative public guarantee instead
of treating it as “not computed.” Authored `suspends;` / `blocks;` clauses now
parse independently, survive syntax/resolved/typed trees and snapshots, enter
checked `MachineContractPlan` values and fingerprints, and drive task
admission. Operational names are rejected in source `reaches` rows, normalized
service rows filter every operational member, and the migrated task/carry
fixtures use the split spelling. Boundary traits now mint canonical identities
after symbol assignment; machine, requirement, and nested machine-parameter
rows resolve through the symbol table and include boundary-parent closure. A
separate recursive service fixed point drives checked ceilings, static-machine
and checked-provider admission, provider schemas, contract fingerprints,
snapshots, and manifests. Ordinary policy traits never mint service identity.
Executable capability manifests now read the checked service, suspension, and
blocking plans directly: they publish canonical service names and independent
`may_suspend` / `may_block` values without a lowercase-name or numeric-bit
projection. Boundary-provider approval is exact to the reached capability
symbol, capability acquisition follows normalized call topology, primitive
provider authority is categorical metadata, and reports never reconstruct
authority from service names. Static-machine refinement compares normalized
service rows directly. Checked trees now expose grouped `ServiceReachFacts` as
a first-class root; their duplicate `EffectRowFacts` carrier and the legacy
reach-row field/input in machine contract artifacts and fingerprints are
deleted. The obsolete `EffectRowId`/`EffectRowTable` carrier is also gone from
core, resolved trees, and typed trees; those stages retain only
symbol-resolved `ServiceReachRowId` values. General validation consumes
canonical service rows plus the operational plan directly.
Normalized inference now retains machine/state/call structure as shared-row
identities in grouped arenas. Checked-flow, state-graph, and control-flow
records carry those identities alongside independent suspension/blocking
summaries, and the persistent graph crates no longer depend on `omega-effects`.
The typed-tree report joins those same normalized scopes with the independent
operational fixed points. All semantic phase filters derive their sorted
canonical service catalog from rendered node rows rather than the global
lowercase effect-name table. Provider-plan method schemas and fingerprints
likewise retain only canonical service names and independent
`may_suspend`/`may_block` ceilings; the duplicate lowercase method surface and
plan-wide compatibility bitset are gone. Dedicated may-axis fixed points never
depend on service rows or numeric bits. The global lowercase service catalog
and `u64` engine are deleted; std, canaries, and compiler fixtures author
boundary-trait identities. Build-script admission consumes exact service reach
and admits only the pinned canonical `FilesystemHost` and `Console` staging
slots; a custom boundary wrapper remains a distinct, rejected service.

Implementation status (`invokes`, 2026-07-31): syntax, resolved, typed, and
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
- Service reach, possible suspension, possible blocking, and positive
  termination remain independent after parsing, normalization, inference,
  diagnostics, and artifact emission.
- An exported authored service/operational contract is stable when prover
  strength changes; internal omissions reach deterministic least fixed points
  in their checked call component.
- A blocking provider cannot satisfy a slot that permits suspension but omits
  `blocks`.
- No semantic decision projects suspension or blocking from service reach, or
  service reach from an operational boolean.

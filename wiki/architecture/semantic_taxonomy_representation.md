# Semantic Taxonomy Representation Rework

Status: **high-priority compiler architecture task**, loaded 2026-07-18.

The domain-facet, machine-taxonomy, core-multiplicity, service/operational, and
termination/ranking settlements are not documentation-only classifications. They make distinctions that affect
resolution, interface identity, admissibility, ownership, diagnostics, and
lowering. If the compiler stores only the old undifferentiated shapes and
reconstructs the distinctions later, it loses a clean representation and will
eventually encode policy in scattered conditionals.

This record audits the current loss and defines the migration target. Decision
22 supplies the effects algebra and decision 23 supplies the termination
guarantee/ranking-witness firewall. The compiler must represent both directly
rather than extending old booleans and bitsets by convention.

## Current representation loss

### Domains

`omega-symbol-resolved-trees/src/domain.rs` and
`omega-typed-trees/src/domain.rs` represent every domain as one
`DomainDefinition` containing invariant `facts`, `operators`, and an explicit
normalized predicate/semantic facet pair. The pair is a transitional
compatibility projection populated once at syntax-to-resolved lowering and
copied to typed trees. It does not yet represent:

- an optional predicate body;
- semantic contributions keyed by compiler-owned roles;
- owner-authorized establishment relationships;
- denotation schema and implicit-weakening certificate/sealed theory;
- the distinction between flow knowledge and binding-site semantic
  qualification; or
- evidence-source identity for proof, checked establishment, transformation,
  and admitted receipt.

`omega-checked-trees::DomainFacts` is appropriately fact-shaped for predicate
membership, while qualification casts and emitted semantic commitments now
consult the explicit semantic facet. There is no complete role-keyed
semantic-qualification plan yet. Arithmetic policies survive through
compiler-specific `ArithmeticDomain` paths, which is useful bootstrap behavior
but not the general domain model.

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
as bodyless establishment, receipt-backed boundary guarantees, and
resource-frontier transformations land.

The compiler records whether each domain fact originated through checked proof,
owner establishment, validation, resource transfer, or accepted boundary
evidence. The owner-authorized requirement and provider receipt contribute to
trust identity; private proof and transformation witnesses remain
implementation evidence.

See
[`../design_briefs/authority_values_and_boundary_evidence.md`](../design_briefs/authority_values_and_boundary_evidence.md).

### Domain theory

Introduce a shared domain model used by symbol-resolved, typed, and checked
layers:

```text
DomainTheory {
    carrier,
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

Implementation status (DOM1/STR2, 2026-07-23): core, symbol-resolved, and typed
layers carry the normalized facet pair. Syntax lowering is the sole legacy
shape projection; downstream tree propagation copies it verbatim and both
resolved/typed structural snapshots publish it beside the semantic identity.
Semantic qualification and trust publication currently consume
`facets.semantic`, and qualification demands proof only when
`facets.predicate` is active. Repeated normalized declarations compare the
pair. Role-keyed semantic contributions, bodyless establishment routes,
receipt origins, and the checked core qualification relationship remain.

Implementation status (DOM1 body presence, 2026-07-28): `domain T::Fact;` and
`domain T::Fact {}` both parse as an explicit bodyless predicate-body record,
while `{ true; }` is explicitly predicate-bearing. Syntax, symbol-resolved, and
typed trees plus their snapshots preserve that record, and the compatibility
predicate facet is projected from it rather than reconstructed from fact
count.

Implementation status (P1a evidence origin, 2026-07-28): checked semantic facts
now carry an establishment-evidence axis independent of their program-point
origin. The normalized origin classes distinguish prover, checked validation,
owner establishment, checked transformation, admitted receipt, propagation,
and canonical qualification. A checked machine attached to a domain's carrier
may discharge its own bodyless result guarantee; the same route does not bypass
a bodyful predicate and an unrelated carrier machine cannot mint the fact.
Call-result binding and ordinary statement transfer preserve the evidence.
Granted selected provider plans attach their normalized plan fingerprint to
matching admitted facts, and checked artifacts publish
`05_qualification_evidence.json` with origin, source, program point, and receipt
identity. Exact owner-authorized admitted-subject matching is now live.
Canonical qualification conformance and the remaining independent
domain-theory records remain P1 work.

Implementation status (DOM alias expansion, 2026-07-28): transparent
declared-domain aliases retain independent syntax, resolved, and typed records.
Their constituent symbols resolve after the complete declaration set exists;
uses expand recursively to atomic facts before constrained-type and contract
identity, compatibility, admission, and executable predicate lowering.
Validation rejects empty, unknown, cross-carrier, cyclic, and public-to-private
expansions, while call diagnostics name the unmet atom. Compiler-owned `Carry`
atoms and `Carry::Portable` remain part of the separate per-claim carry
migration.

Implementation status (DOM1 generic propagation, 2026-07-23): typed
`TypeConstraintNode::Domain` is a normalized binding-site record, not a bare
name. A post-lowering pass resolves the short name only against declarations
whose target matches the constraint's carrier, then stores the declaration
symbol, semantic identity, and facet pair. Nested generic arguments and all
type-table copy paths preserve the record. Validation checks the record against
the carrier declaration; checked field/contract facts and byte predicates use
the stored symbol directly instead of repeating a global short-name lookup.
Typed snapshots publish the full record. Generic substitution is therefore no
longer a facet-loss boundary.

Implementation status (DOM1 per-axis composition, 2026-07-23): a constrained
type's domain chain is no longer projected to its first member. Predicate
facets compose conjunctively through implicit parameter requirements, checked
writes and constructions, entry/read facts, return/parameter implication, and
post-write re-establishment. Members without predicate bodies never enter that
fact lattice; their normalized identities remain on the type for semantic
qualification consumers. The remaining DOM1 work is to replace the
factful=hybrid/factless=semantic-only compatibility projection with the
independent domain-theory records.

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

Runtime behavior is a distinct normalized provider-plan record, not a fifth
carry axis and not a source type property:

```text
RuntimeBehaviorContract {
    preemption: SafePoints | Asynchronous,
    cpu: Preserved | MayMigrate,
    host_thread: Preserved | MayMigrate,
    continuation_address: Stable | Movable,
}
```

Suspension permission has no runtime counterpart: canonical liveness checks it
locally against the checked suspension plan. Admission joins the other three
carry dimensions with runtime behavior, while preemption granularity selects
the relevant crossing points. Unknown behavior normalizes pessimistically.
Checked provider evidence may prove a narrower record; accepted evidence needs
an ordinary admission receipt. No second provider/admission representation is
introduced.

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
State-graph metadata and backend reports consume that fact. Safe-point task
demands join crossings across the cycle-safe descendant closure, while
asynchronous demands join the descendant all-instruction envelopes. There is
no separate `contains` source form or compatibility carrier.

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
Runtime descriptor/object-safety design remains in `OWNER_QUESTIONS.md`.

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
class or a second effects row. The satisfied requirement supplies the public
contract/ceiling; validation and admission check the binding/provider behavior
as a refinement and produce any trust receipt. `ProviderPlan` is then derived
from explicit conformance closure rather than authored rows.

Source `boundary` remains insufficient to reconstruct this enum: a checked
exported callable and an accepted bodyless declaration both mention the word
but have different supply modes. Likewise, body absence distinguishes a trait
requirement only in its declaration context. Populate the enum once during
semantic lowering and carry it thereafter.

Termination needs an explicit interface/implementation split:

```text
TerminationGuarantee = NoGuarantee | EventualTerminal {
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
    continuation_requirement,
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

Implementation status (TR2/TR3, 2026-07-21): core owns the source-visible
`[linear] Task<T>` claim carrier plus `TaskOutcome<T>`,
`StartOutcome<T, Arguments>`, and the generic `TaskRuntime::start` /
`try_start` boundary surface. Symbol-keyed generic substitution preserves
conditional payload debt, with pass and scope-loss canaries covering returned
linear results and rejected linear argument bundles.

Concrete static-machine specializations retain their executable instance
symbol. The compiler derives a validated `TaskActivationPlan` for each closed
TaskRuntime start specialization and emits `05_task_activations.json`. The plan
uses checked contract/entry/layout/calling identities, the normalized
transitive suspension plan, canonical crossing liveness/carry facts,
and concrete target layout to size its continuation. Safe-point migration is
therefore evidence-backed. A separate checked all-instruction envelope joins
every persistent slot, parameter, local, call signature, aggregate/cast
temporary, and reference formation for asynchronous preemption; unresolved
coverage marks it incomplete and leaves the activation demand absent, so
admission fails closed. The carry artifact exposes that completeness and
joined policy. Every activation requires cancellation support because
cancellation-request authority is part of every `Task<T>` claim. Provider
admission/dispatch, claim provenance, and lease accounting remain later
task-runtime rungs.

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

Implementation status (CML4 migration, 2026-07-21): checked flow retains normalized
`Establish | Transfer | Consume | AffineDrop` events, including whether a
conditional sum event carries live payload debt. CML3's second slice propagates
the same typed events through state graph, control flow, abstract/target/
assigned operations, machine instructions/program/bytes, and the backend
report. The older move/drop arenas remain compatibility output only through
control flow and are dropped at the abstract-operation boundary; no backend
representation carries them, and no semantic producer or consumer may
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
establishment. A unique linear obligation returned from a state-local place is
joined to the caller's receiving establishment without minting a caller-local
origin. Nontrivial state-exit code actions now target the settled
`EdgeCleanupPlan`: materialize outgoing values, commit the transfer map, clean
the ordered dying affine places, and retain the exact conservation witness.
Composite per-field debt uses the settled path-indexed frontier: explicit
nominal linearity contributes one root, transparent aggregates contribute their
contained child claims, and static field/case/index moves preserve siblings.
Content-bearing n-to-m transformations additionally retain the selected
compiler-owned algebra, normalized claim projection and admitted backing,
root-lineage mapping, and exact separated-conservation witness. The initial
closed vocabulary is `Indivisible | Interval<Scalar>`; correspondence-bearing
symbolic mappings and runtime-indexed extraction remain fail-closed extensions.

### Service reach and operational ceilings

Represent service reach as symbol-resolved boundary-trait identities and keep
suspension and blocking in independent plans:

```text
ServiceReachId  = normalized boundary-trait identity
ServiceReachRow = normalized set of ServiceReachId + parent closure

ServiceReachPlan {
  interface: InternalInferred | PublishedCeiling(ServiceReachRowId),
  checked_inferred: ServiceReachRowId,
}

SuspensionPlan {
  interface: InternalInferred | PublishedMaySuspend(bool),
  checked_may_suspend: bool,
}

BlockingPlan {
  interface: InternalInferred | PublishedMayBlock(bool),
  checked_may_block: bool,
}
```

Boundary-trait declarations mint service identities. `suspends;` and `blocks;`
publish independent may-ceilings; omission on a public requirement is the
corresponding negative guarantee. Private omission infers each axis. The
deterministic normalizer owns service-row and operational contract identity;
the entailment engine may gate reachability or legality but never rewrite a
published ceiling. `MachineTerminationPlan` remains independent and retains
the positive `terminates` guarantee and private ranking witness.

Implementation status (EFX symbol-resolved service plans, 2026-07-23): `omega-core` now owns
distinct `ServiceReachId`/`ServiceReachRowId` identities, deterministic
service-row set normalization, and independent service, suspension, and
blocking plans. The operational interfaces distinguish private inference from
published `false`, preserving omission as a negative public guarantee instead
of treating it as “not computed.” Authored `suspends;` / `blocks;` clauses now
parse independently, survive syntax/resolved/typed trees and snapshots, enter
checked `MachineContractPlan` values and fingerprints, and drive task
admission. Operational names are rejected in source `effects` rows, normalized
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
effect-row field/input in machine contract artifacts and fingerprints are
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
- A domain carrying both a predicate body and semantic roles is representable
  without duplication.
- Static qualification survives generics and containers while proven
  predicates remain flow facts.
- A canonical bodyless qualification conformance is value-identical,
  terminating, behavior-free, owner-authorized, and erased.
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

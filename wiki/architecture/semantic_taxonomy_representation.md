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
normalized predicate/semantic facet pair. The pair represents hybrids without
duplication and is populated once at syntax-to-resolved lowering, then copied
to typed trees. They do not yet represent:

- authored predicate-versus-semantic facet policy (the compatibility
  projection classifies factful declarations as hybrids and factless
  declarations as semantic-only);
- semantic introduction policy or mint authority;
- denotation schema;
- implicit-weakening certificate/sealed theory;
- the distinction between fact membership and binding-site semantic
  qualification.

`omega-checked-trees::DomainFacts` is appropriately fact-shaped for predicate
membership, while qualification casts and emitted semantic commitments now
consult the explicit semantic facet. There is no complete parallel
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

`omega-effects` currently represents service names plus suspension/blocking
compatibility names as bits in one flat `EffectSet`. The source and validator
also use effect-name rows. This is adequate for the existing transitive check,
but it conflates boundary-service reach with operational behavior and loses
name resolution, service-parent closure, independent authored ceilings, pinned
slot ceilings, and per-axis provider refinement. It also encourages authority,
trust, resources, failure, and mutation to be folded into the set even though
they have separate semantic homes.

## Target representations

### Domain theory

Introduce a shared semantic domain model used by symbol-resolved, typed, and
checked layers:

```text
DomainTheory {
    carrier,
    predicate: Option<PredicateFacet>,
    semantic: Option<SemanticFacet>,
}

PredicateFacet { body/evidence visibility, normalized propositions, ... }
SemanticFacet  { introduction, denotation, operator theory, weakening, ... }
```

This must be a pair of optional facets, not a mutually exclusive enum, because
hybrids are first-class. Checked types/bindings carry a normalized
`SemanticDomainId`; flow facts carry predicate membership. The deterministic
normalizer owns semantic interface identity. Layout continues to use the
carrier ABI.

Implementation status (DOM1/STR2, 2026-07-23): core, symbol-resolved, and typed
layers carry the normalized facet pair. Syntax lowering is the sole legacy
shape projection; downstream tree propagation copies it verbatim and both
resolved/typed structural snapshots publish it beside the semantic identity.
Semantic qualification, commitment collection, introduction-authority lookup,
and trust publication consume `facets.semantic`, and qualification demands
proof only when `facets.predicate` is active. Repeated normalized declarations
compare the pair. Authored source policy, full facet bodies, and the checked
qualification plan remain.

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
post-write re-establishment. Semantic-only members never enter that fact
lattice; their normalized identities remain on the type for semantic
qualification consumers. The remaining DOM1 gate is genuinely language
design: freeze the authored facet declaration/policy surface, then remove the
factful=hybrid/factless=semantic-only compatibility projection.

Implementation status (DOM2 binding activation, 2026-07-23): checked operator
selection reads only static binding sources: normalized declared constraints,
explicit mints, and signature `requires`. The selector has no flow/fact-plan
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
structural derivation, and sealed per-mint domain facts all lower into this one
representation. Opaque omission produces the strict policy; opaque authored
relaxation remains an inert claim until validation/admission grants it.
Permission entries retain any provenance anchor needed to interpret `Origin`
or `Stable`. Aggregates share a field traversal with other properties but each
axis owns its composition algebra.

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

Implementation status (2026-07-19): `CarryPolicy` and its four closed axes live
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
evaluation. Opaque admission, per-mint qualification, contained-machine
subtrees, runtime admission, and artifact/model export remain. The legacy
contained-machine IR span currently has no parser surface; subtree carry work
waits on a deliberate retirement-or-reintroduction decision rather than
inventing semantics for an unreachable compatibility form.

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
and import-slot identity. `RankingWitness` does not: it feeds checker legality,
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
`StartOutcome<T, Arguments>`, and the opaque generic `TaskRuntime::start` /
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
`[linear]` maps to `Linear`. Keep `zero_init` and the four-axis carry policy
orthogonal to multiplicity. Flow joins operate over permission entries with
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
origin. Nontrivial state-exit code actions are owner-blocked on the
cleanup-edge, partial-value order, and proof/effect decisions in
`OWNER_QUESTIONS.md` under "automatic cleanup's graph-edge and partial-value
contract." Composite per-field debt is owner-blocked on the resource-frontier
and component-origin decisions under "composite linear value's resource
frontier." Broader
resource algebra remains.

### Service reach and operational ceilings

Represent service reach as symbol-resolved boundary-trait identities and keep
suspension and blocking in independent plans:

```text
ServiceReachId  = normalized boundary-trait identity
ServiceReachRow = normalized set of ServiceReachId + parent closure

MachineServiceReachPlan {
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

Authority possession, provider trust receipts, resource bounds, failure
outcomes, and mutation remain separate fields/analyses. Do not manufacture a
single all-purpose effect record. Provide compatibility projections to today's
`EffectSet` during migration; the flat set may remain a cache for legacy
service members after it ceases to be the semantic source of truth, but
suspension and blocking must not be reconstructed from it.

## Staged migration

1. **Inventory and invariants.** Add compile-time tests/snapshots showing where
   domain facet, supply mode, multiplicity, carry policy, and contract identity
   must survive.
2. **Core semantic enums/IDs.** Land facet pair, introduction policy,
   multiplicity, carry policy, supply mode, termination guarantee/witness,
  progress-profile ID, service-reach ID/row, suspension plan, blocking plan,
  and other identity handles in the lowest dependency-safe crates. No
   behavior change.
3. **Tree propagation.** Carry the representations through symbol-resolved and
   typed trees, snapshots, cloning/substitution, and diagnostics. Eliminate
   re-derivation from body shape/keyword presence.
4. **Checked plans.** Split predicate facts from semantic qualifications; add
  the place-keyed permission plan, service-reach plan, suspension plan,
  blocking plan, termination plan, and
   normalized machine contracts.
5. **Validation and resolution.** Enforce facet activation, introduction,
   operator selection, multiplicity conservation, carry derivation/local
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

- Domain mint/operator-family work must not grow on the undifferentiated
  `DomainDefinition` shape.
- Linear `Task<T>`, transactions, or dependent-linear buffers must not grow on
  move/drop-only ownership summaries.
- Component import slots and hot-swap manifests must not pin a body hash or
  flat service row in place of normalized machine contract identity.
- Ranking subjects, views, ranges, SCC mapping, and certificates must not enter
  published machine-contract identity.
- Service-row and operational-ceiling identity must not depend on prover
  strength, provider selection, or a legacy numeric bit.
- Import slots pin authored normalized service and operational ceilings;
  provider admission compares each axis independently and never consults a
  global import scan.

## Acceptance criteria

- No checked-stage query infers predicate-vs-semantic domain behavior by
  testing whether a domain happens to have facts or operators.
- A hybrid domain is representable without duplication.
- Semantic qualification survives generics and containers while predicate
  facts remain flow facts.
- Requirement/provider/accepted/checked supply modes survive into artifacts.
- A requirement's termination guarantee can be inherited while its checked
  implementation carries a private ranking witness.
- Replacing one valid ranking witness with another revalidates only the
  provider/proof artifact and leaves caller/import-slot contract identity
  unchanged.
- Runtime lowering may reject a non-tail ranked cycle while proof-time
  evaluation consumes the same checked machine and witness.
- A type has one explicit multiplicity and `zero_init` remains orthogonal.
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
- The legacy `EffectSet` service projection can be derived from the normalized
  service row during migration, but no semantic decision projects suspension
  or blocking back from it.

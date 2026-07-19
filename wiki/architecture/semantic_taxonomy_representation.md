# Semantic Taxonomy Representation Rework

Status: **high-priority compiler architecture task**, loaded 2026-07-18.

The domain-facet, machine-taxonomy, core-multiplicity, kinded-effect-row, and
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
`DomainDefinition` containing `classifier`, `facts`, and `operators`. They do
not represent:

- predicate versus semantic facets, including hybrids;
- semantic introduction policy or mint authority;
- denotation schema;
- implicit-weakening certificate/sealed theory;
- normalized semantic-domain identity; or
- the distinction between fact membership and binding-site semantic
  qualification.

`omega-checked-trees::DomainFacts` is appropriately fact-shaped for predicate
membership, but there is no parallel semantic-qualification plan. Arithmetic
policies survive through compiler-specific `ArithmeticDomain` paths, which is
useful bootstrap behavior but not the general domain model.

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

### Effects

`omega-effects` currently represents standard effects as bits in one flat
`EffectSet`. The source and validator also use effect-name rows. This is
adequate for the existing transitive-effect check, but it loses decision 22's
member kinds, name resolution, parent closure, authored public ceiling, pinned
slot ceiling, and provider refinement. It also encourages authority, trust,
resources, failure, and mutation to be folded into the row even though they
have separate semantic homes.

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
locally against possible `Suspend` reach. Admission joins the other three
carry dimensions with runtime behavior, while preemption granularity selects
the relevant crossing points. Unknown behavior normalizes pessimistically.
Checked provider evidence may prove a narrower record; accepted evidence needs
an ordinary admission receipt. No second provider/admission representation is
introduced.

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

Implementation status (TR2A, 2026-07-17): core owns the source-visible
`[linear] Task<T>` claim carrier, and task-specific canaries pin transfer,
conditional payload extraction, terminal by-value-self consumption, and scope
loss. Generic terminal/start outcome sums remain gated on qualifier-aware
payload propagation so substituted linear results and rejected argument
bundles cannot lose their debts.

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

Implementation status (CML3, 2026-07-17): checked flow retains normalized
`Establish | Transfer | Consume | AffineDrop` events, including whether a
conditional sum event carries live payload debt. CML3's second slice propagates
the same typed events through state graph, control flow, abstract/target/
assigned operations, machine instructions/program/bytes, and the backend
report. The older move/drop arenas remain compatibility output only; no
semantic producer or consumer may reconstruct permission kind from that lossy
pair.

### Effects and observation

Represent the qualitative effect row as symbol-resolved, kinded identities:

```text
EffectMemberKind = ServiceReach | OperationalMay
EffectMemberId   = normalized declaration/core identity
EffectRow        = normalized set of EffectMemberId + parent closure

MachineEffectPlan {
    interface: InternalInferred | PublishedCeiling(EffectRowId),
    checked_inferred: EffectRowId,
}
```

Boundary-trait declarations mint `ServiceReach` identities. The core mints the
small v1 `OperationalMay` set (`Suspend`, `Block`). The deterministic
normalizer owns row and exported-contract identity; the entailment engine may
gate reachability or legality but never rewrite a published ceiling.

Authority possession, provider trust receipts, resource bounds, failure
outcomes, and mutation remain separate fields/analyses. Do not manufacture a
single all-purpose effect record merely because the surface has one `effects`
clause. Provide a compatibility projection to today's `EffectSet` during
migration. The flat set may remain a fast cache for legacy members after it
ceases to be the semantic source of truth.

## Staged migration

1. **Inventory and invariants.** Add compile-time tests/snapshots showing where
   domain facet, supply mode, multiplicity, carry policy, and contract identity
   must survive.
2. **Core semantic enums/IDs.** Land facet pair, introduction policy,
   multiplicity, carry policy, supply mode, termination guarantee/witness,
   progress-profile ID, effect-member kind/ID, normalized effect-row ID, and other identity
   handles in the lowest dependency-safe crates. No
   behavior change.
3. **Tree propagation.** Carry the representations through symbol-resolved and
   typed trees, snapshots, cloning/substitution, and diagnostics. Eliminate
   re-derivation from body shape/keyword presence.
4. **Checked plans.** Split predicate facts from semantic qualifications; add
   the place-keyed permission plan, kinded effect plan, termination plan, and
   normalized machine contracts.
5. **Validation and resolution.** Enforce facet activation, introduction,
   operator selection, multiplicity conservation, carry derivation/local
   transition legality/runtime refinement, row inclusion/propagation, and
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
  flat effect row in place of normalized machine contract identity.
- Ranking subjects, views, ranges, SCC mapping, and certificates must not enter
  published machine-contract identity.
- Effect-row identity must not depend on prover strength, provider selection,
  or the legacy numeric bit assigned to a name.
- Import slots pin authored normalized ceilings; provider admission compares
  normalized rows by subset and never consults a global import scan.

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
- Service reach and operational possibility remain distinguishable after
  parsing, normalization, inference, diagnostics, and artifact emission.
- An exported authored row is stable when prover strength changes; an internal
  omitted row reaches the deterministic least fixed point of its checked call
  component.
- A provider carrying `Block` cannot satisfy a slot pinned to `Suspend` alone.
- The legacy `EffectSet` can be derived from the normalized row during
  migration, but no semantic decision depends on projecting back from it.

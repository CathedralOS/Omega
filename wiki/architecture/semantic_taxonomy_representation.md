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

`DataProperties` carries three booleans (`copy`, `zero_init`, `send`). There is
no first-class multiplicity. Control-flow and abstract-operation ownership
summaries record move and drop events only. They do not distinguish
establishment, transfer, linear consumption, affine drop, or a permission
entry's access/provenance. `CheckFacts` stores borrow and semantic facts in
separate fields, which is a sound starting point, but there is no unified
place-keyed permission context with per-entry algebra.

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

### Machine semantic contract

Introduce a normalized `MachineSemanticContract` (name provisional) containing
the complete substitutable contract plus an explicit `MachineSupplyMode`.
Syntax trees may retain source spelling, but provider admission, proof
artifacts, component manifests, compile-time evaluation, spawn checking, and
lowering must consume this normalized object rather than re-derive it from
`boundary`, bodies, and effect names.

Consumption eligibility should normally be derived views/queries over the
contract, not stored independent booleans that can drift.

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

Boundary progress profiles referenced by premises are sealed semantic
commitments with grant/receipt identity. They participate in provider
admission but remain outside the ordinary proof-fact catalog in v1.

### Multiplicity and permission context

Replace `copy` as the whole usage model with:

```text
Multiplicity = Unrestricted | Affine | Linear
PermissionEntry = place + establishment + multiplicity + access + provenance
OwnershipEvent = Establish | Transfer | Consume | AffineDrop
```

`[copy]` maps to `Unrestricted`; ordinary data defaults to `Affine`;
`[linear]` maps to `Linear`. Keep `zero_init` and `send` orthogonal. Flow joins
operate over permission entries with path-sensitive sum state. Borrow events
remain permission operations, not linear obligations by fiat.

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
   domain facet, supply mode, multiplicity, and contract identity must survive.
2. **Core semantic enums/IDs.** Land facet pair, introduction policy,
   multiplicity, supply mode, termination guarantee/witness, progress-profile
   ID, effect-member kind/ID, normalized effect-row ID, and other identity
   handles in the lowest dependency-safe crates. No
   behavior change.
3. **Tree propagation.** Carry the representations through symbol-resolved and
   typed trees, snapshots, cloning/substitution, and diagnostics. Eliminate
   re-derivation from body shape/keyword presence.
4. **Checked plans.** Split predicate facts from semantic qualifications; add
   the place-keyed permission plan, kinded effect plan, termination plan, and
   normalized machine contracts.
5. **Validation and resolution.** Enforce facet activation, introduction,
   operator selection, multiplicity conservation, row inclusion/propagation,
   and supply/admission rules.
6. **Lowering boundary.** Lower only from checked selections/plans. Preserve
   semantic contract IDs in proof/component/debug artifacts while erasing
   proof-only material from executable operations.
7. **Retire compatibility paths.** Remove compiler-special arithmetic-policy
   routing and boolean/context re-derivation only after their general
   equivalents have differential coverage.

## Ordering constraints

- Domain mint/operator-family work must not grow on the undifferentiated
  `DomainDefinition` shape.
- Linear `Join`, transactions, or dependent-linear buffers must not grow on
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

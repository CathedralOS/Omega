# Design Brief: Domain Facets And Semantic Qualification

Current design as of 2026-07-25. This brief records the facet model,
qualification and weakening, predicate evidence, operator coherence, and the
staged units model. Chapter 8 carries the language-guide surface.

## The model

A **domain** is a zero-cost semantic theory attached to an unchanged carrier.
It has two independently governed **facets**:

- **Predicate facet** — propositions about a value, resource, or current
  program state (`Utf8`, `NonZero`, `Alive`, evidence tokens). Established by
  proof; flow-establishable; lattice-composing (`&`/`|`); freely droppable;
  fully erased.
- **Semantic facet** — an explicitly selected interpretation and operator
  meaning (`Wrapping`, `Km`, `Degrees`' cyclic arithmetic). Introduced only by
  declaration, mint, or signature; binding-site-activated; reaches codegen
  through operator selection; never flow-acquired and never silently dropped.

A domain may have either facet or both (`Utf8` predicate-only, `Wrapping`
semantic-only, `Degrees` both). Attaching, proving, selecting, or forgetting a
domain never changes representation or adds runtime metadata. Validation and
conversion remain ordinary operations and may perform runtime work.

There is no separate "vocabulary" construct: an operator set is the emergent
family of `operator ... spelling` declarations whose signatures reference a
domain. Arithmetic policies (`Wrapping`, `Saturating`, `Trapping`) are the
compiler-blessed closed subset of semantic facets — special only because
primitive arithmetic needs direct lowering. Their direct lowering conforms to
the facet model.

## The five transitions

| Operation | Representation | Effect | Runtime cost |
|---|---|---|---|
| Refinement mint (`as`) | unchanged | certifies already-proved facts | none |
| Semantic qualification (`as`) | unchanged | makes an explicit, authorized commitment | none |
| Forgetting | unchanged | discards facts (free) or meaning (per weakening rules) | none |
| Conversion | may change | preserves denotation across representations (`km -> m` performs the x1000) | ordinary contracted call |
| Validation | unchanged | performs work whose postcondition establishes facts (`utf8_ok`) | ordinary contracted call |

One `as` serves both mint kinds. Diagnostics distinguish the two failure
classes: **"predicate obligation not discharged"** (a proof is owed) versus
**"introduction authority unavailable"** (a permission is owed).

Explicit forgetting and conversion are different operations and must never
share ambiguous behavior: forgetting `raw 1 in Km` yields `raw 1` (denotation
deliberately discarded); converting it yields the canonical value (`1000` in
metres). Both are explicit for unit-class domains.

## The laws

**Activation.** Flow inference may change what is *known* — establish
predicate facets, discharge obligations. Only declarations, mints, and
signatures change what operations *mean*. Operator resolution reads
binding-site declarations and never consults the fact environment.
`if x in Degrees { x + delta }` proves the range; the `+` stays exact.

**Monotonicity.** Increased prover power may turn rejection into acceptance;
it cannot reinterpret or reject an already-valid program (soundness bugfixes
excepted).

**Weakening.** A semantic domain weakens implicitly to its carrier only if

1. the identity representation map **preserves denotation**, and
2. every default operation **agrees with the qualified operation** throughout
   the default's accepted region.

Certified arithmetic policies pass both (raw 7 in `Wrapping` denotes the
integer 7; wrapping/saturating/trapping agree with exact wherever exact is
representable). Units fail condition 1 even where raw arithmetic coincides.
`Degrees` depends on its declared denotation map — each semantic domain
declares one, making the criterion checkable rather than intuited. The
criterion is **mechanically decidable for recognized schemas** (rationally
scaled units, blessed policies) **and otherwise proof-obligated via an
explicit `weakens_to` certificate — never guessed.**

**Sealed theory.** Once a `weakens_to` certificate is accepted, the certified
operator theory is **sealed**: a later operator extension overlapping an
operation on the weakening target must prove the same agreement law or be
rejected. The sealing law is the source of correctness; content-hashing the
normalized theory (proof-caching discipline) merely detects staleness and
forces rechecking — hashing cannot replace sealing or coherence, particularly
across separately compiled packages.

**Introduction authority.** Semantic introduction is owner-controlled by
omission. The owning package and holders of an exported, attenuable
`MintAuthority<D>` (contract-visible, proof-erased) may qualify. Open
introduction is an explicit declaration-site policy (`introduction open`) for
meanings such as units, where qualifying one's own measurement is an ordinary
authorial commitment.

**Predicate evidence.** Predicates need no introduction policy — facts are
proved, not authorized. But provability is scoped by **body visibility**: a
predicate whose body (or named-predicate machines) is package-private cannot
be unfolded by outsiders' flow or `as`. Outsiders establish or propagate it
only through owner-exported evidence — a transformer's postcondition
(`sanitize_sql -> Bytes in SanitizedForSQL`) or an exported decision
procedure's true-arm. The owner chooses the evidence surface.

A bodyless predicate is an abstract fact. Its membership arrives through
existing evidence or a checked resource transformation rather than unfolding a
carrier predicate. A `boundary domain` additionally permits an admitted
provider receipt to originate membership. This permission is additive:
checked proof, validation, propagation, and resource transfer remain available
according to the predicate's body and contracts.

An accepted boundary guarantee may establish predicate membership only when
the domain permits boundary evidence. The signature names the exact fact and
subject; provider selection and its receipt name the accepted evidence source.
This permission participates in domain trust identity and artifact reporting.

**Progress-profile customer.** A progress profile is a semantic domain over a
boundary-provider capability (`domain Scheduler::WeakFair { semantic; }`). It
classifies the provider commitment rather than an execution result.
Admission/grants authorize qualification; flow carries only its predicate
facts. V1 profiles supply no operators and entail no other profile.

> Facts may be established by anyone possessing sufficient proof; modules
> control the premises and evidence for abstract facts. Commitments may be
> introduced only according to the semantic domain's declared introduction
> policy.

**Propagation.** Refinement facts drop freely and flow through contracts (the
normalized fact catalog). Certified arithmetic policies weaken implicitly
where their denotation and operation-agreement obligations have been
discharged — sound because the exact-loud default reinstates obligations on
the far side. Units and kinds never weaken silently; crossing a carrier-only
boundary requires explicit forgetting or conversion. Ordinary domain generics
parameterize **carriers and semantic domains**; refinement polymorphism and
value-indexed facts (`Range<Lo, Hi>`, `Array<T, N>`) remain in contract and
dependent logic. Generic code must not invent, preserve, or restore a semantic
qualification unless its signature says so — `Vec<f64 in Km>` is not
`Vec<f64>`.

## Operators and coherence

- Operators are **free-standing tuple-resolved declarations**
  (`operator add(left: Km, right: Metres) -> Km spelling +`) — no privileged
  self/lhs position. Commutative flips are one-line delegations
  (`operator add(left: Metres, right: Km) -> Km = add(right, left);`).
- **Closed families by default.** An open family declares a **designated
  dispatch-owner position** — one owner per implementation key — which kills
  the sibling-collision case (A owns X, B owns Y, both otherwise eligible for
  `+(X, Y)`). Packages owning neither operand write an explicit adapter
  domain. Collisions among the eligible are hard errors, never ranked.
- **Coherence test:** adding an unrelated dependency cannot change resolution,
  invalidate typechecking, or introduce a new collision for an existing
  expression. This falls out of ownership anchoring — candidates come only
  from the participating domains, their owning packages, and the declared
  family; imports are never scanned for injectable meanings.
- **Resolution is a compile-time decision recorded in the checked artifact;
  runtime dispatch never repeats domain resolution.** Interaction with
  hot-swap:
  **bodies swap at machine boundaries gated by contracts; resolutions never
  travel.** A swapped-in operator body must re-discharge the declaration's
  contract and laws; a change to the declaration surface is a version change
  requiring dependent recompilation (chapter 22 territory), never a silent
  runtime rebind. Consequence for the backend: no inlining across a
  swap-point boundary without the inline being recorded in the swap manifest
  (see whole_program_assumptions.md).

## Normalization is not entailment

A small deterministic, confluent, terminating **normalizer** owns what a
domain expression *is* — canonical dimension vectors, scale products, kind
tags. Type identity, semantic interface identity, and monomorphization keys
depend only on it. The **entailment engine** proves propositions *about*
expressions and can never redefine canonical identity: identity must not
depend on solver timeouts, tactic selection, or a stronger future prover.
Shared arithmetic libraries; separated semantic roles. (This is the
type-identity face of the monotonicity law.)

Two identities are distinguished: the physical calling-convention/ABI, which
remains the **carrier's** ABI (representation erasure holds); and the
**semantic interface identity**, which includes normalized domains.

The compiler's first completed normalizer slice makes the current conjunction
algebra explicit. It resolves declared terms to semantic-domain identities,
uses canonical identities for the closed arithmetic policies, flattens nested
constraint shells, then sorts and deduplicates conjunction terms. The resulting
canonical type identity is the sole oracle for structural type matching,
operator operand signatures, generic-specialization grouping/fingerprints, and
task-plan type hashes. Diagnostic strings retain authored presentation and have
no identity role. This makes conjunction commutativity and idempotence stable
across source order while preserving distinct same-arity domain expressions.

## Units (the staged stress test)

`Quantity = structural dimension x nominal kind x rational scale x
presentation`, where:

- **Dimension** — product of base dimensions with exponents, canonical
  normal form (sorted vector, e.g. `{Length: 1, Time: -1}`). Computed
  structurally by `*`/`/`.
- **Kind** — nominal identity distinguishing same-dimension quantities
  (Energy vs Torque, both `Mass·L²·T⁻²`). A semantic facet: no flow
  inference, no silent drop, explicit mint in; kind-drop and re-mint are two
  visible steps (no *silent* laundering). v1: **closed derivation rules**
  shipped with the core units family (`Force x Displacement -> Energy`);
  external kinds require explicit mints until a coherent extension regime is
  earned.
- **Scale** — rational factor to the dimension's canonical unit.
  **Scale never changes silently**: it survives cancellation
  (`km / m = raw 1 in Dimensionless<1000>`); structure normalizes at compile
  time, but numeric scale normalization is a value operation and stays an
  explicit conversion.
- **Presentation** — a preferred spelling or display alias (`J`, `N·m`,
  `kWh`). **Non-semantic**: the normalizer canonicalizes presentation away
  before computing identity, which depends on dimension x kind x canonical
  scale only.

v1 rules: mixed-scale `+`/`-` requires explicit conversion (`km + metres` is
a compile error naming the one-mint fix; an explicit `add_as<Metre>(a, b)`
sugar may come later — ordinary `+` never gets context-sensitive meaning).
One generic operator family owns dimension arithmetic; unit packages
contribute **metadata** (dimension, scale, kind, affine status), not pairwise
operators. Affine quantities (Timestamp/Duration; CelsiusPoint/CelsiusDelta)
come after vector dimensions are stable. Logarithmic units, currencies, and
calendar arithmetic are explicitly out of the dimensional algebra.

## The test register

1. `Km + Metre` rejects without an explicit conversion.
2. `Km / Metre` yields a scaled dimensionless result preserving the 1000;
   **explicit forgetting yields the raw magnitude (1); conversion yields the
   canonical 1000; implicit erasure to `f64` is forbidden.**
3. Energy and Torque share a dimension and remain statically distinct kinds.
4. `Vec<f64 in Km>` survives a generic identity function unweakened.
5. `f64 in Km` cannot pass to a plain-`f64` parameter **without explicit
   forgetting or conversion**.
6. Energy cannot **silently** launder into Torque (drop + re-mint remain two
   explicit, auditable steps).
7. Two sibling packages cannot independently claim the same cross-domain
   operator tuple.

## Deferred (open design spaces, not contradictions)

- External quantity-kind extension (contributed kind equations under a
  separate coherence regime).
- General open-family linking (consent/linking mechanisms beyond the
  designated-owner rule).
- Surface grammar: explicit facet authorship, bodyless predicates,
  `boundary domain`, open semantic introduction, mint-authority passing, and
  `weakens_to` certificate blocks.

## Provenance

Distilled from an extended adversarial design review (2026-07). Key
corrections absorbed along the way: the facet model over a kind-split or a
separate vocabulary construct; the denotation clause in the weakening law; the
mint/qualification/forgetting/conversion/validation transition split;
sealed-by-default introduction; owner-exported evidence for
abstract predicates; the sealed-theory rule for weakening certificates;
normalization/entailment role separation; presentation excluded from identity.
Rejected en route: nominal wrapper types (`Wrapping<u8>` — the cost is the
name, not the bytes), transparent aliases, implicit coercion, flow-activated
vocabularies, hidden policy polymorphism, runtime tags of any kind.

## Cross-references

Chapter 8 (surface), chapter 16 (validation as fallible call), chapter 22 +
whole_program_assumptions.md (swap boundaries), the primitive-arithmetic
domain model, the normalized fact catalog, proof_caching.md (certificate
hashing discipline),
proof_engine_north_star.md (schema-decided vs certificate-proved is the
automation-plus-kernel pattern; domains-over-carriers is the named
substrate investment), architecture/semantic_taxonomy_representation.md
(required compiler representation and migration gate).

# Relations and quotients

A relation is a mathematical condition, not necessarily an executable decision.
Quotient formation needs a selected equivalence proof; each operation needs a
selected proof that its domain and result are independent of representatives.
The retained representative is not observable quotient meaning.

This contract depends on [general proof logic](contracts.md): proof-static
carrier families, relation expressions, witness/law bundles, and exact selected
conformances. General predicate and quantifier source forms remain undetermined.
Schematic relation notation below is mathematics, not an extra declaration form.
These rules imply no arbitrary runtime value-to-type computation or layout.

The set-quotient foundation below is the selected assumption-bearing interface,
not a completed implementation. It adds no primitive quotient reductions or
general higher inductive types to the [selected foundation](foundation.md).
The laws, their interpretation and executable adapters must be checked under
the stated assumption policy before claiming the corresponding guarantee.

## Set-quotient foundation

The notation in this section is kernel mathematics, not new Omega syntax. Start
with `A : Type u` and `R : A → A → Type v`, with selected proofs that every
`R(a,b)` is a mere proposition and that `R` is an equivalence relation. A mere
proposition has propositionally equal inhabitants; it need not be definitionally
irrelevant. The carrier `A` need not be a set.

An author may explicitly select the squashed version of a proof-relevant
relation, using `Box(Squash(R(a,b)))` to express it in relevant `Type`. This is
not implicit witness erasure. Original witnesses and their dependencies remain
distinct wherever retained. The quotient's laws concern the selected relation.

Use `Q(A,R) : Type max(u,v)`, without resizing it to a lower universe. Its
identity includes the carrier, relation and level application, not the chosen
equivalence/proposition-proof implementation. The mathematical carrier is opaque:
no ordinary record destructuring, recursor inferred from a private representative,
or executable provider is supplied by its declaration.

| Required item | Exact obligation |
| --- | --- |
| `project : A → Q(A,R)` | Introduce the quotient image of a representative. |
| `setQ` | `isSet(Q)`: for all `x,y : Q` and `p,q : Id Q x y`, `Id (Id Q x y) p q`. This is propositional, not definitional proof irrelevance. |
| `sound` | For all `a,b : A`, `R(a,b) → Id Q (project(a)) (project(b))`. |
| `effective` | For all `a,b : A`, `Id Q (project(a)) (project(b)) → R(a,b)`. It supplies evidence of exactly the selected relation. |
| Dependent elimination | A section into a set-valued family, given representative cases and the transport compatibility below. |
| Point computation | A relevant identity relating the section at `project(a)` to its supplied case. No new conversion rule. |

Precisely, for any explicitly checked level `w`, take:

```text
P : Q → Type w
sets : (z : Q) → isSet(P(z))
d : (a : A) → P(project(a))
c : (a b : A) → (r : R(a,b))
    → Id (P(project(b))) (transport(P,sound(r),d(a))) (d(b))

elim(P,sets,d,c) : (z : Q) → P(z)
beta(P,sets,d,c,a) : Id (P(project(a))) (elim(P,sets,d,c,project(a))) (d(a))
```

Here the last argument to `elim` applies its resulting section. The eliminator
does not target arbitrary higher types. Its coherence is expressed by `c`;
with set-valued fibers, comparisons between parallel transport proofs are
propositionally unique. No additional proof-irrelevance conversion is inferred.
Strict-valued properties use the reference core's boxing/unboxing rules.

The initial interface explicitly admits these carrier/operation/law declarations
as named mathematical assumptions, including effectivity. A selected checked
derivation may instead establish a required law, retaining its actual closure.
Do not silently use univalence, function/proposition extensionality or resizing
to call a law assumption-free. The
[HoTT quotient development](https://hott.github.io/Coq-HoTT/coqdoc-html/HoTT.HIT.quotient.html)
illustrates the transport-based interface, but its effectivity development uses
an explicit univalence context. Neither its private-inductive representation nor
its computational behavior is imported as an Omega kernel rule.

Derive the following from the selected interface, with checked terms, rather
than adding separate default axioms:

- Ordinary `lift`, using a constant set-valued family and representative congruence.
- Coverage: `Squash(Σ (a : A). Id Q (project(a)) z)` for each `z : Q`.
  This supplies no representative-extraction function.
- Proposition-valued induction, through set-valued elimination and boxing when needed.
- Pointwise uniqueness of sections agreeing on projections. Equality of entire
  functions from pointwise equality needs its own extensionality justification.

If the selected relation is a boxed squash, effectivity yields that boxed
existence evidence, and unboxing yields `Squash(R(a,b))`, not an original `R`
witness. Executable extraction does not follow. Quotient setness also does not
erase the identities or assumption dependencies of relation witnesses elsewhere.

## Representative layer and executable realization

Representative operations, equivalence proofs and congruence proofs remain
usable independently of the quotient assumptions. Existing canonical Rat code
is a starting implementation, not evidence that all invariant/congruence proofs
or dependency closures are complete. A receiver refusing quotient assumptions
must still be able to accept representative-level results whose actual closure
needs none; other mathematical assumptions remain subject to ordinary policy.

Proof-only values may participate in mathematical computation. Quotients are
not universally proof-only: the representation and materialization rules below
remain in scope. Axiomatic operations supply no algorithm, even when their
result carrier has a runtime layout. Evaluation and native use require checked
computation or a realization with the required correspondence. Proof computation,
runtime representation and axiom admission are separate judgments.

Acceptance pairs a quotient operation with its representative-level theorem
under a policy refusing quotient assumptions. The integration control depends
on conversion-independent transitive assumption closure, including helper types
and statements. Specify the control now; closure implementation is a named
dependency, not a reason to claim the test already passes. Also reject
representative-sensitive observation, illicit squash extraction, unavailable
set-valued elimination evidence and universe lowering without justification.

## Families, relations, and evidence

For carrier family `C` with complete typed index pack `I`, a heterogeneous relation
has mathematical shape `R<I,J>(left: C<I>, right: C<J>)`. The subjects are values,
not bare index symbols. The relation may instead share one pack and require equal
indices. No global `index`/`phantom` role on carrier parameters decides this.
Different families do not match; empty telescopes are the ordinary nullary case.

Without an authored heterogeneous relation, structural lifting requires equal
static arguments. A selected relation may deliberately relate different policies
or encodings. Normalized relation expressions, binders, and dependencies determine
semantic identity; a selected proof implementation does not redefine the relation.

Interpret a heterogeneous family in the set-quotient interface through its total
carrier over the admissible index telescope:

```text
A = Σ (I : IndexPack). C<I>
R_total((I,x),(J,y)) = R<I,J>(x,y)
```

The pack's dependent fields and universe levels are checked, not erased from
mathematical identity or resized to fit the carrier. A homogeneous fixed-pack
application uses `C<I>` directly. Formation requires explicitly selected law
evidence that every instance of the exact selected relation is a mere proposition,
as well as equivalence. For a proof-relevant relation, explicitly select its
boxed-squash relation and establish those laws; formation performs no implicit
relation replacement. This interpretation grants neither runtime storage for
arbitrary index packs nor representative extraction.

Relation laws use explicit named closed conformances, never suffix lookup,
visibility, declaration order, or bare exact-requirement satisfiers:

| Property | Required laws |
| --- | --- |
| Equivalence | Reflexivity, symmetry, transitivity. |
| Preorder | Reflexivity, transitivity. |
| Partial order | Preorder and antisymmetry. |

Generic selection follows the explicit-argument and closed-map rules for
[named conformances](../language/conformances.md#declaration-and-selection).

The laws quantify the appropriate packs independently: symmetry changes
`R<I,J>(x,y)` to `R<J,I>(y,x)`; transitivity composes `R<I,J>(x,y)` and
`R<J,K>(y,z)` into `R<I,K>(x,z)`. Equivalence inherits its laws without
redeclaring them. Changing a valid proof conformance changes proof provenance,
not relation or quotient identity.

A carrierless named conformance omits the subject and owns its entire telescope:

```omega
TogetherEvidence<machine Left, machine Right>:
    satisfies ConvergenceEvidence<Left, Right>
where machine Left(index: Nat) -> Rat;
where machine Right(index: Nat) -> Rat;
{
    // one closed row for every inherited requirement
}
```

It has a package-root identity, an explicit subjectless marker, and the same
normalized row keys as carrier-owned conformances. Trait arguments neither infer
its telescope nor choose an arbitrary carrier. Static-machine examples do not
replace the required general mathematical function/predicate binders.

Bundles carry witnesses and laws together. Repeated projection preserves the
same witness; two proofs of convergence may choose different moduli. Statement,
witness, and derivation identity remain distinct. Nonconstructive existence under
selected axioms need not yield an executable witness. Outcome, substitution,
scope, erasure, and trust rules follow [proof contracts](contracts.md).

## Formation and representation

Formation consumes a carrier family `C`, relation family `R`, and explicitly
selected `Equivalence<C,R>`, together with the selected mere-proposition evidence
for the exact relation described above. The quotient's identity includes the
relation, not the chosen equivalence implementation. Casting an exact carrier
instance with `as Quotient` is the sole construction route; struct/case literals cannot
construct it directly. Proving `R(a,b)` proves equality of quotient images,
not identity of the representatives.

Construction does not normalize. It may retain the chosen representative and
its ABI unchanged, while suppressing all synthesized representation observers:
structural equality, hashing, ordering, serialization, reflection, matching, and
representative extraction. Only a checked quotient operation licenses an observer.
An unchanged ABI is not permission to bypass semantic opacity.

Ordinary constant materialization may emit the retained representative without
canonicalization; equivalent constants need not have equal bytes. A proved
canonical form is required for stable serialization, public representation,
canonical const-index identity, structural interning/hashing, or reproducible
raw-bit observation. See [materialization](../language/constants.md#materialization).

Initial quotient carriers exclude affine/linear Type content and owned/routed
custody. Equating distinct authority or lease occurrences would launder custody
through logical equality. A future occurrence-preserving relation interface is
needed to admit them; proof irrelevance and result congruence are insufficient.

Mathematical equality may depend on selected axioms, whose exact assumptions
remain transitive dependencies accepted by the consumer. A checked-only policy
rejects admitted laws. A conformance wrapper cannot make an assumed equality
assumption-free. Formation/lifting has no special rule for Rat, Real, Cauchy
sequences, moduli, or convergence; those are library subjects.

## Lifting operations

Every operation explicitly selects a representative machine `F` and a checked
resultless congruence theorem. Equivalence alone licenses no operation. No
ambient theorem search, runtime proof dictionary, or second witness-selection
mechanism exists.

The compiler derives the expected theorem schema from exact `F` application,
argument correspondence, input relations, and requested result relation. Each
quotient-bearing argument appears as a left/right representative pair; an ordinary
pass-through argument uses one shared binder in both calls. The theorem requires
the selected relations and legality of both calls, then proves result congruence.
Both calls must denote under that theorem's own premises, before any later
consumer supplies a public precondition. Extra premises, a finer relation,
redirected calls, missing/duplicate representatives, or rebound shared arguments
reject. Validation of an explicit selection is not discovery.

Let `Q` be the public quotient precondition and `P` the representative precondition.
The owner authors `Q`; implementation changes cannot rewrite it.

| Form | Required correspondence |
| --- | --- |
| `Quotient::lift<F,Congruence>` | The complete exact/arithmetic judgment proves `Q -> P` for every admitted representative application. |
| `Quotient::lift<F,Congruence,Transport>` | One selected resultless theorem proves the complete ordered implication schema. |
| `Quotient::define<F,Congruence>` | `Q <-> P`, position-preserving arguments, matching modes/multiplicities, and unchanged result flow on every normal exit. |

`lift` permits explicitly authored adaptation, constants, omission, permutation,
duplication, and result computation once the complete obligations hold. A selected
transport theorem remains authoritative even if automation could also prove the
implication; rows do not mix automatic and theorem-supplied transport.
`define` checks normalized IR, not a recognized source-body spelling. Any argument
or result adaptation rejects with a suggestion to use `lift`. For partial `F`,
the universally checked forward implication supplies legality for both
representatives; no additional public biconditional surface is implied.

The selected theorem must be bodyful, checked, pure, crash-free,
non-suspending/nonblocking, and terminating. A boundary/admitted declaration
cannot occupy that selection. Fact-only use emits no representative pairs,
runtime call, dictionary, or fuel charge. Recursive theorem citation still
requires the exact strict-edge [induction rule](contracts.md#citation-and-induction).
Initial representative machines are pure and terminating, with only semantic
preconditions and normal results observable. Effectful lifting needs a relation
over complete behavior; result congruence cannot prove matching I/O or crashes.

Each operation retains one canonical role-ordered theorem collection. Every form
has exactly one `Congruence`; only three-argument `lift` also has exactly one
`ForwardPreconditionTransport`. A role discriminant, exact selected application,
role-specific correspondence, and common eligibility enter identity. Missing,
surplus, duplicate, reordered, or unknown roles reject. There is no reserved
reverse-transport role. Selection/provenance is operation-owned, not variable by
call site. Retain public operation, `F` application, exact relations, positional
correspondence, form, and contract/result-flow evidence through publication.

## Executable observers

Logical quotient equality needs no decision procedure. Executable equality is
unavailable until a named quotient-owned operation proves `DecidesEquivalence`:
`equals(x,y) == true <-> R(x,y)`. Representative independence alone is weaker:
a constant-false answer would satisfy congruence but not this law.

The exact checked law from the named conformance may occupy the lift's theorem
position. Together with equivalence it derives congruence; the author need not
prove both. Quotient formation does not bind `==`; the ordinary fixed-operator
declaration may associate that token with the named operation.

Other observers need representative independence plus their actual semantic
law, not one generic substitute. Ordering proves its ordering claims. A canonical
representative remains equivalent and is idempotent. Hashes agree for equivalent
values, but equal hashes need not imply equivalence. Where no named role interface
exists, the ordinary checked operation contract carries its law.

## Constructor relations

A relator binds independent left/right packs; using one pack is its homogeneous
case. The container owner may provide several named checked policies. The quotient
owner selects one for each relation/container-family pair, retaining it in
semantic identity. No ambient default or conformance-priority rule fills a missing
pair.

Transparent nondependent products derive structural lifting recursively from field
relations. A coarser selected lift needs a checked implication from structural
lifting. Opaque constructors publish that bridge explicitly, with all assumptions
visible to the receiving policy.

Dependent fields lift in dependency order. Relate earlier witnesses first, then
normalize dependent applications under those facts. Strict logical proofs of
the same application may use proof irrelevance. Type witnesses and relevant
identity proofs instead need their appropriate equality/transport; erasure
alone supplies neither. A coarser witness relation needs an authored transport theorem.
The quotient owner owes it because it chose that relation. An opaque interface
without sufficient transport makes the lift unavailable.

Erased fields remain in this analysis. A relation depending on erased Type
content has no derived runtime decider unless checked evidence shows that content
is determined by the runtime-relevant projection. Report the exact undetermined
component. Proof irrelevance never identifies different proposition applications.

## Diagnostics and publication

Failures identify the exact failed join: expected theorem schema/application,
public-to-representative implication, reverse implication for `define`, argument
position/mode, result-flow exit, or equality soundness/completeness direction.
Missing automatic implication points to explicit transport; representation
observation points to a named lifted operation. Effectful, nonterminating, or
custody-bearing requests name their unsupported behavioral/occurrence relation.
Diagnostic reconstruction supplies no certificate or execution authority.

### Published quotient correspondence

The current retained Terminal table is proof-only: monomorphic total direct
faithful `define` and position-preserving direct `lift` with congruence and forward
transport evidence. Rows retain exact public callable, selected application,
theorem roles, relations, eligibility, fact coordinates, and direct result shape.

Role tags precede application and payload in identity. Transport retains Left/Right
application side and authored-source/selected-theorem coordinates. Rows and their
evidence use canonical role-specific ordering. Decode and validation rederive
identity and the complete congruence/transport join; substituted, missing,
reversed, surplus, or role/payload-mismatched evidence rejects.

The table owns no executable operation. Execution rejects nonempty tables until
executable quotient lowering exists. Proof-only package review reconstructs the
complete source batch; retention alone is neither checked executable projection
nor source-to-native correspondence. [Validation support](../../../omega-rust/psi/semantics/validation/README.md#quotient-correspondence)
records producer limits; `QUOTIENT-THEOREM-LIFT` and `PROOF-CONTRACT-MIGRATION`
on the [execution board](../../../TASKS.md) own remaining implementation.

# Law-Bearing Relations, Evidence, And Quotients

Current design as of 2026-08-08. This brief resolves the former law-bearing
relations and quotients owner question. It
supersedes the current quotient pilot's executable-`bool` relation and
suffix-based discovery of `_reflexive`, `_symmetric`, `_transitive`, and
respect proof machines.

## Governing law

> A quotient relation is a proposition established by evidence, not a
> decision procedure. Forming a quotient requires a selected equivalence
> conformance. Lifting an operation requires a selected proof that both its
> domain and its result are independent of the representative.

The compiler knows no `Rat`, `Real`, Cauchy sequence, modulus, or convergence
rule. Core packages author those declarations and proofs through the general
surface below.

## Ordered dependency

Evidence-bearing quotients depend on a proof-side fragment of the
dependent-types ladder:

1. carrier families with typed, proof-static index telescopes;
2. proposition-valued families over representative values;
3. carrierless selected-conformance evidence; then
4. relation-property conformances, quotient formation, and quotient lifting.

These items are not independently orderable. The systems dependent-contract
fragment remains separate: this ruling admits proposition-valued dependency
in the erased proof stratum, not arbitrary value-to-runtime-`Type`
computation, runtime proof objects, or value-directed layout.

The rational-carrier work also supplies the first rung of F7's float semantics.
Public `Rat` carries an `IntPair` numerator over a
positive `Nat` denominator, and `mk_signed_rat` canonicalizes the difference
pair before reduction. `rat_gap` remains `Nat`-valued; its reflexive, symmetric,
and shared-denominator triangle theorems were rebuilt over the signed
coordinates without changing their public statements. `FloatMeaning` and float
target providers must consume this one public carrier rather than grow a
parallel private rational theory.

## Carrier families and heterogeneous representatives

Every proof carrier participating in this facility is read as a family with a
typed index telescope, which may be empty:

```text
Rat                         index telescope = ()
CauchySeq<machine S>        index telescope = (machine S : Nat -> Rat)
```

Write an arbitrary instance as `C<I>`, where `I` denotes the complete index
pack. A binary relation over the family has the normalized shape

```text
R<I, J>(left: C<I>, right: C<J>) : Proposition
```

Relation subjects are representative values, never bare index symbols.
`CauchySeq<A>` and `CauchySeq<B>` may have different concrete types while
remaining instances of the same carrier-family identity. `Rat` is the
nullary case, so its index packs disappear. Quotient carrier matching uses the
family identity and rejects representatives from another family.

Heterogeneity belongs to the proposition telescope, not to a global role on a
carrier parameter. One proposition may bind independent `I` and `J` packs,
while another over the same carrier may use one shared pack and therefore
require identical indices. Declaring the proposition only forms that formula;
checked evidence establishes its applications, selected relation-law
conformances license quotient formation, and `Respects` licenses operations.
Carrier declarations consequently have no `index` or `phantom` relation
properties.

Structural lifting is conservative when no authored relation says otherwise:
corresponding static arguments must be identical. A heterogeneous proposition
or selected relator may instead bind distinct left and right packs and prove
the required relation. This keeps a static policy such as
`Encoded<Utf8>` distinct from `Encoded<Ascii>` by default without preventing a
different proposition from deliberately relating them.

The committed telescopes here are proof-static. This brief does not admit a
runtime-dependent carrier such as a vector indexed by a runtime value.

## Proposition families and evidence

A proposition family states a fact depending on representative values:

```omega
proposition rat_equivalent(left: Rat, right: Rat);
```

A nominal `converges_together<Left, Right>(left, right)` proposition publishes
`ConvergenceEvidence<Left, Right>` as its one opaque evidence interface:

```omega
proposition converges_together<machine Left, machine Right>(
    left: CauchySeq<Left>,
    right: CauchySeq<Right>
) evidence ConvergenceEvidence<Left, Right>;
```

The `evidence` clause is owner-controlled, fingerprinted signature content. A
witness-bearing proposition contains exactly one interface. The clause states
what an erased term of the proposition may project; it neither supplies a
producer nor exposes the selected producer in mathematical APIs.

Its truth is inhabitance by checked proof evidence, not the result of running
a `bool` machine. An arbitrary pair of generator machines cannot in general
be tested for convergence, but a package may exhibit a modulus and prove its
universal law.

`proposition` is a proof-formula declaration and generic binder kind. It has
no result, runtime value, layout, operation contract, lowering, or executable
body. A primitive fact-only declaration ends with `;`. A witness-bearing
declaration's `evidence` clause is its one canonical carrierless interface.
The owner thereby fixes both introduction and elimination: every establishing
conformance supplies that interface, and a named binding projects the exact
retained evidence term in the proof stratum.

The proposition remains nominal rather than definitionally equal to its
evidence package. Its interface is normalized, fingerprinted semantic content,
so changing that interface is a breaking proof-interface revision without
turning the selected conformance into the proposition's identity. An ordinary
`where` bound cannot carry this role, and `=` remains reserved for transparent
logical expansion.

An ordinary checked proof machine may establish either form through its
`ensures`. For a witness-bearing proposition the proof must also supply the
declared evidence. Relation-law contracts that already name the proposition
establish their applications by ordinary entailment; they need no separate
authorization route.

Naming a `requires` clause binds its exact incoming evidence term. Naming an
`ensures` clause declares an exact outgoing evidence term, definitely assigned
once on every outcome path where that clause applies. Member projection uses
ordinary `term.member` syntax; no `open` form exists. Producer conformance
selection occurs privately when the proof body assigns the named output, while
forwarding assigns an existing term and preserves its identity.

The initial forwarding form is a bare-name assignment from a current machine's
named `requires` term to one of its named `ensures` terms. It erases before the
runtime statement stream and records an exact checked source-to-output binding;
both normalized proposition application and evidence interface must match. It
does not search visible facts or mint another witness. A concrete non-generic
subjectless conformance alias may instead introduce the output. That selection
retains its exact conformance and evidence-trait symbols plus the complete
normalized realization rows; a different interface rejects. Instantiated
generic producer aliases remain fenced. Producer assignment and path-sensitive
outgoing definite assignment stay separate. Definite assignment records each
erased assignment's exact statement coordinate and carries assignment state
through the finite named-state graph.
Every ordinary outcome must assign each outgoing slot exactly once; duplicate
assignment rejects, and a crash-only outcome has no outgoing package. A
state-level named `requires` binds an exact arrival term, and every named
transition carries a separate erased identifier lane after `;`; checked binding
substitutes the target state's ordinary arguments before comparing proposition
identity. Machine-level incoming terms remain live across internal transitions
and are not redundantly repassed in the state-arrival lane.

Named requirements are positional erased inputs. Calls pass them explicitly in
clause order after a `;` lane separator, never through visible-fact or
conformance inference. Each position checks the supplied term against the
callee proposition after ordinary call-argument substitution; source and
callee binding names do not participate in matching. The separator remains in
an evidence-only call (`callee(; proof)`) and is omitted only when there is no
evidence lane. Named guarantees are
public fields of an inferred,
source-unnameable, compiler-generated nominal output package. The package may
be retained, projected, or completely destructured; evidence fields erase and
remain subject to ordinary multiplicity. Outcome-specific evidence appears
only in the matching outcome shape.

A transparent logical definition uses `=`:

```omega
proposition cauchy<machine Sequence>(value: CauchySeq<Sequence>) =
    converges_together<Sequence, Sequence>(value, value);
```

It expands before normalization and has no independent semantic identity. It
inherits the expanded proposition's requirements, trust class,
fact-or-witness classification, and evidence interface. Only its source name
survives for diagnostics and debug maps.

Proposition parameters state their kind and application signature explicitly:

```omega
trait Reflexive<C, proposition Relation>
where
    proposition Relation(left: C, right: C);
```

A `bool` machine application remains a valid contract fact: in a fact position,
bare `decision(a, b)` and `decision(a, b) == true` normalize identically. It may
back a transparent proposition definition when its proof expression is total
and otherwise fact-position eligible. Primitive propositions cover facts for
which no decision procedure exists.

The existential package reuses the complete conformance-row projection. One
requirement projection has two strata:

- a carrier-bearing, dynamically eligible machine contributes a runtime table
  slot;
- a carrierless machine contributes a stable opaque proof symbol plus its
  normalized contract;
- a law contributes its contract and never a runtime slot.

For example, convergence evidence contains a hidden modulus and its law:

```text
ConvergenceEvidence<A, B>
|- modulus(precision: Nat) -> Nat       opaque proof symbol
`- close_after(...)                    checked universal law
```

Projecting the same evidence term twice yields the same opaque symbol. Distinct
evidence terms may yield distinct proof symbols even when they establish the
same proposition. Proof irrelevance applies to proposition identity and runtime
representation; it does not make hidden witness terms definitionally equal
inside a proof.

A selected dynamic value may be owned by value when its *entire normalized
dynamic representation* has no runtime carrier. Absence of table slots alone
is insufficient: a runtime instance with no eligible methods may still have
unknown size and cleanup. Carrierless convergence evidence has neither an
instance nor slots, so an erased evidence term can retain an opaque selected
implementation without allocation, size/alignment metadata, cleanup, or a
source-visible conformance name.

Published mathematical APIs expose proposition names such as
`ConvergesTogether(a, b)` and transparent conveniences such as `Cauchy(s)`.
The underlying carrierless selected conformance is mechanism, not user-facing
mathematical vocabulary.

## Relation-property hierarchy

Relation laws are ordinary explicit conformances. The compiler does not find
free machines by suffix or privileged global name. Each selected law
conformance is one closed implementation block; proof machines written in that
block or explicitly referenced by its trait-qualified rows supply the laws.
Bare exact-requirement satisfiers remain usable as proof lemmas but do not by
themselves form a selectable `Equivalence` conformance.

The reusable properties are independent:

```text
Equivalence<C, R>
|- Reflexive<C, R>
|- Symmetric<C, R>
`- Transitive<C, R>

Preorder<C, R>
|- Reflexive<C, R>
`- Transitive<C, R>

PartialOrder<C, R>
|- Preorder<C, R>
`- Antisymmetric<C, R>
```

Their normalized laws quantify index packs independently:

```text
Reflexive:
  forall I, x: C<I>.
    R<I, I>(x, x)

Symmetric:
  forall I, J, x: C<I>, y: C<J>.
    R<I, J>(x, y) -> R<J, I>(y, x)

Transitive:
  forall I, J, K, x: C<I>, y: C<J>, z: C<K>.
    R<I, J>(x, y) ->
    R<J, K>(y, z) ->
    R<I, K>(x, z)
```

`Equivalence` composes the three parent requirements and redeclares no law.
Several conformances may prove the same property. Selection always passes one
package-scoped conformance name explicitly; visibility, specificity, and
declaration order never choose it.
Changing the proof conformance does not change the nominal relation or quotient
identity.

The proposition or other static subject of a law conformance is authored by
the law surface, never inferred from parameter occurrence. A truly subjectless
evidence interface uses the subjectless conformance form and receives a stable
package-scoped name; it is not forced onto an arbitrary parameter merely to
reuse a type-owned namespace.

The settled source form names the implementation first and owns its complete
telescope. A carrierless implementation simply omits the subject:

```omega
TogetherEvidence<machine Left, machine Right>:
    satisfies ConvergenceEvidence<Left, Right>
where machine Left(index: Nat) -> Rat;
where machine Right(index: Nat) -> Rat;
{
    // one closed row for every inherited requirement
}
```

It lowers to the shared resolved/typed conformance representation with an
explicit subjectless marker, a package-root `TogetherEvidence` symbol, and the
same exact normalized row keys used by carrier-owned blocks. Its inline
realizations have no attached data carrier. Trait arguments never infer the
telescope or nominate a carrier.

Fact-only versus witness-bearing classification does change proposition
identity, because it changes what a consumer may eliminate. Primitive
proposition symbols, binders, and that classification are fingerprinted in
terminal Psi. Transparent definitions are expanded and remain only in source
and debug metadata.

## Quotient formation

The quotient former consumes:

1. a proof carrier family `C`;
2. a proposition relation family `R` over heterogeneous instances of `C`; and
3. a selected `Equivalence<C, R>` conformance.

Conceptually:

```omega
data Real = CauchySeq % ConvergesTogether
where
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

The relation, not the selected proof implementation, enters quotient identity.
`representative as Quotient` is legal only for an instance of the declared
carrier family. Proven `R(a, b)` establishes equality between their quotient
images; quotient equality means membership in the same bucket, never
representative identity.

An admitted boundary axiom may state an environmental assumption elsewhere,
but it cannot establish the equivalence or respect conformances of a checked
quotient. Quotient substitution is an internal logical construction whose
false equality would propagate without a containment boundary. A relation
resting on environmental equality produces an explicitly axiomatized type
instead.

## Lifting operations: `Respects`

Equivalence licenses the quotient type; it does not license operations on it.
Every lifted operation needs a selected `Respects` conformance.

Normalize any machine's parameters, including an attached receiver, into one
ordered argument telescope:

```text
F : (argument_0, ..., argument_n) -> Result
requires P(argument_0, ..., argument_n)
```

The selected `Respects` requirement exposes two parallel copies of this
telescope. Positions are semantic identity; source parameter names are local
debug aliases and do not enter the fingerprint. Attached and free machines
therefore share one form without a generated global record name or an
author-declared adapter.

`RA` is the pointwise lift of the quotient relations selected for the
representative-bearing positions. It is never an arbitrary author-supplied
relation: a relation that is too fine could make both respect obligations
vacuous. `RR` comes from the requested lifted codomain relation, not merely
from the result type, because one result carrier may support several
quotients. `Respects<F, RA, RR>` proves two clauses.

### Domain invariance

```text
RA(x, y) -> (P(x) <-> P(y))
```

Equivalent representatives must agree on whether the operation is callable.
For a total machine `P` is true and this clause discharges structurally.
Only semantic preconditions depending on the representative participate.
Fixed ambient facts, authority, and resource requirements do not vary by
representative and remain ordinary machine-contract obligations.

The compiler finds the representative-dependent portion by semantic
dependency, not textual mention. A conjunct depending directly or indirectly
on at least one quotiented position enters `P`; a conjunct depending only on
ambient authority, capacity, or other fixed subjects does not. A conjunct
mixing representative and ambient subjects is conservatively
representative-dependent.

### Result congruence

```text
RA(x, y) && P(x) -> RR(F(x), F(y))
```

Domain invariance supplies `P(y)`. A failure represented as an explicit result
sum participates in `RR`; it does not need a second lifting mechanism.

For binary Real addition, `Arguments` contains both operands, `RA` is the
fieldwise product of `ConvergesTogether` for both places, and `RR` is
`ConvergesTogether`. Division additionally proves that equivalent
denominators agree about being zero. Comparison uses equality as its result
relation. The parallel-telescope normalization avoids an open-ended
`Respects1`/`Respects2`/`Respects3` hierarchy.

## Relation lifting through constructors

A relator supplies a proposition-valued `Lift` member. Its normalized form is
inherently heterogeneous:

```text
Lift<I, J, R>(left: C<I>, right: C<J>) : Proposition
```

Instantiating both sides with one pack is the homogeneous case; it is not a
different relator. The container owner may publish several named, checked lift
policies, such as structural and unordered lifting. The quotient owner chooses
the exact policy for each `(quotient relation, container family)` use. That
selection is retained in semantic identity. There is no ambient default
relator and no conformance-priority rule; an uncovered pair rejects at
instantiation with the missing pair in the diagnostic.

For a transparent non-dependent product, the compiler derives the structural
lift recursively from the supplied field relations. An owner-provided coarser
lift is accepted only with a checked bridge showing that the structural lift
implies the chosen lift. An opaque constructor must publish the same bridge as
checked evidence; admitted relation or `Respects` evidence cannot license `%`.

Dependent records lift in dependency order rather than as independent fields.
For example:

```omega
data Certified {
    root: RootId;
    proof [erased]: Authorized<root>;
}
```

The lift first relates the two `root` fields, adds that fact to the relational
environment, and then normalizes the two `Authorized<...>` applications. If
the roots are equal, the proposition applications coincide and proof
irrelevance discharges the evidence field. A coarser root relation requires an
authored transport theorem for `Authorized` under that relation.

The quotient owner must discharge that transport obligation because it chose
the coarser relation. The dependent type owner may publish a conditional
generic lift, and the proposition owner controls which elimination or
transport laws its proposition exposes. If an opaque proposition exposes no
sufficient theorem, the quotient owner cannot manufacture one and that lift
is unavailable.

An erased field remains part of this proof-side dependency analysis even
though it has no runtime representation. A lifted relation depending on
erased `Type` content is well-defined but has no derived runtime decider unless
checked evidence shows that content is determined by the runtime-relevant
projection. Requesting a decider without that evidence reports the exact
undetermined erased component. Proof irrelevance hides evidence identity only
after the proposition applications themselves agree; it never equates proofs
of different propositions.

## Cauchy construction

Core's landed metric theorem already proves the mathematical transitivity
step:

```text
close(p, q, 2e) && close(q, r, 2e) -> close(p, r, e)
```

Given evidence with opaque moduli `M1` and `M2`, transitivity defines the
symbolic witness

```text
M3(e) = max(M1(2e), M2(2e))
```

and proves its law from the two published laws plus the existing rational
triangle theorem. It never evaluates either hidden modulus at a numeral.
A transparent concrete modulus may still normalize when a separate proof
needs a concrete numeric result; evaluability is not required for quotient
transitivity.

## Separate compilation and resources

The evidence producer checks the selected conformance. A consumer projects a
named published evidence term to artifact-local opaque symbols characterized
solely by the normalized contracts; witness implementation identity does not
cross the boundary. Proof-only evaluation, when a transparent body is actually
needed, uses the ordinary gated build-time evaluator: semantic eligibility
requires the complete checked invocation contract and ordinary termination.
Deterministic work metering supplies progress, caching/accounting evidence,
warnings, and any
optional root-selected ceiling; long or unlimited evaluation remains legal.

## Migration and acceptance

The current N6 pilot accepts a generic `bool` relation and discovers
`relation_reflexive`, `relation_symmetric`, and `relation_transitive` by naming
convention. That is implemented legacy behavior, not the final semantic model.
Migration must retain its heterogeneous-family coverage while replacing the
decider and suffix lookup with proposition evidence and explicit selected
conformances.

Acceptance requires:

1. primitive and witness-bearing proposition declarations retain their exact
   binder telescopes and classification, while transparent definitions expand
   without acquiring an identity;
2. carrierless evidence owns no runtime words and named terms project stable
   opaque symbols;
3. a proof of a witness-bearing proposition without its evidence rejects;
4. named proof inputs are passed explicitly in the erased call lane, named
   outputs are definitely assigned per applicable outcome and retained in
   inferred nominal packages, and no visible-fact or conformance inference
   selects either;
5. proposition, evidence-term, and derivation-provenance identities remain
   distinct through forwarding, serialization, and checking;
6. a literally bodyless ordinary theorem machine rejects, an explicit boundary
   axiom is reported as admitted, and an equivalence depending on it cannot
   license `%`;
7. different family indices may relate, while a different carrier family
   rejects;
8. quotient formation requires explicit reflexive, symmetric, and transitive
   conformances through `Equivalence`;
9. a total respecting operation lifts;
10. a partial operation rejects unless equivalent representatives agree on its
   precondition;
11. an operation whose result depends on representative choice rejects; and
12. no compiler rule mentions `Rat`, `Real`, Cauchy sequences, moduli, or
   convergence.

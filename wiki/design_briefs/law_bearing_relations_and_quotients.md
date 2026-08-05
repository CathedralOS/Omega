# Law-Bearing Relations, Evidence, And Quotients

Current design as of 2026-08-03. This brief resolves the former law-bearing
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
Landed 2026-07-28: public `Rat` now carries an `IntPair` numerator over a
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

The committed telescopes here are proof-static. This brief does not admit a
runtime-dependent carrier such as a vector indexed by a runtime value.

## Proposition families and evidence

A proposition family states a fact depending on representative values:

```omega
proposition rat_equivalent(left: Rat, right: Rat);
```

A nominal `converges_together<Left, Right>(left, right)` proposition publishes
`ConvergenceEvidence<Left, Right>` as its one opaque evidence interface. The
exact dedicated source clause for that interface remains
[`OWNER_QUESTIONS.md` Q5](../../OWNER_QUESTIONS.md); its semantic role is
settled independently of that spelling.

Its truth is inhabitance by checked proof evidence, not the result of running
a `bool` machine. An arbitrary pair of generator machines cannot in general
be tested for convergence, but a package may exhibit a modulus and prove its
universal law.

`proposition` is a proof-formula declaration and generic binder kind. It has
no result, runtime value, layout, operation contract, lowering, or executable
body. A primitive fact-only declaration ends with `;`. A witness-bearing
declaration contains exactly one canonical carrierless evidence interface in
a dedicated declaration position. The owner thereby fixes both introduction
and elimination: every establishing conformance supplies that interface, and
eliminating the proposition reopens the exact retained evidence term in the
proof stratum.

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

The existential package reuses selected-conformance projection. One
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

Opening the same evidence term twice yields the same opaque symbol. Distinct
evidence terms yield distinct proof symbols even when they establish the same
proposition. Proof irrelevance applies to proposition identity and runtime
representation; it does not make hidden witness terms definitionally equal
inside a proof.

A selected dynamic value may be owned by value when its *entire normalized
dynamic representation* has no runtime carrier. Absence of table slots alone
is insufficient: a runtime instance with no eligible methods may still have
unknown size and cleanup. Carrierless convergence evidence has neither an
instance nor slots, so by-value `dyn` is proof-only and erases without
allocation, size/alignment metadata, or cleanup.

Published mathematical APIs expose proposition names such as
`ConvergesTogether(a, b)` and transparent conveniences such as `Cauchy(s)`.
The underlying carrierless selected conformance is mechanism, not user-facing
mathematical vocabulary.

## Relation-property hierarchy

Relation laws are ordinary explicit conformances. The compiler does not find
free machines by suffix or privileged global name.

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
Several proofs may satisfy the same property. Home-satisfier resolution selects
one when unique; ambiguity uses the ordinary named-conformance selection.
Changing the proof conformance does not change the nominal relation or quotient
identity.

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
data Real = CauchySeq % ConvergesTogether;
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
argument record:

```text
F : Arguments -> Result
requires P(arguments)
```

Let `RA` relate argument records fieldwise and `RR` relate results.
`Respects<F, RA, RR>` proves two clauses.

### Domain invariance

```text
RA(x, y) -> (P(x) <-> P(y))
```

Equivalent representatives must agree on whether the operation is callable.
For a total machine `P` is true and this clause discharges structurally.
Only semantic preconditions depending on the representative participate.
Fixed ambient facts, authority, and resource requirements do not vary by
representative and remain ordinary machine-contract obligations.

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
relation. The argument-record normalization avoids an open-ended
`Respects1`/`Respects2`/`Respects3` hierarchy.

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

The evidence producer checks the selected conformance. A consumer opens the
published evidence to artifact-local opaque symbols characterized solely by
the normalized contracts; witness implementation identity does not cross the
boundary. Proof-only evaluation, when a transparent body is actually needed,
uses the ordinary gated build-time evaluator: semantic eligibility requires
the complete checked invocation contract and ordinary termination. Deterministic
work metering supplies progress, caching/accounting evidence, warnings, and any
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
2. carrierless evidence owns no runtime words and opens to stable opaque
   symbols;
3. a proof of a witness-bearing proposition without its evidence rejects;
4. a literally bodyless ordinary theorem machine rejects, an explicit boundary
   axiom is reported as admitted, and an equivalence depending on it cannot
   license `%`;
5. different family indices may relate, while a different carrier family
   rejects;
6. quotient formation requires explicit reflexive, symmetric, and transitive
   conformances through `Equivalence`;
7. a total respecting operation lifts;
8. a partial operation rejects unless equivalent representatives agree on its
   precondition;
9. an operation whose result depends on representative choice rejects; and
10. no compiler rule mentions `Rat`, `Real`, Cauchy sequences, moduli, or
   convergence.
